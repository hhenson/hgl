//! Source tokenization without name or phase resolution.
use std::ops::Range;

#[derive(Debug, Clone)]
/// One source token, retaining its original byte range.
pub struct Token {
    /// Original spelling, with line endings normalized to newline.
    pub text: String,
    /// Half-open byte range in the original source.
    pub span: Range<usize>,
}

/// Tokenize HGL without interpreting identifiers or discarding documentation.
pub fn lex(text: &str) -> Result<Vec<Token>, String> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < text.len() {
        let start = pos;
        let c = text.as_bytes()[pos];
        pos += 1;
        match c {
            b' ' | b'\t' => continue,
            b'\n' | b'\r' => {
                if c == b'\r' && text.as_bytes().get(pos) == Some(&b'\n') {
                    pos += 1;
                }
                out.push(Token {
                    text: "\n".into(),
                    span: start..pos,
                });
                continue;
            }
            b'#' => {
                while pos < text.len() && !matches!(text.as_bytes()[pos], b'\r' | b'\n') {
                    pos += 1;
                }
                continue;
            }
            b'/' if text.as_bytes().get(pos) == Some(&b'*') => {
                let end = text[pos + 1..].find("*/").ok_or("unterminated comment")?;
                pos += end + 3;
                if !text[start..pos].starts_with("/**") {
                    continue;
                }
            }
            b'"' => {
                let mut closed = false;
                while pos < text.len() {
                    let next = text.as_bytes()[pos];
                    pos += 1;
                    if next == b'\\' {
                        pos += usize::from(pos < text.len());
                    } else if next == b'"' {
                        closed = true;
                        break;
                    }
                }
                if !closed {
                    return Err("unterminated string".into());
                }
            }
            b'0'..=b'9' => {
                while pos < text.len() {
                    let next = text.as_bytes()[pos];
                    if next.is_ascii_alphanumeric()
                        || next == b'.'
                        || next == b'_'
                        || (matches!(next, b'+' | b'-')
                            && matches!(text.as_bytes()[pos - 1], b'e' | b'E'))
                    {
                        pos += 1;
                    } else {
                        break;
                    }
                }
            }
            b'@' => {
                while pos < text.len()
                    && (text.as_bytes()[pos].is_ascii_alphanumeric()
                        || matches!(text.as_bytes()[pos], b'-' | b':' | b'.' | b'+'))
                {
                    pos += 1;
                    if text.as_bytes()[pos - 1] == b'Z' {
                        break;
                    }
                }
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                while pos < text.len()
                    && (text.as_bytes()[pos].is_ascii_alphanumeric()
                        || text.as_bytes()[pos] == b'_')
                {
                    pos += 1;
                }
            }
            _ => {
                pos = start + text[start..].chars().next().map_or(1, char::len_utf8);
                if pos < text.len()
                    && matches!(
                        text.get(start..=pos).unwrap_or(""),
                        "::" | "->" | "=>" | "+=" | "==" | "!=" | "<=" | ">=" | "&&" | "||"
                    )
                {
                    pos += 1;
                }
            }
        }
        out.push(Token {
            text: text[start..pos].into(),
            span: start..pos,
        });
    }
    Ok(out)
}
