use crate::model::{Kind, Token};

pub fn handwritten(source: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut n = 0;
    while n < source.len() {
        let start = n;
        let rest = &source[n..];
        let c = rest.as_bytes()[0];
        let kind = if let Some(tail) = rest.strip_prefix("/*") {
            if let Some(end) = tail.find("*/") {
                n += end + 4;
                Kind::Comment
            } else {
                n = source.len();
                Kind::Error
            }
        } else if c == b'#' {
            n += rest.find(['\r', '\n']).unwrap_or(rest.len());
            Kind::Comment
        } else if rest.starts_with("->") {
            n += 2;
            Kind::Arrow
        } else if rest.starts_with("=>") {
            n += 2;
            Kind::FatArrow
        } else if c.is_ascii_alphabetic() || c == b'_' {
            n += rest
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
                .count();
            match &source[start..n] {
                "module" => Kind::Module,
                "struct" => Kind::Struct,
                "fn" => Kind::Fn,
                _ => Kind::Name,
            }
        } else if c.is_ascii_digit() {
            n += rest.bytes().take_while(u8::is_ascii_digit).count();
            Kind::Int
        } else if c == b' ' || c == b'\t' {
            n += rest
                .bytes()
                .take_while(|b| *b == b' ' || *b == b'\t')
                .count();
            Kind::Space
        } else {
            n += rest.chars().next().unwrap().len_utf8();
            match c {
                b'\r' => {
                    if source.as_bytes().get(n) == Some(&b'\n') {
                        n += 1;
                    }
                    Kind::Newline
                }
                b'\n' => Kind::Newline,
                b'(' => Kind::LParen,
                b')' => Kind::RParen,
                b'{' => Kind::LBrace,
                b'}' => Kind::RBrace,
                b'<' => Kind::Less,
                b'>' => Kind::Greater,
                b':' => Kind::Colon,
                b',' => Kind::Comma,
                b'.' => Kind::Dot,
                b'+' => Kind::Plus,
                b'*' => Kind::Star,
                _ => Kind::Error,
            }
        };
        tokens.push(Token {
            kind,
            span: start..n,
        });
    }
    tokens
}
#[cfg(feature = "logos")]
pub fn generated(source: &str) -> Vec<Token> {
    use logos::Logos;
    Kind::lexer(source)
        .spanned()
        .map(|(kind, span)| Token {
            kind: kind.unwrap_or(Kind::Error),
            span,
        })
        .collect()
}

// Both lexers retain trivia; only the parser-facing view applies newline rules.
pub fn significant(source: &str, raw: &[Token]) -> Vec<Token> {
    let mut result: Vec<Token> = Vec::new();
    let mut depth = 0i32;
    for token in raw {
        let mut t = token.clone();
        if t.kind == Kind::Comment {
            let Some(offset) = source[t.span.clone()].find(['\n', '\r']) else {
                continue;
            };
            t.kind = Kind::Newline;
            t.span = (t.span.start + offset)..(t.span.start + offset + 1);
        }
        if t.kind == Kind::Space {
            continue;
        }
        if t.kind == Kind::Newline
            && (depth > 0
                || result.last().is_some_and(|p| {
                    matches!(
                        p.kind,
                        Kind::Newline
                            | Kind::FatArrow
                            | Kind::Arrow
                            | Kind::Plus
                            | Kind::Star
                            | Kind::Comma
                            | Kind::Colon
                    )
                }))
        {
            continue;
        }
        match t.kind {
            Kind::LParen | Kind::Less => depth += 1,
            Kind::RParen | Kind::Greater => depth = (depth - 1).max(0),
            // Declaration recovery cannot inherit an unterminated signature's depth.
            Kind::Fn | Kind::Struct => depth = 0,
            _ => (),
        }
        result.push(t);
    }
    result
}
