use std::ops::Range;

#[derive(Debug, Clone)]
pub(crate) struct Token {
    pub text: String,
    pub span: Range<usize>,
}

pub(crate) fn lex(text: &str) -> Result<Vec<Token>, String> {
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
            b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'0'..=b'9' => {
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Ty {
    I64,
    Bool,
    Str,
    Duration,
    Void,
}
impl Ty {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Duration => "duration",
            Self::Void => "void",
        }
    }
    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
            "duration" => Some(Self::Duration),
            "void" => Some(Self::Void),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Literal {
    Int(i64),
    Bool(bool),
    Str(String),
    Duration(i64),
}
impl Literal {
    pub(crate) fn ty(&self) -> Ty {
        match self {
            Self::Int(_) => Ty::I64,
            Self::Bool(_) => Ty::Bool,
            Self::Str(_) => Ty::Str,
            Self::Duration(_) => Ty::Duration,
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) enum Expr {
    Literal(Literal),
    Name(String),
    Call(String, Vec<(Option<String>, Self)>),
    Binary(String, Box<Self>, Box<Self>),
}
#[derive(Debug, Clone)]
pub(crate) enum Stmt {
    Let(String, Expr),
    Return(Expr),
    Call(Expr),
    Add(String, Expr),
    If(Expr, Vec<Self>, Vec<Self>),
}

pub(crate) struct Cursor<'a> {
    pub tokens: &'a [Token],
    pub pos: usize,
}
impl<'a> Cursor<'a> {
    pub(crate) fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }
    pub(crate) fn at(&self, s: &str) -> bool {
        self.peek() == s
    }
    pub(crate) fn peek(&self) -> &str {
        self.tokens.get(self.pos).map_or("", |t| t.text.as_str())
    }
    pub(crate) fn take(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    pub(crate) fn next(&mut self) -> Result<String, String> {
        if self.at("") {
            return Err("unexpected end of declaration".into());
        }
        let s = self.peek().to_owned();
        self.pos += 1;
        Ok(s)
    }
    pub(crate) fn need(&mut self, s: &str) -> Result<(), String> {
        if self.take(s) {
            Ok(())
        } else {
            Err(format!("expected {s}, found {}", self.peek()))
        }
    }
    pub(crate) fn lines(&mut self) {
        while self.take("\n") {}
    }
    pub(crate) fn name(&mut self) -> Result<String, String> {
        let name = self.next()?;
        if !name
            .bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit()))
        {
            return Err(format!("expected name, found {name}"));
        }
        Ok(name)
    }
    pub(crate) fn expr(&mut self) -> Result<Expr, String> {
        self.binary(0)
    }
    fn binary(&mut self, min: u8) -> Result<Expr, String> {
        let mut left = self.atom()?;
        loop {
            let precedence = match self.peek() {
                "||" => 1,
                "&&" => 2,
                "==" | "!=" | ">" | "<" | ">=" | "<=" => 3,
                "+" | "-" => 4,
                "%" | "*" => 5,
                _ => 0,
            };
            if precedence == 0 || precedence < min {
                break;
            }
            let op = self.next()?;
            let right = self.binary(precedence + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn atom(&mut self) -> Result<Expr, String> {
        if self.take("(") {
            self.lines();
            let value = self.expr()?;
            self.lines();
            self.need(")")?;
            return Ok(value);
        }
        let negative = self.take("-");
        let text = self.next()?;
        if text.as_bytes()[0].is_ascii_digit() {
            let count = text.bytes().take_while(u8::is_ascii_digit).count();
            let value: i64 = format!(
                "{}{digits}",
                if negative { "-" } else { "" },
                digits = &text[..count]
            )
            .parse()
            .map_err(|error| format!("invalid i64: {error}"))?;
            let literal = if count == text.len() {
                Literal::Int(value)
            } else {
                let factor = match &text[count..] {
                    "us" => 1,
                    "ms" => 1000,
                    "s" => 1_000_000,
                    _ => return Err("unsupported duration unit".into()),
                };
                Literal::Duration(value.checked_mul(factor).ok_or("duration overflow")?)
            };
            return Ok(Expr::Literal(literal));
        }
        if negative {
            return Err("expected integer after '-'".into());
        }
        if text == "true" || text == "false" {
            return Ok(Expr::Literal(Literal::Bool(text == "true")));
        }
        if text.starts_with('"') {
            return string_literal(&text);
        }
        let mut name = text;
        if self.take("::") {
            name.push_str("::");
            name.push_str(&self.name()?);
        }
        if !self.take("(") {
            return Ok(Expr::Name(name));
        }
        let mut args = Vec::new();
        self.lines();
        while !self.at(")") {
            let named = if self.tokens.get(self.pos + 1).is_some_and(|t| t.text == ":") {
                let name = self.name()?;
                self.need(":")?;
                Some(name)
            } else {
                None
            };
            args.push((named, self.expr()?));
            self.lines();
            if !self.take(",") {
                break;
            }
            self.lines();
        }
        self.need(")")?;
        Ok(Expr::Call(name, args))
    }
    pub(crate) fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.need("{")?;
        self.lines();
        let mut out = Vec::new();
        while !self.take("}") {
            let statement = if self.take("let") {
                let name = self.name()?;
                self.need("=")?;
                Stmt::Let(name, self.expr()?)
            } else if self.take("return") {
                Stmt::Return(self.expr()?)
            } else if self.take("if") {
                let condition = self.expr()?;
                let yes = self.block()?;
                self.lines();
                let no = if self.take("else") {
                    self.block()?
                } else {
                    Vec::new()
                };
                out.push(Stmt::If(condition, yes, no));
                self.lines();
                continue;
            } else if self
                .tokens
                .get(self.pos + 1)
                .is_some_and(|t| t.text == "+=")
            {
                let name = self.name()?;
                self.need("+=")?;
                Stmt::Add(name, self.expr()?)
            } else {
                Stmt::Call(self.expr()?)
            };
            out.push(statement);
            if !self.at("}") && !self.at("\n") {
                return Err(format!("expected statement end, found {}", self.peek()));
            }
            self.lines();
        }
        Ok(out)
    }
}

fn string_literal(text: &str) -> Result<Expr, String> {
    let mut value = String::new();
    let mut chars = text[1..text.len() - 1].chars();
    while let Some(c) = chars.next() {
        value.push(if c == '\\' {
            match chars.next() {
                Some('n') => '\n',
                Some('r') => '\r',
                Some('t') => '\t',
                Some('"') => '"',
                Some('\\') => '\\',
                _ => return Err("unsupported string escape".into()),
            }
        } else {
            c
        });
    }
    Ok(Expr::Literal(Literal::Str(value)))
}
