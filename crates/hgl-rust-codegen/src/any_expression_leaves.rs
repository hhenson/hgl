//! Typed leaf views for prepared Any frame codecs.
use hgl_semantics::ir::{Kind, Plan, Value};
pub(crate) fn emit(
    plan: &Plan,
    value: &Value,
    prelude: &mut Vec<String>,
    emit: &impl Fn(&Value) -> String,
    boxed: bool,
) -> String {
    let id = prelude.len();
    let name = crate::any_values::name(&value.ty);
    let prefix = if boxed { "any_box" } else { "any" };
    if let Kind::Input(input, false) = value.kind
        && value.ty.scalar_kind().is_some()
    {
        prelude.push(format!("if !_ctx.valid(self.input{input}) {{return Err(hgl_types::NodeError::new(\"prepared input is invalid\"));}}"));
        return format!(
            "any_write_{name}(observation.scalar::<{}>(self.input{input}.id()).expect(\"preflighted scalar input\"),out)",
            crate::layouts::rust_type(&value.ty)
        );
    }
    if let Some((arena, source)) = crate::source_slots::source(plan, value) {
        prelude.push(format!("let expression_source{id}={source};"));
        format!(
            "{prefix}_slots_{name}({},expression_source{id},out)",
            crate::source_slots::columns(arena)
        )
    } else {
        prelude.push(format!("let expression_value{id}={};", emit(value)));
        let operation = if boxed { "native" } else { "write" };
        format!("{prefix}_{operation}_{name}(&expression_value{id},out)")
    }
}
