//! Source tokens, expressions and statements shared by HGL compiler stages.
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
/// Types admitted by the executable source compiler.
pub enum Ty {
    /// Signed integer.
    I64,
    /// Binary floating point.
    F64,
    /// Boolean.
    Bool,
    /// UTF-8 text.
    Str,
    /// Microsecond interval.
    Duration,
    /// Calendar date.
    Date,
    /// Time of day.
    Time,
    /// UTC instant.
    DateTime,
    /// Reference designation.
    Ref(Box<Self>),
    /// Set membership.
    Set(Box<Self>),
    /// No result.
    Void,
}
impl Ty {
    /// Canonical scalar spelling; constructed types retain their child separately.
    pub fn name(&self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Str => "str",
            Self::Duration => "duration",
            Self::Date => "date",
            Self::Time => "time",
            Self::DateTime => "datetime",
            Self::Ref(_) => "ref",
            Self::Set(_) => "set",
            Self::Void => "void",
        }
    }
    /// Parse a concrete admitted type spelling.
    pub fn parse(name: &str) -> Option<Self> {
        if let Some(child) = name.strip_prefix("ref<").and_then(|s| s.strip_suffix('>')) {
            return Some(Self::Ref(Box::new(Self::parse(child)?)));
        }
        if let Some(child) = name.strip_prefix("set<").and_then(|s| s.strip_suffix('>')) {
            return Some(Self::Set(Box::new(Self::parse(child)?)));
        }
        match name {
            "i64" => Some(Self::I64),
            "f64" => Some(Self::F64),
            "bool" => Some(Self::Bool),
            "str" => Some(Self::Str),
            "duration" => Some(Self::Duration),
            "date" => Some(Self::Date),
            "time" => Some(Self::Time),
            "datetime" => Some(Self::DateTime),
            "void" => Some(Self::Void),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq)]
/// A fixed scalar value in HGL source.
pub enum Literal {
    /// Signed integer value.
    Int(i64),
    /// Floating value.
    Float(f64),
    /// Boolean.
    Bool(bool),
    /// UTF-8 text.
    Str(String),
    /// Microsecond interval.
    Duration(i64),
    /// Calendar date.
    Date(i64),
    /// Time of day.
    Time(i64),
    /// UTC instant.
    DateTime(i64),
}
impl Literal {
    /// The literal scalar type.
    pub fn ty(&self) -> Ty {
        match self {
            Self::Int(_) => Ty::I64,
            Self::Float(_) => Ty::F64,
            Self::Bool(_) => Ty::Bool,
            Self::Str(_) => Ty::Str,
            Self::Duration(_) => Ty::Duration,
            Self::Date(_) => Ty::Date,
            Self::Time(_) => Ty::Time,
            Self::DateTime(_) => Ty::DateTime,
        }
    }
}
#[derive(Debug, Clone)]
/// An expression before name and type resolution.
pub enum Expr {
    /// Fixed scalar.
    Literal(Literal),
    /// Unresolved identifier.
    Name(String),
    /// Dense harness cells; absent cells carry no tick.
    Sequence(Vec<Option<Self>>),
    /// Unary operator.
    Unary(String, Box<Self>),
    /// Callable name and positional or named arguments.
    Call(String, Vec<(Option<String>, Self)>),
    /// Binary operator.
    Binary(String, Box<Self>, Box<Self>),
}
#[derive(Debug, Clone)]
/// A statement before phase and type checking.
pub enum Stmt {
    /// Local binding.
    Let(String, Expr),
    /// Result publication.
    Return(Expr),
    /// Callable name and positional or named arguments.
    Call(Expr),
    /// Increment a state or cache slot.
    Add(String, Expr),
    /// Assignment to state, cache or out.
    Assign(String, Expr),
    /// Collection iteration.
    For(String, Expr, Vec<Self>),
    /// Conditional branches.
    If(Expr, Vec<Self>, Vec<Self>),
}

