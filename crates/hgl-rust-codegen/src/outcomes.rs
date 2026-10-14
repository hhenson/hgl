//! Typed outcome construction and selection preserve selected payload ownership.
use hgl_semantics::ir::{Kind, Plan, Value};
use hgl_source::Ty;
use std::fmt::Write as _;
/// Emit one complete ordinary tagged outcome or diagnostic in supplied operand order.
pub fn construct(value: &Value, emit: impl Fn(&Value) -> String) -> String {
    let Kind::Construct(fields) = &value.kind else {
        unreachable!("checked outcome constructor");
    };
    let mut code = String::from("{");
    for (index, value) in fields {
        write!(code, "let field{index}={};", emit(value))
            .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    if matches!(value.ty, Ty::Result(..)) {
        if fields.iter().any(|(index, _)| *index == 1) {
            code += "Ok(field1)";
        } else {
            code += "Err(field2)";
        }
    } else {
        let code_field = if fields.iter().any(|(index, _)| *index == 1) {
            "Some(field1)"
        } else {
            "None"
        };
        write!(
            code,
            "hgl_types::ErrorValue{{message:field0,code:{code_field},context:None}}"
        )
        .unwrap_or_else(|_| unreachable!("String formatting"));
    }
    code.push('}');
    code
}
/// Emit checked predicate, payload projection or ordinary propagation.
pub fn query(plan: &Plan, op: &str, args: &[Value], emit: impl Fn(&Value) -> String) -> String {
    if matches!(op, "is_ok" | "is_err") {
        if let Some((arena, slot)) = crate::source_slots::source(plan, &args[0]) {
            let columns = columns(arena);
            let tag = format!("{{let slot={slot};slot.fields().0.read({columns})?}}");
            return if op == "is_ok" {
                tag
            } else {
                format!("!({tag})")
            };
        }
        let method = if op == "is_ok" { "is_ok" } else { "is_err" };
        let operand = if let Kind::Local(id) | Kind::MutableLocal(id) = args[0].kind {
            format!("local{id}")
        } else {
            emit(&args[0])
        };
        return format!("({operand}).{method}()");
    }
    let operand = emit(&args[0]);
    match op {
        "\0result_value" => format!(
            "match {operand} {{Ok(value)=>value,Err(_)=>unreachable!(\"checked success variant\")}}"
        ),
        "\0result_error" => format!(
            "match {operand} {{Err(value)=>value,Ok(_)=>unreachable!(\"checked failure variant\")}}"
        ),
        "\0error_message" => format!("({operand}).message"),
        "\0error_code" => format!("({operand}).code"),
        "\0error_context" => format!("({operand}).context"),
        "\0context_node" => format!("({operand}).node"),
        "\0context_phase" => format!("({operand}).phase"),
        "\0context_time" => format!("({operand}).evaluation_time"),
        "\0propagate_helper" => {
            format!("match {operand} {{Ok(value)=>value,Err(error)=>return Ok(Err(error))}}")
        }
        "\0propagate_node" => format!(
            "match {operand} {{Ok(value)=>value,Err(error)=>return Err(Box::new(hgl_types::NodeError::from_error(error)))}}"
        ),
        _ => unreachable!("checked outcome operation"),
    }
}
fn columns(arena: &str) -> String {
    if arena == "configuration_columns" {
        "&self.configuration_columns".into()
    } else {
        format!("_ctx.store().{arena}()")
    }
}
