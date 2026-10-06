//! Cold typed conversion and lexical test execution emission.
use hgl_harness_ir::{Step, Test};
use hgl_rust_ir::Plan;
pub use hgl_rust_value_convert::{decode, encode};
/// Cold recursive conversion methods for complete reachable nominal batches.
pub fn recursive_markers(plan: &Plan) -> String {
    hgl_rust_recursive_convert::markers(plan, decode, encode)
}
/// Emit a lexical test runner using its exact prepared graph callbacks.
pub fn emit_test(test: &Test, _plans: &[Plan]) -> String {
    let callbacks = callbacks(&test.steps);
    let data = hgl_rust_checked_data::test(test);
    format!(
        "fn test()->Result<usize,String> {{ let mut context=hgl_time_context::RunContext::from_bundled()?; let test={data}; hgl_harness::execute(&test, |recipe|context.materialize(recipe).map_err(hgl_value_eval::EvalError::Operation), |case,prepared|match case {{{callbacks}_=>Err(\"unknown generated eval\".into())}}) }}\n"
    )
}

fn callbacks(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|s| match s {
            Step::Eval(e) | Step::BindEval(_, _, e) => {
                format!("{}=>case{}::run(prepared),", e.case, e.case)
            }
            Step::Raises(_, body) => callbacks(body),
            Step::If(_, a, b) => format!("{}{}", callbacks(a), callbacks(b)),
            Step::Ordinary(_) | Step::Assert(_) => String::new(),
        })
        .collect::<Vec<_>>()
        .concat()
}

/// Emit named execution outcomes and process status for a checked suite.
pub fn emit_main(tests: &[Test]) -> String {
    let mut out = Vec::new();
    out.push("fn main() {\nlet mut failed=0;\nlet mut evaluations=0;\n".into());
    for (i, test) in tests.iter().enumerate() {
        out.push(format!("match test{i}::test() {{Ok(count)=>{{evaluations+=count; println!(\"{{}} ... ok [executed]\",{:?});}},Err(e)=>{{failed+=1;println!(\"{{}} ... FAILED [executed]\",{:?});eprintln!(\"{{e}}\");}}}}\n",test.name,test.name));
    }
    out.push(format!("println!(\"{} executed tests, {{evaluations}} evaluations, {{failed}} failures\");\nif failed != 0 {{std::process::exit(1);}}\n}}\n",tests.len()));
    out.concat()
}
