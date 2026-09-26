use crate::lex::{Token, error, lex};
use crate::model::{Body, Expression, ExpressionKind, Function, Guard, Parameter, ParsedStatement};
use crate::{Diagnostic, Source};

pub(crate) struct Parsed {
    pub module: String,
    pub part: Option<String>,
    pub functions: Vec<Function>,
}

pub(crate) fn parse(source: &Source, id: usize) -> Result<Parsed, Diagnostic> {
    Parser {
        source,
        id,
        tokens: lex(source)?,
        cursor: 0,
    }
    .module()
}
struct Parser<'a> {
    source: &'a Source,
    id: usize,
    tokens: Vec<Token>,
    cursor: usize,
}
impl Parser<'_> {
    fn token(&self) -> &Token {
        &self.tokens[self.cursor]
    }
    fn at(&self, text: &str) -> bool {
        self.token().text == text
    }
    fn take(&mut self, text: &str) -> bool {
        if self.at(text) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }
    fn fail(&self, message: &str) -> Diagnostic {
        error(self.source, self.token().span.clone(), message)
    }
    fn need(&mut self, text: &str) -> Result<(), Diagnostic> {
        if self.take(text) {
            Ok(())
        } else {
            Err(self.fail(&format!("expected '{text}'")))
        }
    }
    fn lines(&mut self) {
        while self.take("\n") {}
    }
    fn end(&mut self) -> Result<(), Diagnostic> {
        if !self.at("}") && !self.at("") && !self.at("\n") {
            return Err(self.fail("expected end of statement"));
        }
        self.lines();
        Ok(())
    }
    fn name(&mut self) -> Result<String, Diagnostic> {
        let text = &self.token().text;
        if !text
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_alphabetic() || *b == b'_')
            || matches!(
                text.as_str(),
                "fn" | "const"
                    | "module"
                    | "export"
                    | "return"
                    | "let"
                    | "start"
                    | "stop"
                    | "when"
                    | "true"
                    | "false"
                    | "struct"
            )
        {
            return Err(self.fail("expected a non-reserved name"));
        }
        let name = text.clone();
        self.cursor += 1;
        Ok(name)
    }
    fn module(mut self) -> Result<Parsed, Diagnostic> {
        self.lines();
        self.need("module")?;
        let mut module = self.name()?;
        while self.take(".") {
            module.push('.');
            module.push_str(&self.name()?);
        }
        let part = if self.take("part") {
            Some(self.name()?)
        } else {
            None
        };
        self.end()?;
        let mut functions = Vec::new();
        while !self.at("") {
            functions.push(self.function()?);
            self.end()?;
        }
        Ok(Parsed {
            module,
            part,
            functions,
        })
    }
    fn function(&mut self) -> Result<Function, Diagnostic> {
        let start = self.token().span.start;
        let exported = self.take("export");
        let native = self.take("native");
        if native {
            self.need("const")?;
        }
        self.need("fn")?;
        let name = self.name()?;
        self.need("(")?;
        self.lines();
        let mut parameters = Vec::new();
        while !self.at(")") {
            let constant = self.take("const");
            let name = self.name()?;
            self.need(":")?;
            self.need("i64")?;
            parameters.push(Parameter { name, constant });
            self.lines();
            if !self.take(",") {
                break;
            }
            self.lines();
        }
        self.need(")")?;
        let result = if self.take("->") {
            self.need("i64")?;
            true
        } else {
            false
        };
        let body = if self.take("{") {
            let body = self.body(native)?;
            self.need("}")?;
            Some(body)
        } else {
            None
        };
        if !native && body.is_none() {
            return Err(self.fail("an HGL function requires a body"));
        }
        Ok(Function {
            name,
            source: self.id,
            span: start..self.token().span.start,
            native,
            exported,
            parameters,
            result,
            body,
        })
    }
    fn body(&mut self, native: bool) -> Result<Body, Diagnostic> {
        self.lines();
        let mut body = Body::default();
        if native {
            if !self.at("}") {
                return Err(
                    self.fail("this slice supports empty native value implementation parts only")
                );
            }
            return Ok(body);
        }
        if self.take("inject") {
            self.need("scheduler")?;
            body.scheduler = true;
            self.end()?;
        }
        if self.take("start") {
            self.need("{")?;
            self.lines();
            self.need("scheduler")?;
            self.need(".")?;
            self.need("schedule")?;
            self.need("(")?;
            self.need("0s")?;
            self.need(")")?;
            self.end()?;
            self.need("}")?;
            self.lines();
            body.start = true;
        }
        if self.take("when") {
            body.guard = Some(if self.take("scheduled") {
                self.need("(")?;
                self.need(")")?;
                Guard::Scheduled
            } else {
                Guard::Modified
            });
            self.need("{")?;
            body.statements = self.statements()?;
            self.need("}")?;
            self.lines();
        } else {
            body.statements = self.statements()?;
        }
        Ok(body)
    }
    fn statements(&mut self) -> Result<Vec<ParsedStatement>, Diagnostic> {
        self.lines();
        let mut result = Vec::new();
        while !self.at("}") {
            let statement = if self.take("let") {
                let name = self.name()?;
                self.need("=")?;
                ParsedStatement::Let(name, self.expression()?)
            } else if self.take("return") {
                ParsedStatement::Return(self.expression()?)
            } else {
                ParsedStatement::Call(self.expression()?)
            };
            result.push(statement);
            self.end()?;
        }
        Ok(result)
    }
    fn expression(&mut self) -> Result<Expression, Diagnostic> {
        let start = self.token().span.start;
        let negative = self.take("-");
        let kind = if self.token().text.bytes().all(|b| b.is_ascii_digit()) && !self.at("") {
            let literal = format!("{}{}", if negative { "-" } else { "" }, self.token().text);
            let value = literal
                .parse()
                .map_err(|error| self.fail(&format!("invalid i64 integer: {error}")))?;
            self.cursor += 1;
            ExpressionKind::Integer(value)
        } else {
            if negative {
                return Err(self.fail("expected an integer after '-'"));
            }
            let name = self.name()?;
            if self.take("(") {
                let arguments = self.arguments()?;
                ExpressionKind::Call(name, arguments)
            } else {
                ExpressionKind::Name(name)
            }
        };
        Ok(Expression {
            span: start..self.token().span.start,
            kind,
        })
    }
    fn arguments(&mut self) -> Result<Vec<Expression>, Diagnostic> {
        let mut arguments = Vec::new();
        self.lines();
        while !self.at(")") {
            arguments.push(self.expression()?);
            self.lines();
            if !self.take(",") {
                break;
            }
            self.lines();
        }
        self.need(")")?;
        Ok(arguments)
    }
}
