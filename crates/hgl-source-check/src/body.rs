use crate::{annotation, inferred};
use hgl_library::Library;
use hgl_source::{Cursor, Issue, Ty};
use std::collections::BTreeMap;
pub(super) struct Checker<'a> {
    pub library: &'a Library,
    pub module: &'a str,
}
impl Checker<'_> {
    pub(super) fn block(
        &self,
        cursor: &mut Cursor<'_>,
        environment: &mut BTreeMap<String, Ty>,
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
    fn statement(
        &self,
        cursor: &mut Cursor<'_>,
        environment: &mut BTreeMap<String, Ty>,
    ) -> Result<(), Issue> {
        if cursor.take("yield") {
            let span = cursor.span();
            let expression = cursor.expr()?;
            let ty = inferred(self.library, self.module, &expression, environment).map_err(
                |message| {
                    let mut issue = Issue::from(message).at(span.clone());
                    issue.category = "type";
                    issue
                },
            )?;
            if let Some(ty) = ty {
                crate::yield_time(&ty, span)?;
            }
            cursor.need(":")?;
            cursor.expr()?;
        } else if matches!(cursor.peek(), "let" | "var" | "state" | "cache") {
            self.binding(cursor, environment)?;
        } else if cursor.take("inject") {
            cursor.lines();
            loop {
                cursor.name()?;
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
                cursor.expr()?;
            }
            self.block(cursor, &mut environment.clone())?;
        } else if cursor.take("if") {
            cursor.expr()?;
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
            cursor.name()?;
            while cursor.take(",") {
                cursor.name()?;
            }
            cursor.need("in")?;
            cursor.expr()?;
            self.block(cursor, &mut environment.clone())?;
        } else if cursor.take("return") {
            if !cursor.at("}") && !cursor.at("\n") {
                cursor.expr()?;
            }
        } else {
            cursor.expr()?;
            if cursor.take("=") || cursor.take("+=") {
                cursor.expr()?;
            }
        }
        Ok(())
    }
    fn binding(
        &self,
        cursor: &mut Cursor<'_>,
        environment: &mut BTreeMap<String, Ty>,
    ) -> Result<(), Issue> {
        cursor.consume()?;
        let name = cursor.name()?;
        let declared = if cursor.take(":") {
            let start = cursor.pos;
            let spelling = cursor.type_name()?;
            annotation(&cursor.tokens[start..cursor.pos])?;
            Ty::parse(&spelling)
        } else {
            None
        };
        cursor.need("=")?;
        let expression = cursor.expr()?;
        let ty = declared.or(inferred(
            self.library,
            self.module,
            &expression,
            environment,
        )?);
        if let Some(ty) = ty {
            environment.insert(name, ty);
        }
        Ok(())
    }
}
