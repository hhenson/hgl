//! Source runtime provenance across lexical Tuple constant boundaries.
use crate::name_check::Scope;
use hgl_source::{Expr, Issue};
use std::collections::{BTreeMap, BTreeSet};
/// Source facts keep lexical shadowing distinct from writes to an outer binding.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// Bindings whose payload currently depends on execution.
    pub runtime: BTreeSet<String>,
    entries: BTreeMap<String, (usize, bool)>,
    next: usize,
}
impl Facts {
    /// Seed formal runtime inputs before checking a source body.
    pub fn new(runtime: BTreeSet<String>) -> Self {
        Self {
            runtime,
            ..Self::default()
        }
    }
    fn record(&mut self, name: &str, expression: &Expr, persistent: bool) {
        if persistent || crate::tuple_phase::depends(expression, &self.runtime) {
            self.runtime.insert(name.into());
        } else {
            self.runtime.remove(name);
        }
    }
    /// Declare a fresh lexical identity and its initializer dependence.
    pub fn bind(&mut self, name: &str, expression: &Expr, persistent: bool) {
        self.record(name, expression, persistent);
        self.entries.insert(name.into(), (self.next, persistent));
        self.next += 1;
    }
    /// Update an existing binding without changing its lexical identity.
    pub fn assign(&mut self, target: &Expr, expression: &Expr) {
        if let Expr::Name(name) = target.syntax() {
            let persistent = self
                .entries
                .get(name)
                .is_some_and(|(_, persistent)| *persistent);
            self.record(name, expression, persistent);
        }
    }
    fn merge(&mut self, other: &Self) {
        for name in &other.runtime {
            if self
                .entries
                .get(name)
                .is_some_and(|entry| other.entries.get(name) == Some(entry))
            {
                self.runtime.insert(name.clone());
            }
        }
        self.next = self.next.max(other.next);
    }
}
/// Preserve writes to outer bindings while excluding inner declarations.
pub fn nested(
    scope: &mut Scope,
    check: impl FnOnce(&mut Scope) -> Result<(), Issue>,
) -> Result<(), Issue> {
    let mut child = scope.clone();
    check(&mut child)?;
    scope.tuple.merge(&child.tuple);
    Ok(())
}
