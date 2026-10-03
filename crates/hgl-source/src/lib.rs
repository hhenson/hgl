//! Source tokens, expressions and statements shared by HGL compiler stages.
pub use hgl_lex::{Token, lex};
pub use hgl_type_shape::{Nominal, Ty, application};
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
    /// Contextual absence, checked at its use site.
    Null,
    /// Read a selected property or field of an expression.
    Property(Box<Self>, String),
    /// Indexed expression and index.
    Index(Box<Self>, Box<Self>),
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
    /// Explicit struct application and supplied named fields.
    Applied(String, Vec<(Option<String>, Self)>),
    /// Binary operator.
    Binary(String, Box<Self>, Box<Self>),
}
#[derive(Debug, Clone)]
/// A statement before phase and type checking.
pub enum Stmt {
    /// Local binding.
    Let(String, Option<String>, Expr),
    /// Mutable local binding.
    Var(String, Option<String>, Expr),
    /// End runtime evaluation without publication.
    Exit,
    /// Result publication.
    Return(Expr),
    /// Callable name and positional or named arguments.
    Call(Expr),
    /// Increment a state or cache slot.
    Add(Expr, Expr),
    /// Assignment to state, cache or out.
    Assign(Expr, Expr),
    /// Ordered timed source publication operands.
    TimedYield(Expr, Expr),
    /// Runtime conditional loop; omitted source condition is true.
    While(Expr, Vec<Self>),
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
        if matches!(name.as_str(), "while" | "yield") {
            return Err(format!("{name} is a reserved word"));
        }
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
        while self.take("::") {
            name.push_str("::");
            name.push_str(&self.name()?);
        }
        if self.at("<") {
            name.push_str(&self.type_arguments()?);
            if name.starts_with("ref<ref<") {
                return Err("explicit ref<ref<T>> is not valid".into());
            }
        }
        Ok(name)
    }
    fn type_arguments(&mut self) -> Result<String, String> {
        self.need("<")?;
        let mut text = String::from("<");
        let mut depth = 1;
        while depth > 0 {
            let token = self.consume()?;
            if token == "<" {
                depth += 1;
            }
            if token == ">" {
                depth -= 1;
            }
            if token != "\n" {
                text.push_str(&token);
            }
        }
        Ok(text)
    }
    fn applied_constructor(&self) -> bool {
        if !self.at("<") {
            return false;
        }
        let mut depth = 0;
        let mut parentheses = 0;
        let mut previous = "";
        for (index, token) in self.tokens.iter().enumerate().skip(self.pos) {
            let text = token.text.as_str();
            if parentheses > 0 {
                if text == "(" {
                    parentheses += 1;
                }
                if text == ")" {
                    parentheses -= 1;
                }
                previous = text;
                continue;
            }
            match text {
                "<" => depth += 1,
                ">" => {
                    depth -= 1;
                    if depth == 0 {
                        return self.tokens.get(index + 1).is_some_and(|t| t.text == "(");
                    }
                }
                "(" => parentheses = 1,
                "\n" => {
                    let next = self.tokens[index + 1..].iter().find(|t| t.text != "\n");
                    if !matches!(previous, "<" | ",")
                        && !next.is_some_and(|t| matches!(t.text.as_str(), ">" | ","))
                    {
                        return false;
                    }
                    continue;
                }
                "," | "::" | "+" | "-" | "*" | "/" | "%" => {}
                other if generic_token(other) => {}
                _ => return false,
            }
            previous = text;
        }
        false
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
        let mut value = self.primary()?;
        loop {
            if self.take("[") {
                let index = self.expr()?;
                self.need("]")?;
                value = Expr::Index(Box::new(value), Box::new(index));
            } else if self.take(".") {
                value = Expr::Property(Box::new(value), self.name()?);
                if self.at("(") {
                    return Err(
                        "properties cannot be invoked; capability method calls are not admitted"
                            .into(),
                    );
                }
            } else {
                break;
            }
        }
        Ok(value)
    }
    fn primary(&mut self) -> Result<Expr, String> {
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
        if text == "null" {
            return Ok(Expr::Null);
        }
        if text == "true" || text == "false" {
            return Ok(Expr::Literal(Literal::Bool(text == "true")));
        }
        if text.starts_with('"') {
            return string_literal(&text);
        }
        let mut name = text;
        while self.at("::") {
            name.push_str(&self.consume()?);
            name.push_str(&self.name()?);
        }
        let applied = self.applied_constructor();
        if applied {
            name.push_str(&self.type_arguments()?);
        }
        if !self.take("(") {
            return Ok(Expr::Name(name));
        }
        let args = self.call_arguments()?;
        Ok(if applied {
            Expr::Applied(name, args)
        } else {
            Expr::Call(name, args)
        })
    }
    fn call_arguments(&mut self) -> Result<Vec<(Option<String>, Expr)>, String> {
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
        Ok(args)
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
        self.block_contents()
    }
    /// Parse a block after its opening brace has already been consumed.
    pub fn block_contents(&mut self) -> Result<Vec<Stmt>, String> {
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
            } else if self.take("while") {
                let condition = if self.at("{") {
                    Expr::Literal(Literal::Bool(true))
                } else {
                    self.expr()?
                };
                let body = self.block()?;
                out.push(Stmt::While(condition, body));
                self.lines();
                continue;
            } else if self.take("yield") {
                let time = self.expr()?;
                self.need(":")?;
                Stmt::TimedYield(time, self.expr()?)
            } else if self.at("let") || self.at("var") {
                let mutable = self.consume()? == "var";
                let name = self.name()?;
                let annotation = if self.take(":") {
                    Some(self.type_name()?)
                } else {
                    None
                };
                self.need("=")?;
                if mutable {
                    Stmt::Var(name, annotation, self.expr()?)
                } else {
                    Stmt::Let(name, annotation, self.expr()?)
                }
            } else if self.take("return") {
                if self.at("}") || self.at("\n") {
                    Stmt::Exit
                } else {
                    Stmt::Return(self.expr()?)
                }
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
            } else {
                let target = self.expr()?;
                if self.take("=") {
                    Stmt::Assign(target, self.expr()?)
                } else if self.take("+=") {
                    Stmt::Add(target, self.expr()?)
                } else {
                    Stmt::Call(target)
                }
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
            Self::Null
            | Self::Property(..)
            | Self::Index(..)
            | Self::Name(_)
            | Self::Call(..)
            | Self::Applied(..)
            | Self::Sequence(_) => None,
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

impl Expr {
    /// Expand contextual selectors and supply missing top-level handler defaults.
    #[must_use]
    pub fn handler_guard(self, inputs: &[String]) -> Self {
        if inputs.is_empty() {
            return self;
        }
        let args = inputs
            .iter()
            .map(|n| (None, Self::Name(n.clone())))
            .collect::<Vec<_>>();
        let (mut expr, selectors) = expand_selectors(self, &args);
        for (name, flag) in [("modified", 1), ("valid", 2)] {
            if selectors & flag == 0 {
                expr = Self::Binary(
                    "&&".into(),
                    Box::new(Self::Call(name.into(), args.clone())),
                    Box::new(expr),
                );
            }
        }
        expr
    }
}
fn expand_selectors(expr: Expr, inputs: &[(Option<String>, Expr)]) -> (Expr, u8) {
    match expr {
        Expr::Binary(op, a, b) if op == "&&" => {
            let (a, left) = expand_selectors(*a, inputs);
            let (b, right) = expand_selectors(*b, inputs);
            (Expr::Binary(op, Box::new(a), Box::new(b)), left | right)
        }
        Expr::Call(name, args) if matches!(name.as_str(), "valid" | "modified") => {
            let flag = if name == "modified" { 1 } else { 2 };
            (
                Expr::Call(
                    name,
                    if args.is_empty() {
                        inputs.to_vec()
                    } else {
                        args
                    },
                ),
                flag,
            )
        }
        other @ (Expr::Null
        | Expr::Property(..)
        | Expr::Index(..)
        | Expr::Literal(_)
        | Expr::Name(_)
        | Expr::Sequence(_)
        | Expr::Unary(..)
        | Expr::Call(..)
        | Expr::Applied(..)
        | Expr::Binary(..)) => (other, 0),
    }
}

fn generic_token(text: &str) -> bool {
    if matches!(
        text,
        "fn" | "struct"
            | "abstract"
            | "module"
            | "use"
            | "as"
            | "export"
            | "native"
            | "const"
            | "let"
            | "var"
            | "return"
            | "if"
            | "else"
            | "for"
            | "in"
            | "requires"
            | "operator"
            | "impl"
            | "test"
            | "when"
            | "start"
            | "stop"
            | "throws"
    ) {
        return false;
    }
    text.starts_with('"')
        || text.starts_with('@')
        || text
            .bytes()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == b'_')
}
