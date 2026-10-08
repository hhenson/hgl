//! Lexical source-rejection annotations; compiler errors retain independent origins.
use hgl_source::diagnostics::{CATEGORIES, SOURCE_CODES};
use hgl_source::{Cursor, Expr, Literal, lex};
/// One expectation targeting exactly the following physical source line.
#[derive(Debug, Clone)]
pub struct Expectation {
    /// One-based target line.
    pub line: usize,
    /// Catalogue diagnostic category.
    pub category: String,
    /// Exact catalogue error identifier.
    pub code: String,
}
/// Read only actual standalone comment annotations, validating all metadata.
pub fn annotations(text: &str) -> Result<Vec<Expectation>, String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut line = 1;
    let mut standalone = true;
    let mut expected = Vec::new();
    while index < bytes.len() {
        match bytes[index] {
            b'\n' => {
                line += 1;
                standalone = true;
                index += 1;
            }
            b' ' | b'\t' | b'\r' => index += 1,
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                standalone = false;
                index += 2;
                while index < bytes.len()
                    && !(bytes[index] == b'*' && bytes.get(index + 1) == Some(&b'/'))
                {
                    if bytes[index] == b'\n' {
                        line += 1;
                    }
                    index += 1;
                }
                index = (index + 2).min(bytes.len());
            }
            b'"' => {
                standalone = false;
                index += 1;
                while index < bytes.len() {
                    let next = bytes[index];
                    index += 1;
                    if next == b'\n' {
                        line += 1;
                    }
                    if next == b'\\' {
                        index = (index + 1).min(bytes.len());
                    } else if next == b'"' {
                        break;
                    }
                }
            }
            b'#' => {
                let start = index + 1;
                while index < bytes.len() && bytes[index] != b'\n' {
                    index += 1;
                }
                let comment = text[start..index].trim();
                if standalone && comment.starts_with("expect-error") {
                    if line >= text.lines().count() {
                        return Err(format!(
                            "line {line}: expectation has no following source line"
                        ));
                    }
                    expected.push(
                        annotation(comment, line + 1)
                            .map_err(|e| format!("line {line}: invalid expectation: {e}"))?,
                    );
                }
            }
            _ => {
                standalone = false;
                index += 1;
            }
        }
    }
    Ok(expected)
}
fn annotation(comment: &str, line: usize) -> Result<Expectation, String> {
    let suffix = comment
        .strip_prefix("expect-error")
        .ok_or("expected expect-error")?;
    let tokens = lex(suffix)?;
    let mut cursor = Cursor::new(&tokens);
    cursor.need("(")?;
    let mut category = cursor.name()?;
    if cursor.take("-") {
        category.push('-');
        category.push_str(&cursor.name()?);
    }
    cursor.need(",")?;
    let start = cursor.pos;
    let Expr::Literal(Literal::Str(code)) = cursor.expr()?.syntax().clone() else {
        return Err("expected literal source-error code".into());
    };
    if cursor.pos != start + 1 {
        return Err("expected one string literal".into());
    }
    cursor.need(")")?;
    let end = tokens
        .get(cursor.pos.saturating_sub(1))
        .map_or(0, |t| t.span.end);
    if !cursor.at("") || !suffix[end..].trim().is_empty() {
        return Err("unexpected annotation suffix".into());
    }
    if !CATEGORIES.contains(&category.as_str()) {
        return Err(format!("unknown diagnostic category {category}"));
    }
    if category == "build" {
        return Err("build is not an eligible source-rejection category".into());
    }
    if !SOURCE_CODES.contains(&(category.as_str(), code.as_str())) {
        return Err(format!(
            "unknown or mismatched category/code {category}[{code}]"
        ));
    }
    Ok(Expectation {
        line,
        category,
        code,
    })
}
