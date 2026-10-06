use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
/// Hoist literal-only family publications into independent cold configuration slots.
/// Calls and provider recipes retain their original evaluation phase and ordering.
pub fn prepare(plan: &Plan) -> Plan {
    let mut plan = plan.clone();
    for node in &mut plan.nodes {
        statements(&mut node.start, &mut node.configuration);
        statements(&mut node.stop, &mut node.configuration);
        for (_, body) in &mut node.handlers {
            statements(body, &mut node.configuration);
        }
        if let Some(body) = &mut node.generator {
            statements(body, &mut node.configuration);
        }
    }
    plan
}
fn statements(body: &mut [Statement], configuration: &mut Vec<Value>) {
    for statement in body {
        match statement {
            Statement::Return(value)
            | Statement::Yield(value)
            | Statement::TimedYield(_, value) => {
                if family(&value.ty) && literal(value) {
                    let id = configuration.len();
                    configuration.push(value.clone());
                    value.kind = Kind::Configuration(id);
                }
            }
            Statement::If(_, yes, no) => {
                statements(yes, configuration);
                statements(no, configuration);
            }
            Statement::While(_, body) | Statement::For(_, _, body) => {
                statements(body, configuration);
            }
            Statement::Let(..)
            | Statement::Var(..)
            | Statement::Borrow(..)
            | Statement::Assign(..)
            | Statement::Call(_)
            | Statement::Exit => {}
        }
    }
}
fn literal(value: &Value) -> bool {
    if matches!(value.kind, Kind::Literal(_)) {
        return true;
    }
    if let Kind::Construct(fields) = &value.kind {
        return fields.iter().all(|(_, value)| literal(value));
    }
    if let Kind::List(values) = &value.kind {
        return values.iter().all(literal);
    }
    if let Kind::Unary(op, value) = &value.kind {
        return op == "family" && literal(value);
    }
    false
}
fn family(ty: &Ty) -> bool {
    if matches!(ty, Ty::Family(_)) {
        return true;
    }
    if let Ty::List(child, _) = ty {
        return family(child);
    }
    if let Ty::Struct(_, fields, _) = ty {
        return fields.iter().any(|(_, ty)| family(ty));
    }
    if let Ty::Tuple(children) = ty {
        return children.iter().any(family);
    }
    false
}
