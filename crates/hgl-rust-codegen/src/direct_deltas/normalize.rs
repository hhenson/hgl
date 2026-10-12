use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
use std::collections::BTreeMap;
type Aliases = BTreeMap<usize, Value>;
fn retained(value: &Value) -> bool {
    value.ty.owning_scalar() && value.ty != Ty::Any
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
        if let Some(body) = &mut node.generator {
            generator_payloads(body, &mut node.configuration);
        }
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
            Statement::While(v, body)
            | Statement::ForItems(_, _, _, v, body)
            | Statement::For(_, v, body) => {
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
    hgl_semantics::ir_visit::value_mut(value, &mut |value| {
        if let Kind::Unary(op, child) = &mut value.kind
            && op == "any"
        {
            retain_tree(child, config);
        }
        if let Kind::Push(_, child) = &mut value.kind {
            retain_tree(child, config);
        }
        if let Kind::Local(id) = value.kind
            && let Some(origin) = aliases.get(&id)
        {
            *value = origin.clone();
            return false;
        }
        if let Kind::Delta(parts) = &mut value.kind {
            for part in parts {
                for value in part.operands_mut() {
                    rewrite(value, config, aliases);
                    retain(value, config);
                }
            }
            return false;
        }
        if let Kind::ValueCall(args, _) = &mut value.kind {
            for argument in args {
                rewrite(argument, config, aliases);
            }
            return false;
        }
        !matches!(value.kind, Kind::Captured(..))
    });
}

fn retain_tree(value: &mut Value, config: &mut Vec<Value>) {
    hgl_semantics::ir_visit::value_mut(value, &mut |v| {
        retain(v, config);
        !matches!(v.kind, Kind::Captured(..) | Kind::ValueCall(..))
    });
}

fn retain(value: &mut Value, config: &mut Vec<Value>) {
    if retained(value) && matches!(value.kind, Kind::Literal(_)) {
        let id = config.len();
        config.push(value.clone());
        value.kind = Kind::Configuration(id);
    }
}

fn generator_payloads(body: &mut [Statement], config: &mut Vec<Value>) {
    hgl_semantics::ir_visit::nodes_mut(body, &mut |statement| {
        if let Statement::Yield(value) | Statement::TimedYield(_, value) = statement
            && value.closed()
        {
            let id = config.len();
            config.push(value.clone());
            value.kind = Kind::Configuration(id);
        }
    });
}
