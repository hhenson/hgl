use crate::library::Library;
use crate::name_check::Scope;
use crate::source_check::{annotation, inferred};
use crate::tuple_writes::{assignment, conditional, handler};
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
        let value = cursor.expr()?;
        let selected = self.test_only.then_some(self.module);
        crate::name_check::expression(self.library, self.module, selected, &value, environment)
            .map_err(|issue| issue.at(span))?;
        Ok(value)
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
    fn scoped(&self, cursor: &mut Cursor<'_>, scope: &mut Scope) -> Result<(), Issue> {
        crate::tuple_flow::nested(scope, |child| self.block(cursor, child))
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
                environment.inject(cursor.name()?);
                if !cursor.take(",") {
                    break;
                }
                cursor.lines();
            }
        } else if cursor.at("start") || cursor.at("stop") {
            cursor.consume()?;
            self.scoped(cursor, environment)?;
        } else if matches!(cursor.peek(), "when" | "while") {
            handler(
                cursor,
                environment,
                |c, s| self.expression(c, s),
                |c, s| self.block(c, s),
            )?;
        } else if cursor.take("if") {
            let selector = self.expression(cursor, environment)?;
            conditional(
                cursor,
                environment,
                &selector,
                |c, s| self.block(c, s),
                |c, s| self.statement(c, s),
            )?;
        } else if cursor.take("for") {
            let mut body = environment.clone();
            body.iteration(&cursor.name()?);
            while cursor.take(",") {
                body.iteration(&cursor.name()?);
            }
            cursor.need("in")?;
            let collection = self.expression(cursor, environment)?;
            body.tuple
                .writes
                .select(&collection, &environment.tuple.runtime);
            self.block(cursor, &mut body)?;
            environment.tuple.merge(&body.tuple);
        } else if cursor.take("return") {
            if !cursor.at("}") && !cursor.at("\n") {
                self.expression(cursor, environment)?;
            }
        } else {
            let target = self.expression(cursor, environment)?;
            assignment(cursor, environment, &target, |c, s| self.expression(c, s))?;
        }
        Ok(())
    }
    fn binding(&self, cursor: &mut Cursor<'_>, environment: &mut Scope) -> Result<(), Issue> {
        let retained = matches!(cursor.consume()?.as_str(), "state" | "cache");
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
        environment.alias(&name, &expression, retained);
        let ty = declared.or(inferred(
            self.library,
            self.module,
            &expression,
            &environment.types,
        )?);
        if let Some(ty) = ty {
            environment.types.insert(name, ty);
        }
        Ok(())
    }
}
