//! Ordered constructor operands retain native helper results before destination writes.
use hgl_semantics::ir::{Kind, Statement, Value};
use hgl_source::Ty;
fn operands(value: &mut Value, prefix: &mut Vec<Statement>, next: &mut usize) {
    if let Kind::GlobalSet(_, child) = &mut value.kind {
        operands(child, prefix, next);
    }
    let Kind::Construct(fields) = &mut value.kind else {
        return;
    };
    let native = fields.iter().any(|(_, value)| {
        let mut found = false;
        hgl_semantics::ir_visit::value(value, &mut |v| {
            found |= matches!(v.ty, Ty::Native(_)) && matches!(v.kind, Kind::Native(..));
            !matches!(v.kind, Kind::ValueCall(..))
        });
        found
    });
    if !native {
        return;
    }
    for (_, child) in fields {
        operands(child, prefix, next);
        let id = *next;
        *next += 1;
        child.snapshot |= matches!(child.ty, Ty::Native(_));
        prefix.push(Statement::Let(id, child.clone()));
        child.kind = Kind::Local(id);
    }
}
/// Stage complete operands in their written order within the selected lexical branch.
pub(crate) fn statements(body: &mut Vec<Statement>, next: &mut usize) {
    let mut result = Vec::new();
    for mut statement in std::mem::take(body) {
        match &mut statement {
            Statement::Let(_, value)
            | Statement::Var(_, value)
            | Statement::Assign(_, value)
            | Statement::Return(value)
            | Statement::Call(value) => operands(value, &mut result, next),
            Statement::If(_, yes, no) => {
                statements(yes, next);
                statements(no, next);
            }
            Statement::While(_, body)
            | Statement::For(_, _, body)
            | Statement::ForItems(_, _, _, _, body) => statements(body, next),
            Statement::Borrow(..)
            | Statement::Exit
            | Statement::Yield(_)
            | Statement::TimedYield(..) => {}
        }
        result.push(statement);
    }
    *body = result;
}
