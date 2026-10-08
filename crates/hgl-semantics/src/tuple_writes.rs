//! Source selection and target-read dependence for ordinary Tuple constant boundaries.
use crate::name_check::Scope;
use hgl_source::{Cursor, Expr, Issue};
use std::collections::BTreeSet;
/// Control affects updates to existing values, independently of fresh declarations.
#[derive(Debug, Clone, Default)]
pub struct Writes {
    controlled: bool,
}
/// Recover a writable root without treating a member replacement as a whole replacement.
pub fn root(target: &Expr) -> Option<(&str, bool)> {
    if let Expr::Name(name) = target.syntax() {
        Some((name, false))
    } else if let Expr::Property(parent, _) | Expr::Index(parent, _) = target.syntax() {
        root(parent).map(|(name, _)| (name, true))
    } else {
        None
    }
}
impl Writes {
    /// Record selection dependence while retaining any enclosing runtime control.
    pub fn select(&mut self, condition: &Expr, runtime: &BTreeSet<String>) {
        self.controlled |= crate::tuple_phase::depends(condition, runtime);
    }
    /// Compound/member writes read the prior target; runtime indexes select the updated child.
    pub fn preserve(&self, target: &Expr, runtime: &BTreeSet<String>, compound: bool) -> bool {
        self.controlled
            || root(target).is_some_and(|(name, partial)| {
                (compound && runtime.contains(name))
                    || (partial && crate::tuple_phase::depends(target, runtime))
            })
    }
}
/// Check selected writes in a lexical child without tainting new constant declarations.
pub fn branch(
    scope: &mut Scope,
    condition: &Expr,
    check: impl FnOnce(&mut Scope) -> Result<(), Issue>,
) -> Result<(), Issue> {
    crate::tuple_flow::nested(scope, |child| {
        child.tuple.writes.select(condition, &child.tuple.runtime);
        check(child)
    })
}
/// Both alternatives inherit selection dependence, including an enclosing else-if.
pub fn conditional(
    cursor: &mut Cursor<'_>,
    scope: &mut Scope,
    condition: &Expr,
    mut body: impl FnMut(&mut Cursor<'_>, &mut Scope) -> Result<(), Issue>,
    statement: impl FnOnce(&mut Cursor<'_>, &mut Scope) -> Result<(), Issue>,
) -> Result<(), Issue> {
    branch(scope, condition, |child| body(cursor, child))?;
    cursor.lines();
    if cursor.take("else") {
        branch(scope, condition, |child| {
            if cursor.at("if") {
                statement(cursor, child)
            } else {
                body(cursor, child)
            }
        })?;
    }
    Ok(())
}
/// Preserve selector checking and the existing distinction between handlers and loops.
pub fn handler(
    cursor: &mut Cursor<'_>,
    scope: &mut Scope,
    expression: impl FnOnce(&mut Cursor<'_>, &Scope) -> Result<Expr, Issue>,
    body: impl FnOnce(&mut Cursor<'_>, &mut Scope) -> Result<(), Issue>,
) -> Result<(), Issue> {
    let looping = cursor.consume()? == "while";
    let condition = if cursor.at("{") {
        None
    } else {
        Some(expression(cursor, scope)?)
    };
    if let Some(condition) = condition.filter(|_| looping) {
        branch(scope, &condition, |child| body(cursor, child))
    } else {
        crate::tuple_flow::nested(scope, |child| body(cursor, child))
    }
}
/// Check ordinary operands before transferring replacement or compound-write provenance.
pub fn assignment(
    cursor: &mut Cursor<'_>,
    scope: &mut Scope,
    target: &Expr,
    expression: impl FnOnce(&mut Cursor<'_>, &Scope) -> Result<Expr, Issue>,
) -> Result<(), Issue> {
    let compound = cursor.at("+=");
    if cursor.take("=") || cursor.take("+=") {
        let value = expression(cursor, scope)?;
        scope.tuple.assign(target, &value, compound);
    }
    Ok(())
}
