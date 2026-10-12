//! Pure lexical classification for checking-time ordinary evaluation.
use crate::ir::{Kind, Statement, Value};
use hgl_value_model::Ty;
use std::collections::BTreeSet;

/// Whether a complete expression can execute without external values or effects.
pub fn context_free(value: &Value) -> bool {
    expression(value, &BTreeSet::new())
}
fn expression(value: &Value, locals: &BTreeSet<usize>) -> bool {
    let mut constant = true;
    crate::ir_visit::value(value, &mut |value| {
        if !constant {
            return false;
        }
        if let Kind::Local(id) | Kind::MutableLocal(id) = &value.kind {
            constant = locals.contains(id);
            return false;
        }
        if let Kind::ValueCall(args, body) = &value.kind {
            constant = args.iter().all(|v| expression(v, locals))
                && block(body, &(0..args.len()).collect());
            return false;
        }
        constant = matches!(
            value.kind,
            Kind::Literal(_)
                | Kind::Void
                | Kind::List(_)
                | Kind::Construct(_)
                | Kind::Delta(_)
                | Kind::Field(..)
                | Kind::Length(_)
                | Kind::Unary(..)
                | Kind::Binary(..)
                | Kind::Index(..)
                | Kind::Push(..)
        );
        constant
    });
    constant
}

fn block(body: &[Statement], inherited: &BTreeSet<usize>) -> bool {
    let mut locals = inherited.clone();
    body.iter().all(|statement| match statement {
        Statement::Let(id, value) | Statement::Var(id, value) => {
            let closed = expression(value, &locals);
            locals.insert(*id);
            closed
        }
        Statement::Yield(value) | Statement::Call(value) => expression(value, &locals),
        Statement::Assign(target, value) => {
            expression(target, &locals) && expression(value, &locals)
        }
        Statement::If(condition, yes, no) => {
            expression(condition, &locals) && block(yes, &locals) && block(no, &locals)
        }
        Statement::Exit => true,
        Statement::Borrow(..)
        | Statement::ForItems(..)
        | Statement::For(..)
        | Statement::TimedYield(..)
        | Statement::While(..)
        | Statement::Return(_) => false,
    })
}

/// Retain closed owning constructor subtrees as immutable configuration recipes.
pub fn retain_closed(
    value: &mut Value,
    configuration: &mut Vec<Value>,
    owning: &impl Fn(&Ty) -> bool,
) {
    let closed = value.closed()
        || matches!(&value.kind, Kind::Unary(op, child) if op == "family" && child.closed());
    if closed && owning(&value.ty) {
        let id = configuration.len();
        configuration.push(value.clone());
        value.kind = Kind::Configuration(id);
    } else if let Kind::Construct(fields) = &mut value.kind {
        for (_, child) in fields {
            retain_closed(child, configuration, owning);
        }
    }
}
