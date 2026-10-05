//! Checking-only provenance shared by lexical test preparation and graph selection.
use hgl_rust_ir::{Kind, Value};
use std::collections::{BTreeMap, BTreeSet};
/// Compiler-only provenance for values known before graph topology selection.
#[derive(Debug, Default, Clone)]
pub struct StaticValues {
    /// Original checked expressions behind cold eval argument bindings.
    pub prepared: Vec<Value>,
    /// Current node's readonly ordinary configurations.
    pub configuration: Vec<Value>,
    /// Static ordinary constant parameter metadata in the current lexical scope.
    pub locals: BTreeMap<usize, Value>,
}
impl StaticValues {
    /// Follow known static origins without evaluating their expressions.
    pub fn resolve<'a>(&'a self, value: &'a Value) -> &'a Value {
        if let Kind::Prepared(id) = value.kind {
            return self.resolve(&self.prepared[id]);
        }
        if let Kind::Configuration(id) = value.kind {
            return self.resolve(&self.configuration[id]);
        }
        if let Kind::Local(id) = value.kind {
            return self.locals.get(&id).map_or(value, |v| self.resolve(v));
        }
        value
    }
    /// Retain immutable origins while excluding mutable and runtime expression aliases.
    pub fn bind(&mut self, id: usize, value: &Value, mutable: bool) {
        let origin = self.resolve(value).clone();
        self.locals.remove(&id);
        if !mutable
            && (matches!(origin.kind, Kind::TemporalLiteral(_))
                || hgl_value_constant::context_free(&origin))
        {
            self.locals.insert(id, origin);
        }
    }
    /// Prove key eligibility without replacing a retained local or resolving a provider.
    pub fn key(&self, value: Value) -> Result<(Value, Option<hgl_source::Literal>), String> {
        let origin = self.resolve(&value);
        if matches!(origin.kind, Kind::TemporalLiteral(_)) {
            return Ok((value, None));
        }
        if !hgl_value_constant::context_free(origin) {
            return Err("delta position requires a constant scalar".into());
        }
        let known = hgl_value_eval::Evaluator::default()
            .value(origin)
            .map_err(|e| format!("delta position requires a constant: {e}"))?;
        let Kind::Literal(literal) = known.kind else {
            return Err("delta position requires a constant scalar".into());
        };
        let retained = if matches!(value.kind, Kind::Local(_)) {
            value
        } else {
            Value::new(value.ty, Kind::Literal(literal.clone()))
        };
        Ok((retained, Some(literal)))
    }
    /// Map checked constant parameter origins into the source-order local slots.
    pub fn arguments(
        &self,
        signature: &hgl_library::Signature,
        args: &[Value],
        positions: &[usize],
    ) -> BTreeMap<usize, Value> {
        signature
            .parameters
            .iter()
            .zip(positions)
            .filter(|(p, _)| p.constant)
            .filter_map(|(_, id)| {
                let value = self.resolve(&args[*id]);
                (!matches!(value.kind, Kind::Local(_) | Kind::MutableLocal(_)))
                    .then(|| (*id, value.clone()))
            })
            .collect()
    }
}

/// Persistent checked lexical scope; no initializer is executed by this metadata.
#[derive(Debug, Default, Clone)]
pub struct PreparedLexicalScope {
    /// Visible checked binding identities and access permissions.
    pub bindings: BTreeMap<String, Value>,
    /// Next globally fresh lexical identity within this test.
    pub next: usize,
    /// Origins used only to establish cold eligibility and known identities.
    pub origins: StaticValues,
    /// Presence facts about immutable bindings in the current branch.
    pub facts: BTreeSet<(String, usize)>,
}
