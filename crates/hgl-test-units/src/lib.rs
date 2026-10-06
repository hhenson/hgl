//! Source extents for mixed rejection and executable tests, before semantic checking.
use hgl_source::{Cursor, Token, lex};
use std::ops::Range;
/// An independently bounded declaration in its original lexical scope.
#[derive(Debug, Clone)]
pub struct Unit {
    /// Original byte range, without changing test-context wrappers.
    pub span: Range<usize>,
    /// One-based declaration-start line.
    pub line: usize,
    /// Whether this declaration is the first syntax on its start line.
    pub first_on_line: bool,
    /// Last physical line owned by this declaration.
    pub end_line: usize,
    /// Owning module identity.
    pub module: String,
    /// Recoverable declaration name, absent for malformed declarations.
    pub name: Option<String>,
    /// Whether this unit is a named test rather than a module check.
    pub test: bool,
}
impl Unit {
    /// Qualified selector identity for a named test.
    pub fn test_name(&self) -> Option<String> {
        self.test
            .then(|| {
                self.name
                    .as_ref()
                    .map(|name| format!("{}::{name}", self.module))
            })
            .flatten()
    }
}
/// Find reliable declaration boundaries, rejecting ambiguous recovery.
pub fn units(text: &str) -> Result<Vec<Unit>, String> {
    let tokens = lex(text)?;
    let mut cursor = Cursor::new(&tokens);
    cursor.lines();
    if cursor.peek().starts_with("/**") {
        cursor.consume()?;
        cursor.lines();
    }
    cursor.need("module")?;
    let mut module = cursor.name()?;
    while cursor.take(".") {
        module.push('.');
        module.push_str(&cursor.name()?);
    }
    if cursor.take("part") {
        cursor.name()?;
    }
    if !cursor.at("\n") && !cursor.at("") {
        return Err("invalid module header".into());
    }
    let mut result = Vec::new();
    scope(
        &tokens,
        cursor.pos,
        tokens.len(),
        text,
        &module,
        &mut result,
    )?;
    Ok(result)
}
fn scope(
    tokens: &[Token],
    mut index: usize,
    end: usize,
    text: &str,
    module: &str,
    result: &mut Vec<Unit>,
) -> Result<(), String> {
    while index < end {
        if tokens[index].text == "\n" || tokens[index].text.starts_with("/**") {
            index += 1;
            continue;
        }
        if !boundary(tokens, index) {
            return Err(format!(
                "cannot establish declaration boundary at line {}",
                line(text, tokens[index].span.start)
            ));
        }
        if tokens[index].text == "test" && tokens.get(index + 1).is_some_and(|t| t.text == "{") {
            let close = closing(tokens, index + 1, end)?;
            scope(tokens, index + 2, close, text, module, result)?;
            index = close + 1;
            continue;
        }
        let after = extent(tokens, index, end)?;
        let start = tokens[index].span.start;
        let finish = if after == end {
            tokens.get(end).map_or(text.len(), |t| t.span.start)
        } else {
            tokens[after - 1].span.end
        };
        let (name, is_test) = identity(&tokens[index..after]);
        result.push(Unit {
            span: start..finish,
            line: line(text, start),
            first_on_line: index == 0 || tokens[index - 1].text == "\n",
            end_line: line(text, finish.saturating_sub(1).max(start)),
            module: module.into(),
            name,
            test: is_test,
        });
        index = after;
    }
    Ok(())
}
fn identity(tokens: &[Token]) -> (Option<String>, bool) {
    let mut cursor = Cursor::new(tokens);
    cursor.take("export");
    cursor.take("impl");
    cursor.take("abstract");
    if cursor.take("native") {
        cursor.take("const");
    }
    cursor.take("const");
    let test = cursor.take("test");
    if !test {
        let _kind = cursor.consume();
    }
    (cursor.name().ok(), test)
}
fn boundary(tokens: &[Token], index: usize) -> bool {
    let word = tokens.get(index).map_or("", |t| t.text.as_str());
    matches!(
        word,
        "fn" | "test"
            | "native"
            | "export"
            | "impl"
            | "operator"
            | "struct"
            | "abstract"
            | "enum"
            | "use"
            | "instantiate"
    ) || (word == "const" && tokens.get(index + 1).is_some_and(|t| t.text == "fn"))
}
fn closing(tokens: &[Token], start: usize, end: usize) -> Result<usize, String> {
    let mut depth = 0;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.text.as_str() {
            "{" => depth += 1,
            "}" => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return Ok(index);
        }
    }
    Err("ambiguous rejection isolation: unclosed body or test context".into())
}
fn extent(tokens: &[Token], start: usize, end: usize) -> Result<usize, String> {
    let mut depth = 0_i32;
    let mut constraint_end = None;
    for index in start..end {
        if constraint_end.is_some_and(|close| index <= close) {
            continue;
        }
        match tokens[index].text.as_str() {
            "{" => {
                let close = closing(tokens, index, end)?;
                if tokens[start..index]
                    .iter()
                    .rev()
                    .find(|t| t.text != "\n")
                    .is_some_and(|t| t.text == "in")
                {
                    constraint_end = Some(close);
                    continue;
                }
                if close + 1 < end && tokens[close + 1].text != "\n" {
                    return Err("ambiguous declaration suffix after body".into());
                }
                return Ok(close + 1);
            }
            "(" | "[" => depth += 1,
            ")" | "]" => depth -= 1,
            "}" => return Err("ambiguous declaration closing delimiter".into()),
            "\n" => {
                let next = (index + 1..end)
                    .find(|i| tokens[*i].text != "\n")
                    .unwrap_or(end);
                if next == end {
                    return Ok(end);
                }
                if boundary(tokens, next) || tokens[next].text.starts_with("/**") {
                    if depth != 0 {
                        return Err(
                            "ambiguous rejection isolation before neighboring declaration".into(),
                        );
                    }
                    return Ok(index + 1);
                }
            }
            _ => {}
        }
    }
    Ok(end)
}
fn line(text: &str, offset: usize) -> usize {
    text.as_bytes()[..offset.min(text.len())]
        .split(|byte| *byte == b'\n')
        .count()
}
/// Exclude declarations without moving any retained diagnostic byte or line.
pub fn mask(text: &str, ranges: impl IntoIterator<Item = Range<usize>>) -> String {
    let mut bytes = text.as_bytes().to_vec();
    for range in ranges {
        for byte in &mut bytes[range] {
            if !matches!(*byte, b'\n' | b'\r') {
                *byte = b' ';
            }
        }
    }
    // Retained UTF-8 is untouched; excluded characters become one ASCII space per byte.
    String::from_utf8_lossy(&bytes).into_owned()
}