/// A bounded parser cursor over source tokens.
#[derive(Debug)]
pub struct Cursor<'a> {
    /// Borrowed token stream.
    pub tokens: &'a [Token],
    /// Index of the next token.
    pub pos: usize,
}
impl<'a> Cursor<'a> {
    /// Start at the first token.
    pub fn new(tokens: &'a [Token]) -> Self {
        Self { tokens, pos: 0 }
    }
    /// Whether the next token has this spelling.
    pub fn at(&self, s: &str) -> bool {
        self.peek() == s
    }
    /// Next spelling, or an empty string at the end.
    pub fn peek(&self) -> &str {
        self.tokens.get(self.pos).map_or("", |t| t.text.as_str())
    }
    /// Consume the next token only when it matches.
    pub fn take(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    /// Consume one token, diagnosing end of input.
    pub fn consume(&mut self) -> Result<String, String> {
        if self.at("") {
            return Err("unexpected end of declaration".into());
        }
        let s = self.peek().to_owned();
        self.pos += 1;
        Ok(s)
    }
    /// Require and consume the requested token.
    pub fn need(&mut self, s: &str) -> Result<(), String> {
        if self.take(s) {
            Ok(())
        } else {
            Err(format!("expected {s}, found {}", self.peek()))
        }
    }
    /// Consume declaration or statement line separators.
    pub fn lines(&mut self) {
        while self.take("\n") {}
    }
    /// Require an identifier spelling.
    pub fn name(&mut self) -> Result<String, String> {
        let name = self.consume()?;
        if !name
            .bytes()
            .enumerate()
            .all(|(i, c)| c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit()))
        {
            return Err(format!("expected name, found {name}"));
        }
        Ok(name)
    }
    /// The literal scalar type.
    pub fn type_name(&mut self) -> Result<String, String> {
        let mut name = self.name()?;
        if self.take("<") {
            let child = self.type_name()?;
            if name == "ref" && child.starts_with("ref<") {
                return Err("explicit ref<ref<T>> is not valid".into());
            }
            name.push('<');
            name.push_str(&child);
            self.need(">")?;
            name.push('>');
        }
        Ok(name)
    }
    /// Parse an expression using operator precedence.
    pub fn expr(&mut self) -> Result<Expr, String> {
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
                "%" | "*" | "/" => 5,
                _ => 0,
            };
            if precedence == 0 || precedence < min {
                break;
            }
            let op = self.consume()?;
            self.lines();
            let right = self.binary(precedence + 1)?;
            left = Expr::Binary(op, Box::new(left), Box::new(right));
        }
        Ok(left)
    }
    fn atom(&mut self) -> Result<Expr, String> {
        if self.take("[") {
            let mut values = Vec::new();
            self.lines();
            while !self.take("]") {
                values.push(if self.take("_") {
                    None
                } else {
                    Some(self.expr()?)
                });
                self.lines();
                if !self.take(",") {
                    self.need("]")?;
                    break;
                }
                self.lines();
            }
            return Ok(Expr::Sequence(values));
        }
        if self.take("(") {
            self.lines();
            let value = self.expr()?;
            self.lines();
            self.need(")")?;
            return Ok(value);
        }
        if self.take("!") {
            return Ok(Expr::Unary("!".into(), Box::new(self.atom()?)));
        }
        let negative = self.take("-");
        let text = self.consume()?;
        if let Some(literal) = numeric_literal(&text, negative)? {
            return Ok(Expr::Literal(literal));
        }
        if negative {
            self.pos -= 1;
            return Ok(Expr::Unary("-".into(), Box::new(self.atom()?)));
        }
        if text == "true" || text == "false" {
            return Ok(Expr::Literal(Literal::Bool(text == "true")));
        }
        if text.starts_with('"') {
            return string_literal(&text);
        }
        let mut name = text;
        if self.at("::") || self.at(".") {
            name.push_str(&self.consume()?);
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
    fn else_body(&mut self) -> Result<Vec<Stmt>, String> {
        if !self.take("if") {
            return self.block();
        }
        let condition = self.expr()?;
        let yes = self.block()?;
        self.lines();
        let no = if self.take("else") {
            self.else_body()?
        } else {
            Vec::new()
        };
        Ok(vec![Stmt::If(condition, yes, no)])
    }
    /// Parse a brace-delimited statement block.
    pub fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.need("{")?;
        self.lines();
        let mut out = Vec::new();
        while !self.take("}") {
            let statement = if self.take("for") {
                let name = self.name()?;
                self.need("in")?;
                let collection = self.expr()?;
                let body = self.block()?;
                out.push(Stmt::For(name, collection, body));
                self.lines();
                continue;
            } else if self.take("let") || self.take("var") {
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
                    self.else_body()?
                } else {
                    Vec::new()
                };
                out.push(Stmt::If(condition, yes, no));
                self.lines();
                continue;
            } else if self.tokens.get(self.pos + 1).is_some_and(|t| t.text == "=") {
                let name = self.name()?;
                self.need("=")?;
                Stmt::Assign(name, self.expr()?)
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

impl Expr {
    /// Evaluate supported fixed expressions without engine or native calls.
    pub fn fixed(&self) -> Option<Literal> {
        match self {
            Self::Literal(l) => Some(l.clone()),
            Self::Binary(op, a, b) => match (op.as_str(), a.fixed()?, b.fixed()?) {
                ("-", Literal::Duration(a), Literal::Duration(b)) => {
                    a.checked_sub(b).map(Literal::Duration)
                }
                ("+", Literal::Duration(a), Literal::Duration(b)) => {
                    a.checked_add(b).map(Literal::Duration)
                }
                ("+", Literal::Int(a), Literal::Int(b)) => a.checked_add(b).map(Literal::Int),
                ("-", Literal::Int(a), Literal::Int(b)) => a.checked_sub(b).map(Literal::Int),
                _ => None,
            },
            Self::Unary(op, v) => match (op.as_str(), v.fixed()?) {
                ("-", Literal::Int(n)) => n.checked_neg().map(Literal::Int),
                ("-", Literal::Float(f)) => Some(Literal::Float(-f)),
                ("!", Literal::Bool(b)) => Some(Literal::Bool(!b)),
                _ => None,
            },
            Self::Name(_) | Self::Call(..) | Self::Sequence(_) => None,
        }
    }
}

fn numeric_literal(text: &str, negative: bool) -> Result<Option<Literal>, String> {
    if let Some(text) = text.strip_prefix('@') {
        if negative {
            return Err("cannot negate a calendar literal".into());
        }
        let value = if text.contains('T') {
            Literal::DateTime(hgl_calendar::datetime(text)?.micros())
        } else if text.contains(':') {
            Literal::Time(hgl_calendar::time(text)?.0)
        } else {
            Literal::Date(hgl_calendar::date(text)?.0)
        };
        return Ok(Some(value));
    }
    if !text.as_bytes()[0].is_ascii_digit() {
        return Ok(None);
    }
    if text.contains('.') || text.contains('e') || text.contains('E') {
        let value = text
            .parse::<f64>()
            .map_err(|e| format!("invalid float: {e}"))?;
        if !value.is_finite() {
            return Err("float literal is outside the finite f64 range".into());
        }
        return Ok(Some(Literal::Float(if negative { -value } else { value })));
    }
    let digits = text.bytes().take_while(u8::is_ascii_digit).count();
    if digits < text.len() {
        let value = hgl_calendar::duration(text)?.micros();
        return Ok(Some(Literal::Duration(if negative {
            -value
        } else {
            value
        })));
    }
    let value = format!("{}{text}", if negative { "-" } else { "" })
        .parse::<i64>()
        .map_err(|e| format!("invalid i64: {e}"))?;
    Ok(Some(Literal::Int(value)))
}
