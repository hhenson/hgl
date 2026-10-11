//! Cold sparse key discovery over retained immutable lexical origins.
use hgl_semantics::ir::{DeltaEntry, Kind, Statement, Value};
use hgl_semantics::static_values::StaticValues;
fn collect(value: &Value, keys: &mut Vec<Value>, origins: &StaticValues) {
    hgl_semantics::ir_visit::value(value, &mut |value| {
        if let Kind::Query(op, args) = &value.kind
            && op.starts_with("collection_")
            && matches!(args.first().map(|v| &v.ty), Some(hgl_source::Ty::Map(..)))
            && let Some(key) = args.get(1)
            && let Ok((_, Some(key))) = origins.key(key.clone())
        {
            keys.push(key);
        }
        if let Kind::Delta(parts) = &value.kind {
            for part in parts {
                if let DeltaEntry::Add(k) | DeltaEntry::Remove(k) | DeltaEntry::Keyed(k, _) = part
                    && let Ok((_, Some(key))) = origins.key(k.clone())
                {
                    keys.push(key);
                }
            }
        }
        if let Kind::ValueCall(values, body) = &value.kind {
            let mut scope = StaticValues {
                configuration: origins.configuration.clone(),
                prepared: origins.prepared.clone(),
                ..StaticValues::default()
            };
            for (id, value) in values.iter().enumerate() {
                collect(value, keys, origins);
                scope.bind(id, origins.resolve(value), false);
            }
            statements(body, keys, &mut scope);
            return false;
        }
        !matches!(value.kind, Kind::Captured(..))
    });
}

fn statements(body: &[Statement], keys: &mut Vec<Value>, origins: &mut StaticValues) {
    for statement in body {
        match statement {
            Statement::Let(id, v) | Statement::Var(id, v) => {
                collect(v, keys, origins);
                origins.bind(*id, v, matches!(statement, Statement::Var(..)));
            }
            Statement::Borrow(_, v, _)
            | Statement::Return(v)
            | Statement::Yield(v)
            | Statement::Call(v) => collect(v, keys, origins),
            Statement::TimedYield(a, b) | Statement::Assign(a, b) => {
                collect(a, keys, origins);
                collect(b, keys, origins);
            }
            Statement::While(v, body)
            | Statement::ForItems(_, _, _, v, body)
            | Statement::For(_, v, body) => {
                collect(v, keys, origins);
                statements(body, keys, &mut origins.clone());
            }
            Statement::If(v, a, b) => {
                collect(v, keys, origins);
                statements(a, keys, &mut origins.clone());
                statements(b, keys, &mut origins.clone());
            }
            Statement::Exit => {}
        }
    }
}

/// Collect complete statically known sparse keys while preserving lexical origins.
pub fn node_keys(node: &hgl_semantics::ir::Node) -> Vec<Value> {
    let mut keys = Vec::new();
    let origins = StaticValues {
        configuration: node.configuration.clone(),
        ..StaticValues::default()
    };
    for value in &node.configuration {
        collect(value, &mut keys, &origins);
    }
    statements(&node.start, &mut keys, &mut origins.clone());
    statements(&node.stop, &mut keys, &mut origins.clone());
    if let Some(body) = &node.generator {
        statements(body, &mut keys, &mut origins.clone());
    }
    for (guard, body) in &node.handlers {
        if let Some(guard) = guard {
            collect(guard, &mut keys, &origins);
        }
        statements(body, &mut keys, &mut origins.clone());
    }
    keys
}
