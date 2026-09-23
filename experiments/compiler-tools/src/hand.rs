use crate::model::{Decl, Expr, Issue, Kind, Parsed, Token, Ty};

struct Parser<'a> {
    source: &'a str,
    tokens: &'a [Token],
    pos: usize,
}
impl Parser<'_> {
    fn kind(&self) -> Option<Kind> {
        self.tokens.get(self.pos).map(|t| t.kind)
    }
    fn error(&self, expected: &str) -> Issue {
        Issue {
            span: self
                .tokens
                .get(self.pos)
                .map_or(self.source.len()..self.source.len(), |t| t.span.clone()),
            message: format!("expected {expected}"),
        }
    }
    fn take(&mut self, kind: Kind) -> Result<String, Issue> {
        if self.kind() != Some(kind) {
            return Err(self.error(&format!("{kind:?}")));
        }
        let text = self.source[self.tokens[self.pos].span.clone()].to_owned();
        self.pos += 1;
        Ok(text)
    }
    fn eat(&mut self, kind: Kind) -> bool {
        if self.kind() == Some(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn ty(&mut self) -> Result<Ty, Issue> {
        let name = self.take(Kind::Name)?;
        match name.as_str() {
            "ref" => {
                self.take(Kind::Less)?;
                let t = self.ty()?;
                self.take(Kind::Greater)?;
                Ok(Ty::Ref(Box::new(t)))
            }
            "list" => {
                self.take(Kind::Less)?;
                let t = self.ty()?;
                self.take(Kind::Comma)?;
                let n = self
                    .take(Kind::Int)?
                    .parse()
                    .map_err(|_| self.error("list length"))?;
                self.take(Kind::Greater)?;
                Ok(Ty::List(Box::new(t), n))
            }
            "i64" => Ok(Ty::I64),
            _ => Ok(Ty::Named(name)),
        }
    }
    fn field(&mut self) -> Result<(String, Ty), Issue> {
        let name = self.take(Kind::Name)?;
        self.take(Kind::Colon)?;
        Ok((name, self.ty()?))
    }
    fn expr(&mut self, min: u8) -> Result<Expr, Issue> {
        let mut left = match self.kind() {
            Some(Kind::Name) => Expr::Name(self.take(Kind::Name)?),
            Some(Kind::Int) => Expr::Int(
                self.take(Kind::Int)?
                    .parse()
                    .map_err(|_| self.error("i64 literal"))?,
            ),
            Some(Kind::LParen) => {
                self.pos += 1;
                let e = self.expr(0)?;
                self.take(Kind::RParen)?;
                e
            }
            _ => return Err(self.error("expression")),
        };
        loop {
            let precedence = match self.kind() {
                Some(Kind::Plus) => 1,
                Some(Kind::Star) => 2,
                _ => break,
            };
            if precedence < min {
                break;
            }
            self.pos += 1;
            let right = self.expr(precedence + 1)?;
            left = if precedence == 1 {
                Expr::Add(Box::new(left), Box::new(right))
            } else {
                Expr::Mul(Box::new(left), Box::new(right))
            };
        }
        Ok(left)
    }
    fn generic_params(&mut self) -> Result<Vec<String>, Issue> {
        let mut generics = Vec::new();
        if !self.eat(Kind::Less) {
            return Ok(generics);
        }
        loop {
            generics.push(self.take(Kind::Name)?);
            if !self.eat(Kind::Comma) {
                break;
            }
        }
        self.take(Kind::Greater)?;
        Ok(generics)
    }
    fn params(&mut self) -> Result<Vec<(String, Ty)>, Issue> {
        self.take(Kind::LParen)?;
        let mut params = Vec::new();
        if self.eat(Kind::RParen) {
            return Ok(params);
        }
        loop {
            params.push(self.field()?);
            if !self.eat(Kind::Comma) {
                break;
            }
        }
        self.take(Kind::RParen)?;
        Ok(params)
    }
    fn declaration(&mut self) -> Result<Decl, Issue> {
        if self.eat(Kind::Struct) {
            let name = self.take(Kind::Name)?;
            self.take(Kind::LBrace)?;
            let mut fields = Vec::new();
            while self.eat(Kind::Newline) {}
            while self.kind() != Some(Kind::RBrace) {
                fields.push(self.field()?);
                if self.kind() != Some(Kind::RBrace) {
                    self.take(Kind::Newline)?;
                }
                while self.eat(Kind::Newline) {}
            }
            self.take(Kind::RBrace)?;
            Ok(Decl::Struct(name, fields))
        } else {
            self.take(Kind::Fn)?;
            let name = self.take(Kind::Name)?;
            let generics = self.generic_params()?;
            let params = self.params()?;
            self.take(Kind::Arrow)?;
            let result = self.ty()?;
            self.take(Kind::FatArrow)?;
            Ok(Decl::Function {
                name,
                generics,
                params,
                result,
                body: self.expr(0)?,
            })
        }
    }
}
pub fn parse(source: &str, tokens: &[Token]) -> Parsed {
    let mut p = Parser {
        source,
        tokens,
        pos: 0,
    };
    let mut result = Parsed::default();
    if p.eat(Kind::Module) {
        let header = (|| {
            p.take(Kind::Name)?;
            while p.eat(Kind::Dot) {
                p.take(Kind::Name)?;
            }
            p.take(Kind::Newline)
        })();
        if let Err(e) = header {
            result.issues.push(e);
        }
    }
    while p.pos < tokens.len() {
        if p.eat(Kind::Newline) {
            continue;
        }
        let start = p.pos;
        match p.declaration().and_then(|d| {
            if p.pos < tokens.len() {
                p.take(Kind::Newline)?;
            }
            Ok(d)
        }) {
            Ok(d) => result.declarations.push(d),
            Err(e) => {
                result.issues.push(e);
                if p.pos == start {
                    p.pos += 1;
                }
                while p.pos < tokens.len() && !matches!(p.kind(), Some(Kind::Fn | Kind::Struct)) {
                    p.pos += 1;
                }
            }
        }
    }
    result
}
