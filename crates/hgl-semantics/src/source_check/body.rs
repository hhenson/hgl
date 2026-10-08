use crate::library::Library;
use crate::name_check::Scope;
use crate::source_check::{annotation, inferred};
use hgl_source::{Cursor, Issue, Ty};
pub(super) struct Checker<'a> {
    pub library: &'a Library,
    pub module: &'a str,
    pub test_only: bool,
}
impl Checker<'_> {
    pub(super) fn expression(
        &self,
        cursor: &mut Cursor<'_>,
        environment: &Scope,
    ) -> Result<hgl_source::Expr, Issue> {
        let span = cursor.span();
        let expression = cursor.expr()?;
        crate::name_check::expression(
            self.library,
            self.module,
            self.test_only.then_some(self.module),
            &expression,
            environment,
        )
        .map_err(|issue| issue.at(span))?;
        Ok(expression)
    }

    pub(super) fn block(
        &self,
        cursor: &mut Cursor<'_>,
        environment: &mut Scope,
    ) -> Result<(), Issue> {
        cursor.need("{")?;
        cursor.lines();
        while !cursor.take("}") {
            let start = cursor.span();
            self.statement(cursor, environment)
                .map_err(|issue| issue.at(start))?;
            cursor.lines();
        }
        Ok(())
    }
    fn statement(&self, cursor: &mut Cursor<'_>, environment: &mut Scope) -> Result<(), Issue> {
        if cursor.take("yield") {
            let span = cursor.span();
            let expression = self.expression(cursor, environment)?;
            let ty = inferred(self.library, self.module, &expression, &environment.types).map_err(
                |message| {
                    let mut issue = Issue::from(message).at(span.clone());
                    issue.category = "type";
                    issue
                },
            )?;
            if let Some(ty) = ty {
                crate::source_check::yield_time(&ty, span)?;
            }
            cursor.need(":")?;
            self.expression(cursor, environment)?;
        } else if matches!(cursor.peek(), "let" | "var" | "state" | "cache") {
            self.binding(cursor, environment)?;
        } else if cursor.take("inject") {
            cursor.lines();
            loop {
                environment.services.insert(cursor.name()?);
                if !cursor.take(",") {
                    break;
                }
                cursor.lines();
            }
        } else if cursor.at("start") || cursor.at("stop") {
            cursor.consume()?;
            self.block(cursor, &mut environment.clone())?;
        } else if cursor.take("when") || cursor.take("while") {
            if !cursor.at("{") {
                self.expression(cursor, environment)?;
            }
            self.block(cursor, &mut environment.clone())?;
        } else if cursor.take("if") {
            self.expression(cursor, environment)?;
            self.block(cursor, &mut environment.clone())?;
            cursor.lines();
            if cursor.take("else") {
                if cursor.at("if") {
                    self.statement(cursor, &mut environment.clone())?;
                } else {
                    self.block(cursor, &mut environment.clone())?;
                }
            }
        } else if cursor.take("for") {
            let mut body = environment.clone();
            body.services.remove(&cursor.name()?);
            while cursor.take(",") {
                body.services.remove(&cursor.name()?);
            }
            cursor.need("in")?;
            self.expression(cursor, environment)?;
            self.block(cursor, &mut body)?;
        } else if cursor.take("return") {
            if !cursor.at("}") && !cursor.at("\n") {
                self.expression(cursor, environment)?;
            }
        } else {
            self.expression(cursor, environment)?;
            if cursor.take("=") || cursor.take("+=") {
                self.expression(cursor, environment)?;
            }
        }
        Ok(())
    }
    fn binding(&self, cursor: &mut Cursor<'_>, environment: &mut Scope) -> Result<(), Issue> {
        cursor.consume()?;
        let name = cursor.name()?;
        let declared = if cursor.take(":") {
            let start = cursor.pos;
            let spelling = cursor.type_name()?;
            annotation(&cursor.tokens[start..cursor.pos])?;
            crate::name_check::type_name(
                self.library,
                self.module,
                &spelling,
                environment,
                self.test_only.then_some(self.module),
            )?;
            Ty::parse(&spelling)
        } else {
            None
        };
        cursor.need("=")?;
        let expression = self.expression(cursor, environment)?;
        let ty = declared.or(inferred(
            self.library,
            self.module,
            &expression,
            &environment.types,
        )?);
        if matches!(expression.syntax(), hgl_source::Expr::Name(source) if environment.services.contains(source))
        {
            environment.services.insert(name.clone());
        } else {
            environment.services.remove(&name);
        }
        if let Some(ty) = ty {
            environment.types.insert(name, ty);
        }
        Ok(())
    }
}
