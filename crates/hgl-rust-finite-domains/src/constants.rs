use hgl_rust_ir::{Kind, Plan, Statement, Value};
/// Visit checked closed hook data without evaluating dynamic expressions.
pub fn collect(plan: &Plan) -> Vec<&Value> {
    let mut result = Vec::new();
    for node in &plan.nodes {
        statements(&node.start, &mut result);
        statements(&node.stop, &mut result);
        if let Some(body) = &node.generator {
            statements(body, &mut result);
        }
        for (guard, body) in &node.handlers {
            if let Some(guard) = guard {
                value(guard, &mut result);
            }
            statements(body, &mut result);
        }
    }
    result
}
fn statements<'a>(body: &'a [Statement], out: &mut Vec<&'a Value>) {
    for statement in body {
        match statement {
            Statement::Let(_, v)
            | Statement::Var(_, v)
            | Statement::Borrow(_, v, _)
            | Statement::Return(v)
            | Statement::Yield(v)
            | Statement::Call(v) => value(v, out),
            Statement::TimedYield(a, b) | Statement::Assign(a, b) => {
                value(a, out);
                value(b, out);
            }
            Statement::While(v, body) | Statement::For(_, v, body) => {
                value(v, out);
                statements(body, out);
            }
            Statement::If(v, a, b) => {
                value(v, out);
                statements(a, out);
                statements(b, out);
            }
            Statement::Exit => {}
        }
    }
}
fn value<'a>(v: &'a Value, out: &mut Vec<&'a Value>) {
    if v.closed() && v.ty != hgl_source::Ty::Void {
        out.push(v);
        return;
    }
    match &v.kind {
        Kind::Delta(parts) => {
            for part in parts {
                for v in part.operands() {
                    value(v, out);
                }
            }
        }
        Kind::List(items) | Kind::Native(_, items) | Kind::Query(_, items) => {
            for v in items {
                value(v, out);
            }
        }
        Kind::Construct(items) => {
            for (_, v) in items {
                value(v, out);
            }
        }
        Kind::ValueCall(args, body) => {
            for v in args {
                value(v, out);
            }
            statements(body, out);
        }
        Kind::Field(v, _)
        | Kind::GlobalSet(_, v)
        | Kind::Length(v)
        | Kind::Unary(_, v)
        | Kind::IsPresent(v)
        | Kind::Present(v) => value(v, out),
        Kind::Index(a, b) | Kind::Push(a, b) | Kind::Binary(_, a, b) => {
            value(a, out);
            value(b, out);
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
