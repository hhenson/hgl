use crate::{emit, index, resolve};
use hgl_harness_ir::{Evaluation, Step, Test};

/// Checked lexical tests and independently selected graph plans.
#[derive(Debug)]
pub struct Suite {
    tests: Vec<Test>,
    plans: Vec<hgl_rust::Plan>,
}
/// Check named tests without executing ordinary setup or supplied eval values.
pub fn compile_tests(sources: &[(String, String)]) -> Result<Suite, String> {
    let library = index::load(sources)?;
    hgl_enums::validate(&library)?;
    let mut suite = Suite {
        tests: Vec::new(),
        plans: Vec::new(),
    };
    let mut names = std::collections::BTreeSet::new();
    for decl in &library.declarations {
        if decl.role != index::Role::Test {
            continue;
        }
        let name = format!("{}::{}", decl.module, decl.name);
        if !names.insert(name.clone()) {
            return Err(format!("duplicate test {name}"));
        }
        let mut scope = hgl_static_values::PreparedLexicalScope::default();
        let mut steps = Vec::new();
        for step in hgl_eval_data::steps(&decl.tokens)? {
            match step {
                hgl_eval_data::TestStep::Ordinary(statement) => {
                    if let hgl_source::Stmt::Let(name, _, _) | hgl_source::Stmt::Var(name, _, _) =
                        &statement
                        && scope.bindings.contains_key(name)
                    {
                        return Err(format!("duplicate test local {name}"));
                    }
                    steps.push(Step::Ordinary(resolve::prepared_statement(
                        library.clone(),
                        &decl.module,
                        &statement,
                        &mut scope,
                    )?));
                }
                hgl_eval_data::TestStep::Assert(expr) => steps.push(Step::Assert(
                    resolve::prepared_assertion(library.clone(), &decl.module, &expr, &scope)?,
                )),
                hgl_eval_data::TestStep::Eval(call) => {
                    let (plan, arguments) = resolve::prepare_evaluation(
                        library.clone(),
                        &decl.module,
                        &call.function,
                        &call.arguments,
                        &scope,
                    )
                    .map_err(|e| format!("{name}: {e}"))?;
                    let expected = call
                        .expected
                        .map(|slots| {
                            let (_, ty) = plan
                                .output
                                .as_ref()
                                .ok_or("outputless eval cannot be compared")?;
                            resolve::prepared_expected(
                                library.clone(),
                                &decl.module,
                                ty,
                                &slots,
                                &scope,
                            )
                            .map_err(|e| format!("{name}: expected output: {e}"))
                        })
                        .transpose()?;
                    steps.push(Step::Eval(Evaluation {
                        case: suite.plans.len(),
                        arguments,
                        expected,
                    }));
                    suite.plans.push(plan);
                }
            }
        }
        suite.tests.push(Test { name, steps });
    }
    if suite.tests.is_empty() {
        return Err("no tests found".into());
    }
    Ok(suite)
}
/// Emit ordered test setup, prepared graph factories and owning capture comparison.
pub fn emit_tests(suite: &Suite) -> String {
    let mut out = vec![emit::shared_layouts(&suite.plans)];
    for (i, plan) in suite.plans.iter().enumerate() {
        out.push(format!(
            "mod case{i} {{ use super::*;\n{}\n{}\n}}\n",
            emit::emit_shared(plan),
            emit::emit_prepared_test_body(plan)
        ));
    }
    for (i, test) in suite.tests.iter().enumerate() {
        let code = hgl_rust_preparation::emit_test(test, &suite.plans)
            .replace("fn test()", "pub fn test()");
        out.push(format!("mod test{i} {{ use super::*; {code} }}\n"));
    }
    out.push("fn main() {\nlet mut failed=0;\nlet mut evaluations=0;\n".into());
    for (i, test) in suite.tests.iter().enumerate() {
        out.push(format!("match test{i}::test() {{Ok(count)=>{{evaluations+=count; println!(\"{{}} ... ok\",{:?});}},Err(e)=>{{failed+=1;eprintln!(\"{{}}: {{e}}\",{:?});}}}}\n",test.name,test.name));
    }
    out.push(format!("println!(\"{} tests, {{evaluations}} evaluations, {{failed}} failures\");\nif failed != 0 {{std::process::exit(1);}}\n}}\n",suite.tests.len()));
    out.concat()
}
