//! Prepared Any comparison operands preserve ordered evaluation without allocating boxes.
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;
pub(crate) fn prepare(v: &mut Value, globals: &mut Vec<(String, Ty)>) -> bool {
    if let Kind::Binary(op, left, right) = &v.kind
        && matches!(op.as_str(), "==" | "!=" | "<" | "<=" | ">" | ">=")
        && left.ty == Ty::Any
    {
        let entry = globals.len();
        for offset in 0..2 {
            globals.push((format!("\0any-compare:{entry}:{offset}"), Ty::Any));
        }
        *v = Value::new(
            Ty::Bool,
            Kind::Query(
                format!("\0any-compare:{entry}:{op}"),
                vec![*left.clone(), *right.clone()],
            ),
        );
        return true;
    }

    false
}
/// Compare independently retained operands through compiled codecs without native owning extraction.
pub(crate) fn comparison(plan: &Plan, op: &str, args: &[Value]) -> String {
    let (entry, operation) = op
        .strip_prefix("\0any-compare:")
        .and_then(|s| s.split_once(':'))
        .unwrap_or_else(|| unreachable!("compiled Any comparison"));
    let entry = entry
        .parse::<usize>()
        .unwrap_or_else(|_| unreachable!("compiled Any slot"));
    let mut code = String::from("{");
    for (offset, arg) in args.iter().enumerate() {
        let box_value = Value::new(Ty::Any, Kind::Unary("any".into(), Box::new(arg.clone())));
        crate::append(
            &mut code,
            format_args!(
                "let slot{offset}=_ctx.global_state().destination(self.global{});{{{}}}",
                entry + offset,
                crate::any_prepared::write(plan, &box_value, &format!("slot{offset}"), |v| {
                    crate::values::condition_code(plan, v)
                })
            ),
        );
    }
    let left = "_ctx.store().global_values().any(slot0.fields())";
    let right = "_ctx.store().global_values().any(slot1.fields())";
    crate::append(
        &mut code,
        format_args!(
            "{}}}",
            crate::any_values::comparison(operation, left, right)
        ),
    );
    code
}
