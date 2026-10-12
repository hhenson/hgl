//! Definite assignment and finite target proof stay separate from source admission.
fn sources(body: &str) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut sources = hgl_program::library_files::sources(
        &[],
        &[
            root.join("external/hgraph_std/hgl/hgraph"),
            root.join("native/stdlib"),
        ],
    )?;
    sources.insert(0,("conditional.hgl".into(),format!("module hgraph.std part conditional_admission_controls\nfn route(value:ref<i64>)->ref<i64> {{when {{return value}}}}\n{body}")));
    Ok(sources)
}
#[test]
fn both_branches_must_definitely_assign_the_declared_reference()
-> Result<(), Box<dyn std::error::Error>> {
    for (body, message) in [
        (
            "if enabled {result=route(value)} else {}",
            "not definitely assigned",
        ),
        (
            "if enabled {result=route(value)} else {result=route(result)}",
            "read before definite assignment",
        ),
    ] {
        let source = sources(&format!(
            "fn choice(enabled:bool,value:i64)->ref<i64> {{var result:ref<i64>\n{body}\nresult}}\nfn flag()->bool {{yield 0us:true}}\nfn scalar()->i64 {{yield 0us:7}}\nfn main()->ref<i64> => choice(flag(),scalar())"
        ))?;
        let error = hgl_program::compile(&source, "main").unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    Ok(())
}
#[test]
fn child_generators_preserve_source_admission_and_report_an_unproved_rust_target()
-> Result<(), Box<dyn std::error::Error>> {
    let source = sources(
        "fn leaf()->i64 {yield 0us:7}\nfn choice(enabled:bool,value:i64)->ref<i64> {var result:ref<i64>\nif enabled {result=route(leaf())} else {result=route(value)}\nresult}\nfn flag()->bool {yield 0us:true}\nfn main()->ref<i64> => choice(flag(),leaf())\nfn followed(enabled:bool,value:i64)->i64 => pass_through(choice(enabled,value))\ntest {test target_bound {eval(followed,enabled:[true,false,true],value:[7,8,9])}}",
    )?;
    let program = hgl_program::compile(&source, "main")?;
    let error = hgl_program::emit_rust(&program);
    assert!(error.starts_with("compile_error!"));
    assert!(error.contains("no child generators or scheduler hooks"));
    let error = hgl_program::compile_module_suite(&source).unwrap_err();
    assert!(
        error.contains("no child generators or scheduler hooks"),
        "{error}"
    );
    Ok(())
}
#[test]
fn owning_cache_reset_requires_a_separate_prepared_activation_proof()
-> Result<(), Box<dyn std::error::Error>> {
    let source = sources(
        "fn leaf()->i64 {state retained:str=\"owned\"\nwhen {return 7}}\nfn choice(enabled:bool,value:i64)->ref<i64> {var result:ref<i64>\nif enabled {result=route(leaf())} else {result=route(value)}\nresult}\nfn flag()->bool {yield 0us:true}\nfn main()->ref<i64> => choice(flag(),leaf())\nfn followed(enabled:bool,value:i64)->i64 => pass_through(choice(enabled,value))\ntest {test target_bound {eval(followed,enabled:[true,false,true],value:[7,8,9])}}",
    )?;
    let program = hgl_program::compile(&source, "main")?;
    assert!(hgl_program::emit_rust(&program).contains("owning cache reset at activation"));
    let error = hgl_program::compile_module_suite(&source).unwrap_err();
    assert!(
        error.contains("owning cache reset at activation"),
        "{error}"
    );
    Ok(())
}
