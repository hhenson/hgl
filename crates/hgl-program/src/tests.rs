use crate::{emit, index, resolve};
use hgl_harness_ir::Test;

pub use hgl_harness_ir::Suite;
/// Check named tests without executing ordinary setup or supplied eval values.
pub fn compile_tests(sources: &[(String, String)]) -> Result<Suite, String> {
    hgl_source_check::ensure_sources(sources)?;
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
        let steps = hgl_harness_check::block(
            hgl_eval_data::steps(&decl.tokens)?,
            &mut hgl_static_values::PreparedLexicalScope::default(),
            &mut suite.plans,
            &mut resolve::TestChecker {
                library: library.clone(),
                module: decl.module.clone(),
            },
        )
        .map_err(|e| hgl_diagnostics::render_issue(sources, e.in_source(&decl.source)))?;
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
        out.push(format!("match test{i}::test() {{Ok(count)=>{{evaluations+=count; println!(\"{{}} ... ok\",{:?});}},Err(e)=>{{failed+=1;eprintln!(\"{{}} ... FAILED: {{e}}\",{:?});}}}}\n",test.name,test.name));
    }
    out.push(format!("println!(\"{} tests, {{evaluations}} evaluations, {{failed}} failures\");\nif failed != 0 {{std::process::exit(1);}}\n}}\n",suite.tests.len()));
    out.concat()
}
