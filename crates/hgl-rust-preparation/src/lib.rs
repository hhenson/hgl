//! Cold typed conversion and lexical test execution emission.
use hgl_harness_ir::{Step, Test};
use hgl_rust_checked_data::ty;
use hgl_rust_ir::Plan;
mod convert;
pub use convert::{decode, encode};
/// Cold recursive conversion methods for complete reachable nominal batches.
pub fn recursive_markers(plan: &Plan) -> String {
    hgl_rust_recursive_convert::markers(plan, decode, encode)
}
/// Emit a lexical test runner using its exact prepared graph callbacks.
pub fn emit_test(test: &Test, _plans: &[Plan]) -> String {
    let callbacks = test
        .steps
        .iter()
        .filter_map(|s| {
            if let Step::Eval(e) = s {
                Some(format!("{}=>case{}::run(prepared),", e.case, e.case))
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .concat();
    let data = hgl_rust_checked_data::test(test);
    format!(
        "fn test()->Result<usize,String> {{ let mut context=hgl_time_context::RunContext::from_bundled()?; let test={data}; hgl_harness::execute(&test, |recipe|context.materialize(recipe).map_err(hgl_value_eval::EvalError::Operation), |case,prepared|match case {{{callbacks}_=>Err(\"unknown generated eval\".into())}}) }}\n"
    )
}
