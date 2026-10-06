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
    /// Immutable key aliases, excluded from general configuration/global-key lookup.
    pub cold_locals: BTreeMap<usize, Value>,
}
impl StaticValues {
    /// Follow known static origins without evaluating their expressions.
    pub fn resolve<'a>(&'a self, value: &'a Value) -> &'a Value {
        if let Kind::Prepared(id) = value.kind {
            return self
                .prepared
                .get(id)
                .map_or(value, |origin| self.resolve(origin));
        }
        if let Kind::Configuration(id) = value.kind {
            return self
                .configuration
                .get(id)
                .map_or(value, |origin| self.resolve(origin));
        }
        if let Kind::Local(id) = value.kind {
            return self.locals.get(&id).map_or(value, |v| self.resolve(v));
        }
        value
    }
    fn key_origin<'a>(&'a self, value: &'a Value) -> &'a Value {
        if let Kind::Local(id) = value.kind
            && let Some(origin) = self.cold_locals.get(&id)
        {
            return self.key_origin(origin);
        }
        self.resolve(value)
    }
    /// Retain immutable origins while excluding mutable and runtime expression aliases.
    pub fn bind(&mut self, id: usize, value: &Value, mutable: bool) {
        let origin = self.key_origin(value).clone();
        self.cold_locals.remove(&id);
        if !mutable && self.eligible(&origin) {
            self.cold_locals.insert(id, origin);
        }
    }
    fn eligible(&self, value: &Value) -> bool {
        let origin = self.key_origin(value);
        if let Kind::Construct(fields) = &origin.kind {
            return fields.iter().all(|(_, child)| self.eligible(child));
        }
        matches!(origin.kind, Kind::TemporalLiteral(_)) || hgl_value_constant::context_free(origin)
    }
    fn constant(&self, value: &Value) -> Result<Value, String> {
        let origin = self.key_origin(value);
        if let Kind::Construct(fields) = &origin.kind {
            let fields = fields
                .iter()
                .map(|(i, child)| Ok((*i, self.constant(child)?)))
                .collect::<Result<Vec<_>, String>>()?;
            return Ok(Value::new(origin.ty.clone(), Kind::Construct(fields)));
        }
        if matches!(origin.kind, Kind::TemporalLiteral(_)) {
            return Ok(origin.clone());
        }
        if !hgl_value_constant::context_free(origin) {
            return Err("delta position requires a constant value".into());
        }
        hgl_value_eval::Evaluator::default()
            .value(origin)
            .map_err(|e| format!("delta position requires a constant: {e}"))
    }
    /// Prove key eligibility while retaining local operands and contextual recipes.
    pub fn key(&self, value: Value) -> Result<(Value, Option<Value>), String> {
        let known = self.constant(&value)?;
        let identity = hgl_composite_keys::known(&known)?;
        let retained =
            if identity.is_none() || matches!(value.kind, Kind::Local(_) | Kind::Construct(_)) {
                value
            } else {
                known.clone()
            };
        Ok((retained, identity.map(|_| known)))
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
