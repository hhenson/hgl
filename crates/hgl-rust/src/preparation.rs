//! Cold typed conversion and lexical test execution emission.
pub use crate::value_convert::{decode, encode};
use hgl_semantics::harness_ir::{Step, Test};
use hgl_semantics::ir::{Kind, Native, Plan};
/// Cold recursive conversion methods for complete reachable nominal batches.
pub fn recursive_markers(plan: &Plan) -> String {
    crate::recursive_convert::markers(plan, decode, encode)
}
/// Emit a lexical test runner using its exact prepared graph callbacks.
pub fn emit_test(test: &Test, _plans: &[Plan]) -> String {
    let callbacks = callbacks(&test.steps);
    let data = crate::checked_data::test(test);
    format!(
        "fn test()->Result<usize,String> {{validate_native_images()?; let context=NativeContext(hgl_source::time_context::RunContext::from_bundled()?); let test={data}; hgl_semantics::harness::execute_in_context(&test, context, |case,prepared|match case {{{callbacks}_=>Err(\"unknown generated eval\".into())}}) }}\n"
    )
}
pub(crate) fn native_bindings(plans: &[Plan], tests: &[Test]) -> Vec<Native> {
    let mut bindings = plans
        .iter()
        .flat_map(|p| p.natives.iter())
        .cloned()
        .collect::<Vec<_>>();
    hgl_semantics::ir_visit::harness(tests, &mut |v| {
        if let Kind::Native(binding, _) = &v.kind {
            bindings.push(*binding.clone());
        }
        true
    });
    bindings
}
pub(crate) fn native_context(plan: &Plan) -> String {
    let mut names = std::collections::BTreeSet::new();
    let callbacks=plan.natives.iter().filter_map(|n| n.provider.as_ref().map(|p|(n,p))).filter(|(n,_)|names.insert(&n.name)).map(|(n,p)| {
        let args=n.args.iter().enumerate().map(|(i,t)|format!("let arg{i}=(||->Result<{},String>{{Ok({})}})().map_err(hgl_semantics::value_eval::EvalError::Operation)?;",crate::layouts::owned_type(t),decode(t,&format!("&args[{i}]")))).collect::<Vec<_>>().concat();
        let call_args=n.args.iter().enumerate().map(|(i,t)|format!("{}arg{i}",if *t==hgl_source::Ty::Str {"&"}else{""})).collect::<Vec<_>>().join(",");
        let call=format!("{}::{}({call_args}){}",hgl_rust_codegen::native_image::image_module(&p.provider),n.method,if n.throws {".map_err(|error|error.code.map_or_else(||hgl_semantics::value_eval::EvalError::Operation(error.message.clone()),|code|hgl_semantics::value_eval::EvalError::Coded(code,error.message.clone())))?"}else{""});
        format!("{:?}=>{{{args}let result={call};Ok({})}},",n.name,encode(&n.result,"&result"))
    }).collect::<Vec<_>>().concat();
    format!(
        "struct NativeContext(hgl_source::time_context::RunContext); impl hgl_semantics::value_eval::ExecutionContext<hgl_semantics::ir::Value,hgl_semantics::ir::Native,hgl_semantics::value_eval::EvalError> for NativeContext {{fn temporal(&mut self,recipe:&hgl_source::TemporalLiteral)->Result<hgl_source::Literal,hgl_semantics::value_eval::EvalError>{{self.0.materialize(recipe).map_err(hgl_semantics::value_eval::EvalError::Operation)}} fn native(&mut self,binding:&hgl_semantics::ir::Native,args:&[hgl_semantics::ir::Value])->Option<Result<hgl_semantics::ir::Value,hgl_semantics::value_eval::EvalError>> {{ Some((|| {{match binding.name.as_str() {{{callbacks}_=>Err(hgl_semantics::value_eval::EvalError::Unsupported(\"unknown checked native binding\".into()))}}}})()) }} }}"
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
