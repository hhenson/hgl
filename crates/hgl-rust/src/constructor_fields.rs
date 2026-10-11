//! Complete constructor writes preserve source order and clear omitted optional presence.
use crate::layouts::global_type;
use hgl_semantics::ir::{Kind, Value};
use hgl_source::Ty;
fn schema(ty: &Ty) -> Ty {
    crate::layouts::ordinary_storage(ty).unwrap_or_else(|| ty.clone())
}
fn child_slot(marker: &str, parent: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{marker}>::bind(&mut columns.prepared_list(({parent}).fields()).first().ok_or_else(||hgl_types::NodeError::new(\"unprepared constructor child\"))?.as_slice())"
    )
}
/// Write declared optional/recursive fields while preserving supplied operand order.
pub(crate) fn fields(
    v: &Value,
    slot: &str,
    mut write: impl FnMut(&Value, &str) -> String,
    scope: impl Fn(&str) -> String,
) -> Option<String> {
    let Kind::Construct(fields) = &v.kind else {
        return None;
    };
    let schema = schema(&v.ty);
    let optional = schema
        .structure()
        .map_or(&[][..], |(_, _, optional)| optional);
    let mut code = String::new();
    for i in optional
        .iter()
        .filter(|i| !fields.iter().any(|(id, _)| id == *i))
    {
        code += &scope(&format!(
            "columns.set_list_len(({slot}).fields().{i}.fields(),0);"
        ));
    }
    for (i, value) in fields {
        let target = format!("({slot}).fields().{i}");
        if optional.contains(i) {
            let (_, schema, _) = schema
                .structure()
                .unwrap_or_else(|_| unreachable!("declared optional constructor field"));
            let edge =
                matches!(&schema[*i].1, Ty::Recursive(batch) if batch.definitions().is_empty());
            let marker = global_type(&value.ty);
            let field = if edge {
                format!("hgl_store::Recursive<{marker}>")
            } else {
                marker.clone()
            };
            let child = child_slot(&field, &target);
            let child = if edge {
                child_slot(&marker, &child)
            } else {
                child
            };
            code += &write(value, &child);
            code += &scope(&format!("columns.set_list_len(({target}).fields(),1);"));
        } else {
            code += &write(value, &target);
        }
    }
    Some(code)
}
