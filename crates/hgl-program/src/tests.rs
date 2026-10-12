use crate::{emit, index, resolve};
use hgl_semantics::harness_ir::Test;

pub use hgl_semantics::harness_ir::Suite;
/// Check named tests without executing ordinary setup or supplied eval values.
pub fn compile_suite(sources: &[(String, String)]) -> Result<Suite, String> {
    suite(sources, false)
}
/// Check only the root module's named tests; imported tests retain separate ownership.
pub fn compile_module_suite(sources: &[(String, String)]) -> Result<Suite, String> {
    suite(sources, true)
}
fn suite(sources: &[(String, String)], module_only: bool) -> Result<Suite, String> {
    let check = |library: &index::Library, decl: &index::Decl| {
        resolve::validate_defaults(library.clone(), decl)
    };
    let errors = if module_only {
        hgl_semantics::source_check::with_module_semantics(sources, check)
    } else {
        hgl_semantics::source_check::with_semantics(sources, check)
    };
    hgl_source::diagnostics::ensure(errors)?;
    let library = index::load(sources)?;
    hgl_semantics::enums::validate(&library)?;
    let mut suite = Suite::default();
    let mut names = std::collections::BTreeSet::new();
    for decl in library.declarations.iter() {
        if decl.role != index::Role::Test || (module_only && decl.module != library.root) {
            continue;
        }
        let name = format!("{}::{}", decl.module, decl.name);
        if !names.insert(name.clone()) {
            return Err(format!("duplicate test {name}"));
        }
        let steps = hgl_semantics::harness_check::block(
            hgl_semantics::eval_data::steps(&decl.tokens)?,
            &mut hgl_semantics::static_values::PreparedLexicalScope::default(),
            &mut suite.plans,
            &mut resolve::TestChecker {
                library: library.clone(),
                module: decl.module.clone(),
            },
        )
        .map_err(|e| hgl_source::diagnostics::render_issue(sources, e.in_source(&decl.source)))?;
        suite.tests.push(Test { name, steps });
    }
    for plan in &suite.plans {
        emit::native_results::validate(plan)?;
        if let Some(error) = emit::conditional_target_error(plan) {
            return Err(error);
        }
    }
    let mut selected_images = suite
        .plans
        .iter()
        .flat_map(|plan| plan.natives.iter())
        .filter_map(|native| native.provider.clone())
        .collect::<Vec<_>>();
    hgl_semantics::ir_visit::harness(&suite.tests, &mut |value| {
        if let hgl_semantics::ir::Kind::Native(binding, _) = &value.kind
            && let Some(image) = &binding.provider
        {
            selected_images.push(image.clone());
        }
        true
    });
    suite.native_images = hgl_semantics::native_images::select(
        &library,
        &hgl_semantics::native_images::combined_scalars(&suite.plans, &suite.tests)
            .map_err(|issue| hgl_source::diagnostics::render_issue(sources, issue))?,
        &selected_images,
    )
    .map_err(|issue| hgl_source::diagnostics::render_issue(sources, issue))?;
    Ok(suite)
}
/// Check named source tests, retaining the ordinary no-tests error.
pub fn compile_tests(sources: &[(String, String)]) -> Result<Suite, String> {
    compile_suite(sources)?.require_tests()
}
/// Emit ordered test setup, prepared graph factories and owning capture comparison.
pub fn emit_tests(suite: &Suite) -> String {
    let mut out = vec![emit::shared_suite_layouts(suite)];
    for (i, plan) in suite.plans.iter().enumerate() {
        out.push(format!(
            "mod case{i} {{ use super::*;\n{}\n{}\n}}\n",
            emit::emit_shared(plan),
            emit::emit_prepared_test_body(plan)
        ));
    }
    for (i, test) in suite.tests.iter().enumerate() {
        let code = hgl_rust::preparation::emit_test(test, &suite.plans)
            .replace("fn test()", "pub fn test()");
        out.push(format!("mod test{i} {{ use super::*; {code} }}\n"));
    }
    out.push(hgl_rust::preparation::emit_main(&suite.tests));
    out.concat()
}
