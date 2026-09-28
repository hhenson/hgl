use crate::{Diagnostic, Source};
use std::ops::Range;

#[derive(Debug)]
pub(crate) struct Token {
    pub text: String,
    pub span: Range<usize>,
}

pub(crate) fn lex(source: &Source) -> Result<Vec<Token>, Diagnostic> {
    let mut tokens = Vec::new();
    let mut offset = 0;
    let bytes = source.text.as_bytes();
    while offset < bytes.len() {
        let start = offset;
        let byte = bytes[offset];
        offset += 1;
        match byte {
            b' ' | b'\t' => continue,
            b'#' => {
                while offset < bytes.len() && !matches!(bytes[offset], b'\r' | b'\n') {
                    offset += 1;
                }
                continue;
            }
            b'/' if bytes.get(offset) == Some(&b'*') => {
                let Some(end) = source.text[offset + 1..].find("*/") else {
                    return Err(error(source, start..bytes.len(), "unterminated comment"));
                };
                if bytes.get(start + 2) == Some(&b'*') && end > 0 {
                    offset += end + 3;
                    tokens.push(Token {
                        text: source.text[start..offset].into(),
                        span: start..offset,
                    });
                    continue;
                }
                // Preserve line boundaries even when a comment spans them.
                for (index, byte) in bytes[offset..offset + 1 + end].iter().enumerate() {
                    if *byte == b'\n' {
                        tokens.push(Token {
                            text: "\n".into(),
                            span: offset + index..offset + index + 1,
                        });
                    }
                }
                offset += end + 3;
                continue;
            }
            b'\r' | b'\n' => {
                if byte == b'\r' && bytes.get(offset) == Some(&b'\n') {
                    offset += 1;
                }
                tokens.push(Token {
                    text: "\n".into(),
                    span: start..offset,
                });
                continue;
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9' => {
                while offset < bytes.len()
                    && (bytes[offset].is_ascii_alphanumeric() || bytes[offset] == b'_')
                {
                    offset += 1;
                }
            }
            b'-' if bytes.get(offset) == Some(&b'>') => offset += 1,
            b'(' | b')' | b'{' | b'}' | b':' | b',' | b'.' | b'=' | b'-' => {}
            _ => {
                let width = source.text[start..]
                    .chars()
                    .next()
                    .map_or(1, char::len_utf8);
                return Err(error(source, start..start + width, "unsupported token"));
            }
        }
        tokens.push(Token {
            text: source.text[start..offset].to_owned(),
            span: start..offset,
        });
    }
    tokens.push(Token {
        text: String::new(),
        span: offset..offset,
    });
    Ok(tokens)
}

pub(crate) fn error(source: &Source, span: Range<usize>, message: &str) -> Diagnostic {
    Diagnostic {
        source: source.name.clone(),
        start: span.start,
        end: span.end,
        message: message.into(),
    }
}
