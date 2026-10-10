//! Required defaults check complete ordinary expressions before graph execution.
fn diagnostics(body: &str) -> Vec<hgl_source::diagnostics::Diagnostic> {
    hgl_program::diagnostics(&[("defaults.hgl".into(), format!("module defaults\n{body}"))])
}
#[test]
fn nested_selectors_keep_required_box_capabilities() {
    let declarations = "struct Cell { value:any }\nstruct Cells { items:list<Cell> }\nstruct Pairs { value:tuple<tuple<any,bool>,bool> }";
    for selected in [
        "Cells(items:[Cell(value:any(map<i64,i64>(items:[])))]).items[0].value",
        "Pairs(value:((any(map<i64,i64>(items:[])),true),true)).value[0][0]",
    ] {
        let errors = diagnostics(&format!(
            "{declarations}\nstruct Bad {{ flag:bool={selected}<any(map<i64,i64>(items:[])) }}"
        ));
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].issue.category, "type");
        assert_eq!(errors[0].issue.code, Some("value.constant_capability"));
        let selected = selected.replace("map<i64,i64>(items:[])", "1");
        let errors = diagnostics(&format!(
            "{declarations}\nstruct Good {{ flag:bool={selected}<any(2) }}"
        ));
        assert!(errors.is_empty(), "{errors:?}");
    }
}
#[test]
fn unused_generic_and_required_const_declarations_check_closed_defaults() {
    let missing = "any(map<i64,i64>(items:[]))<any(map<i64,i64>(items:[]))";
    for declaration in [
        format!("struct Bad<T> {{ value:T\nflag:bool={missing} }}"),
        format!("const fn bad<T>(value:T,flag:bool={missing})->bool => flag"),
        format!("fn bad(const value:i64,const flag:bool={missing}) {{ when {{ }} }}"),
    ] {
        let errors = diagnostics(&declaration);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].issue.code, Some("value.constant_capability"));
        let errors = diagnostics(&declaration.replace(missing, "any(1)<any(2)"));
        assert!(errors.is_empty(), "{errors:?}");
    }
}
#[test]
fn defaults_preserve_exact_ordinary_literal_context() {
    let errors = diagnostics("struct Bad { flag:bool=any([1])<any([2]) }");
    assert!(!errors.is_empty());
    assert!(
        errors
            .iter()
            .all(|e| e.issue.code != Some("value.constant_capability"))
    );
}
#[test]
fn inherited_generic_defaults_keep_declaring_scope_and_source_origin() {
    for (expression, expected) in [
        ("any(1)<any(2)", None),
        (
            "any(map<i64,i64>(items:[]))<any(map<i64,i64>(items:[]))",
            Some("value.constant_capability"),
        ),
    ] {
        let sources = vec![
            (
                "child.hgl".into(),
                "module child\nuse parent::{Parent}\nstruct Child: Parent<bool> { }".into(),
            ),
            (
                "parent.hgl".into(),
                format!(
                    "module parent\nconst fn default_flag()->bool => {expression}\nexport abstract struct Parent<T> {{ value:T=default_flag() }}"
                ),
            ),
        ];
        let errors = hgl_program::diagnostics(&sources);
        if let Some(code) = expected {
            assert_eq!(errors.len(), 1, "{errors:?}");
            assert_eq!(errors[0].issue.code, Some(code));
            assert_eq!(errors[0].source, "parent.hgl");
        } else {
            assert!(errors.is_empty(), "{errors:?}");
        }
    }
}

#[test]
fn public_compilers_check_unused_required_defaults_before_plans() {
    let bad = "any(map<i64,i64>(items:[]))<any(map<i64,i64>(items:[]))";
    let source = format!(
        "module defaults\nstruct Unused {{ flag:bool={bad} }}\nexport fn main() {{ }}\ntest empty {{ assert true }}"
    );
    let sources = vec![("defaults.hgl".into(), source.clone())];
    for result in [
        hgl_program::compile(&sources, "main").map(|_| ()),
        hgl_program::compile_suite(&sources).map(|_| ()),
        hgl_program::compile_module_suite(&sources).map(|_| ()),
    ] {
        assert!(result.unwrap_err().contains("value.constant_capability"));
    }
    let sources = vec![("defaults.hgl".into(), source.replace(bad, "any(1)<any(2)"))];
    assert!(hgl_program::compile(&sources, "main").is_ok());
    assert!(hgl_program::compile_suite(&sources).is_ok());
    assert!(hgl_program::compile_module_suite(&sources).is_ok());
}

#[test]
fn unused_temporal_defaults_remain_cold_run_context_recipes() {
    let source = "module defaults\nstruct Cold {zone:timezone=@[Missing/UnusedDefault]}\nconst fn default_zone(value:timezone=@[Missing/UnusedDefault])->timezone {return value}\nexport fn main() {}";
    let errors = hgl_program::diagnostics(&[("defaults.hgl".into(), source.into())]);
    assert!(errors.is_empty(), "{errors:?}");
    let result = hgl_program::compile(&[("defaults.hgl".into(), source.into())], "main");
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn default_scope_excludes_invocation_parameter_and_instance_field_bindings() {
    for declaration in [
        "const fn earlier(prior:i64,value:i64=prior)->i64=>value",
        "const fn current(value:i64=value)->i64=>value",
        "const fn later(value:i64=next,next:i64=2)->i64=>value",
        "fn configured(const prior:i64,const value:i64=prior) {when {return value}}",
        "struct Pair {prior:i64=7\nvalue:i64=prior}",
    ] {
        assert!(!diagnostics(declaration).is_empty(), "{declaration}");
    }
    assert!(diagnostics("const fn seed()->i64=>7\nconst fn choose(seed:i64,value:i64=seed())->i64=>seed+value\nstruct Pair {seed:i64=1\nvalue:i64=seed()}").is_empty());
}
#[test]
fn explicit_value_selector_controls_overload_role_and_fixed_lifted_arguments()
-> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let sources = hgl_program::library_files::sources(
        &[root.join("crates/hgl-program/tests/fixtures/default_selectors.hgl")],
        &[
            root.join("external/hgraph_std/hgl"),
            root.join("native/stdlib"),
        ],
    )?;
    assert!(hgl_program::compile_module_suite(&sources).is_ok());
    Ok(())
}
