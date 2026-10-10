//! Source runtime provenance across lexical Tuple constant boundaries.
use crate::name_check::Scope;
use hgl_source::{Expr, Issue};
use std::collections::{BTreeMap, BTreeSet};
/// Source facts keep lexical shadowing distinct from writes to an outer binding.
#[derive(Debug, Clone, Default)]
pub struct Facts {
    /// Bindings whose payload currently depends on execution.
    pub runtime: BTreeSet<String>,
    /// Selection dependence is kept separate from binding initialization.
    pub writes: crate::tuple_writes::Writes,
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
    pub fn assign(&mut self, target: &Expr, expression: &Expr, compound: bool) {
        if let Some((name, _)) = crate::tuple_writes::root(target) {
            let persistent = self.entries.get(name).is_some_and(|entry| entry.1);
            let controlled = self.writes.preserve(target, &self.runtime, compound);
            self.record(name, expression, persistent || controlled);
        }
    }
    /// Admitted temporal iteration bindings shadow outer names and carry runtime payloads.
    pub fn iteration(&mut self, name: &str) {
        self.entries.remove(name);
        self.runtime.insert(name.into());
    }
    /// Merge writes only when the nested scope retains the same outer identity.
    pub fn merge(&mut self, other: &Self) {
        for name in &other.runtime {
            if let Some(entry) = self.entries.get(name)
                && other.entries.get(name) == Some(entry)
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
