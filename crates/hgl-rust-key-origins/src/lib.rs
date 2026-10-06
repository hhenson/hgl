//! Cold sparse key discovery over retained immutable lexical origins.
use hgl_rust_ir::{DeltaEntry, Kind, Statement, Value};
use hgl_static_values::StaticValues;
fn collect(value: &Value, keys: &mut Vec<Value>, origins: &StaticValues) {
    match &value.kind {
        Kind::Delta(parts) => {
            for part in parts {
                match part {
                    DeltaEntry::Add(k) | DeltaEntry::Remove(k) | DeltaEntry::Keyed(k, _) => {
                        if let Ok((_, Some(key))) = origins.key(k.clone()) {
                            keys.push(key);
                        }
                    }
                    DeltaEntry::Child(..) => {}
                }
                for value in part.operands() {
                    collect(value, keys, origins);
                }
            }
        }
        Kind::List(values) | Kind::Native(_, values) | Kind::Query(_, values) => {
            for value in values {
                collect(value, keys, origins);
            }
        }
        Kind::Construct(values) => {
            for (_, value) in values {
                collect(value, keys, origins);
            }
        }
        Kind::ValueCall(values, body) => {
            for value in values {
                collect(value, keys, origins);
            }
            let mut scope = StaticValues {
                configuration: origins.configuration.clone(),
                prepared: origins.prepared.clone(),
                ..StaticValues::default()
            };
            for (id, value) in values.iter().enumerate() {
                scope.bind(id, origins.resolve(value), false);
            }
            statements(body, keys, &mut scope);
        }
        Kind::Field(v, _)
        | Kind::GlobalSet(_, v)
        | Kind::Length(v)
        | Kind::Unary(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v) => collect(v, keys, origins),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            collect(a, keys, origins);
            collect(b, keys, origins);
        }
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
        | Kind::Captured(..)
        | Kind::Prepared(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::GeneratorLocal(_)
        | Kind::Local(_)
        | Kind::MutableLocal(_)
        | Kind::Output
        | Kind::Capability
        | Kind::Void => {}
    }
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
            Statement::While(v, body) | Statement::For(_, v, body) => {
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
pub fn node_keys(node: &hgl_rust_ir::Node) -> Vec<Value> {
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
