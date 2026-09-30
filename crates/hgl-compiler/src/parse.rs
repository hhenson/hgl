use crate::lex::{Token, error, lex};
use crate::model::{Body, Expression, ExpressionKind, Function, Guard, Parameter, ParsedStatement};
use crate::{Diagnostic, Source};

pub(crate) struct Parsed {
    pub documentation: Vec<crate::Documentation>,
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
        documentation: Vec::new(),
    }
    .module()
}
struct Parser<'a> {
    source: &'a Source,
    id: usize,
    tokens: Vec<Token>,
    cursor: usize,
    documentation: Vec<crate::Documentation>,
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
        let doc = self.doc()?;
        let start = self.token().span.start;
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
        self.save_doc(doc, start, &module, &[])?;
        self.end()?;
        let mut functions = Vec::new();
        while !self.at("") {
            functions.push(self.function()?);
            self.end()?;
        }
        for doc in &mut self.documentation {
            doc.part = part.clone().unwrap_or_default();
            if !doc.declaration.starts_with("module ") {
                doc.name = format!("{module}.{}", doc.name);
            }
        }
        Ok(Parsed {
            documentation: self.documentation,
            module,
            part,
            functions,
        })
    }
    fn doc(&mut self) -> Result<Option<Token>, Diagnostic> {
        if !self.token().text.starts_with("/**") {
            return Ok(None);
        }
        let doc = Token {
            text: self.token().text.clone(),
            span: self.token().span.clone(),
        };
        self.cursor += 1;
        self.lines();
        if !self.source.text[doc.span.end..self.token().span.start]
            .trim()
            .is_empty()
        {
            return Err(self.fail("documentation must immediately precede a declaration"));
        }
        Ok(Some(doc))
    }
    fn save_doc(
        &mut self,
        doc: Option<Token>,
        start: usize,
        name: &str,
        parameters: &[Parameter],
    ) -> Result<(), Diagnostic> {
        if let Some(doc) = doc {
            let text = crate::documentation::normalize(&doc.text);
            crate::documentation::validate(
                &text,
                &parameters
                    .iter()
                    .map(|p| p.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .map_err(|message| error(self.source, doc.span.clone(), &message))?;
            self.documentation.push(crate::Documentation {
                name: name.into(),
                declaration: self.source.text[start..self.token().span.start]
                    .trim()
                    .replace("\r\n", "\n")
                    .replace('\r', "\n"),
                text,
                part: String::new(),
                source: self.source.name.clone(),
                start: doc.span.start,
                end: doc.span.end,
            });
        }
        Ok(())
    }
    fn function(&mut self) -> Result<Function, Diagnostic> {
        let doc = self.doc()?;
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
        self.save_doc(doc, start, &name, &parameters)?;
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
            self.need("schedule")?;
            self.need("(")?;
            self.need("scheduler")?;
            self.need(",")?;
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
