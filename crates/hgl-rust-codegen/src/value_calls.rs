//! Cold argument representation for directly emitted ordinary value calls.
use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::{Literal, Ty};
/// Reuse an effect-free literal only when no formal-local reference requires owned text.
pub fn argument(
    value: &Value,
    id: usize,
    body: &[Statement],
    emit: impl Fn(&Value) -> String,
) -> String {
    if let Kind::Literal(Literal::Str(text)) = &value.kind
        && !statements(body, Some(id))
    {
        format!("{text:?}")
    } else {
        emit(value)
    }
}
fn visit(value: &Value, id: Option<usize>, used: &mut bool) -> bool {
    if crate::scalars::bytes_input(value).is_some() {
        return false;
    }
    if matches!(value.kind,Kind::Local(local)|Kind::MutableLocal(local) if Some(local)==id) {
        *used = true;
    }
    if id.is_none()
        && let Kind::ValueCall(args, body) = &value.kind
    {
        *used |= args.iter().enumerate().any(|(i, value)| {
            crate::execution_proof::owning_argument(&value.ty)
                && !matches!(value.ty, Ty::Result(..) | Ty::Error)
                && (!(value.ty == Ty::Str && matches!(value.kind, Kind::Literal(Literal::Str(_))))
                    || statements(body, Some(i)))
        });
    }
    true
}
fn statements(body: &[Statement], id: Option<usize>) -> bool {
    let mut used = false;
    hgl_semantics::ir_visit::statements(body, &mut |v| visit(v, id, &mut used));
    used
}

/// Prove that runtime helper text arguments do not require fresh owning storage.
pub fn prepared(plan: &Plan) -> bool {
    plan.nodes.iter().all(|node| {
        let mut used = false;
        hgl_semantics::ir_visit::hooks(node, &mut |v| visit(v, None, &mut used));
        !used
    })
}
