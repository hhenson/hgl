use hgl_rust_ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
type Aliases = BTreeMap<usize, Value>;
fn retained(value: &Value) -> bool {
    matches!(
        value.ty,
        Ty::Str | Ty::TimeZone | Ty::ZonedTime | Ty::ZonedDateTime
    )
}
/// Retain literal owning scalars once and reuse immutable configuration aliases.
/// No provider, ordinary call, mutable local or hook is executed during this pass.
pub fn prepare(plan: &Plan) -> Plan {
    let mut plan = plan.clone();
    for node in &mut plan.nodes {
        statements(
            &mut node.start,
            &mut node.configuration,
            &mut Aliases::new(),
        );
        statements(&mut node.stop, &mut node.configuration, &mut Aliases::new());
        for (guard, body) in &mut node.handlers {
            if let Some(guard) = guard {
                rewrite(guard, &mut node.configuration, &Aliases::new());
            }
            statements(body, &mut node.configuration, &mut Aliases::new());
        }
    }
    plan
}
fn statements(body: &mut Vec<Statement>, config: &mut Vec<Value>, aliases: &mut Aliases) {
    body.retain_mut(|statement| {
        match statement {
            Statement::Let(id, value) => {
                rewrite(value, config, aliases);
                retain(value, config);
                if retained(value) && matches!(value.kind, Kind::Configuration(_)) {
                    aliases.insert(*id, value.clone());
                    return false;
                }
            }
            Statement::Return(v)
            | Statement::Yield(v)
            | Statement::Call(v)
            | Statement::Var(_, v)
            | Statement::Borrow(_, v, _) => rewrite(v, config, aliases),
            Statement::TimedYield(a, b) | Statement::Assign(a, b) => {
                rewrite(a, config, aliases);
                rewrite(b, config, aliases);
            }
            Statement::While(v, body) | Statement::For(_, v, body) => {
                rewrite(v, config, aliases);
                statements(body, config, &mut aliases.clone());
            }
            Statement::If(v, a, b) => {
                rewrite(v, config, aliases);
                statements(a, config, &mut aliases.clone());
                statements(b, config, &mut aliases.clone());
            }
            Statement::Exit => {}
        }
        true
    });
}
fn rewrite(value: &mut Value, config: &mut Vec<Value>, aliases: &Aliases) {
    if let Kind::Local(id) = value.kind
        && let Some(origin) = aliases.get(&id)
    {
        *value = origin.clone();
        return;
    }
    match &mut value.kind {
        Kind::Delta(parts) => {
            for part in parts {
                for value in part.operands_mut() {
                    rewrite(value, config, aliases);
                    retain(value, config);
                }
            }
        }
        Kind::List(items)
        | Kind::Query(_, items)
        | Kind::Native(_, items)
        | Kind::ValueCall(items, _) => {
            for value in items {
                rewrite(value, config, aliases);
            }
        }
        Kind::Construct(fields) => {
            for (_, value) in fields {
                rewrite(value, config, aliases);
            }
        }
        Kind::Field(v, _)
        | Kind::Length(v)
        | Kind::GlobalSet(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v)
        | Kind::Unary(_, v) => rewrite(v, config, aliases),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            rewrite(a, config, aliases);
            rewrite(b, config, aliases);
        }
        Kind::ObservedLocal(_)
        | Kind::WiringFailure(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::Literal(_)
        | Kind::TemporalLiteral(_)
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

fn retain(value: &mut Value, config: &mut Vec<Value>) {
    if retained(value) && matches!(value.kind, Kind::Literal(_)) {
        let id = config.len();
        config.push(value.clone());
        value.kind = Kind::Configuration(id);
    }
}
