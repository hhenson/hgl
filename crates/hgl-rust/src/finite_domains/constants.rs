use hgl_semantics::ir::{Plan, Value};
/// Visit checked closed hook data without evaluating dynamic expressions.
pub fn collect(plan: &Plan) -> Vec<&Value> {
    let mut result = Vec::new();
    for node in &plan.nodes {
        hgl_semantics::ir_visit::hooks(node, &mut |value| {
            if value.closed() && value.ty != hgl_source::Ty::Void {
                result.push(value);
                false
            } else {
                true
            }
        });
    }
    result
}
