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

fn execute_defaults(sources: &[(String, String)], assertions: usize) -> Result<(), String> {
    let suite = hgl_program::compile_module_suite(sources)?;
    assert_eq!(suite.tests.len(), 1);
    let actual = hgl_semantics::harness::execute(
        &suite.tests[0],
        |_| {
            Err(hgl_semantics::value_eval::EvalError::Unsupported(
                "unexpected contextual literal".into(),
            ))
        },
        |_, _| Err("unexpected graph evaluation".into()),
    )?;
    assert_eq!(actual, assertions);
    Ok(())
}
#[test]
fn inferred_generic_defaults_materialize_in_the_declaring_scope() -> Result<(), String> {
    let sources = vec![
        ("caller.hgl".into(), "module caller\nuse defaults::{choose}\nconst fn make_i64()->i64=>99\ntest inferred {assert choose(3)==7\nassert choose(3,fallback:11)==11}".into()),
        ("defaults.hgl".into(), "module defaults\nconst fn make_i64()->i64=>7\nexport const fn choose<T>(value:T,fallback:T=make_i64())->T=>fallback".into()),
    ];
    execute_defaults(&sources, 2)
}
#[test]
fn computed_const_default_precedes_dependent_size_annotations() -> Result<(), String> {
    execute_defaults(&[("defaults.hgl".into(), "module defaults\nconst fn one()->i64=>1\nconst fn sized(const n:i64=one(),values:list<i64,n> = [9])->i64=>values[0]\ntest sizes {assert sized()==9\nlet supplied:list<i64,1> = [4]\nassert sized(n:1,values:supplied)==4}".into())], 2)
}
#[test]
fn inferred_defaults_preserve_capability_errors_and_exact_types() {
    for (expression, argument, expected) in [
        (
            "any(map<i64,i64>(items:[]))<any(map<i64,i64>(items:[]))",
            "true",
            "value.constant_capability",
        ),
        ("false", "3", "type mismatch"),
        ("null", "3", "null"),
    ] {
        let source = format!(
            "module defaults\nconst fn choose<T>(value:T,fallback:T={expression})->T=>fallback\ntest inferred {{assert choose({argument})=={argument}}}"
        );
        let error =
            hgl_program::compile_module_suite(&[("defaults.hgl".into(), source)]).unwrap_err();
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("missing argument fallback"), "{error}");
    }
}
#[test]
fn computed_zero_size_default_retains_an_empty_fixed_list() -> Result<(), String> {
    execute_defaults(&[("defaults.hgl".into(), "module defaults\nconst fn zero()->i64=>0\nconst fn empty(const n:i64=zero(),values:list<i64,n> = [])->list<i64,n> => values\ntest zero {assert len(empty())==0}".into())], 1)
}

#[test]
fn supplied_generic_fields_do_not_hide_invalid_concrete_defaults() {
    for (default, supplied, expected) in [
        ("unknown()", "1", "unknown"),
        ("false", "1", "type mismatch"),
        (
            "any(map<i64,i64>(items:[]))<any(map<i64,i64>(items:[]))",
            "true",
            "value.constant_capability",
        ),
    ] {
        for constructor in [
            format!("Box(value:{supplied})"),
            format!(
                "Box<{}>(value:{supplied})",
                if supplied == "true" { "bool" } else { "i64" }
            ),
        ] {
            let source = format!(
                "module defaults\nstruct Box<T> {{value:T={default}}}\ntest supplied {{let value={constructor}\nassert value.value=={supplied}}}"
            );
            let error = hgl_program::compile_module_suite(&[("defaults.hgl".into(), source)])
                .expect_err("concrete default must be checked even when supplied");
            assert!(error.contains(expected), "{error}");
        }
    }
}

#[test]
fn supplied_generic_fields_keep_values_and_declaring_default_scope() -> Result<(), String> {
    execute_defaults(&[
        ("caller.hgl".into(), "module caller\nuse defaults::{Box}\nconst fn seed()->bool=>false\ntest supplied {assert Box(value:3).value==3\nassert Box<i64>().value==7\nassert Box<i64>(value:11).value==11}".into()),
        ("defaults.hgl".into(), "module defaults\nconst fn seed()->i64=>7\nexport struct Box<T> {value:T=seed()}".into()),
    ], 3)
}

#[test]
fn constructor_defaults_receive_explicit_inferred_and_contextual_type_bindings()
-> Result<(), String> {
    execute_defaults(&[("defaults.hgl".into(), "module defaults\nstruct Box<T> {value:T=7}\nstruct Outer<T> {inner:Box<T> = Box<T>()}\ntest generic_fields {assert Outer<i64>().inner.value==7\nassert Outer(inner:Box(value:3)).inner.value==3\nlet contextual:Outer<i64> = Outer()\nassert contextual.inner.value==7}".into())], 3)
}

#[test]
fn inherited_constructor_defaults_receive_declaring_generic_bindings() -> Result<(), String> {
    execute_defaults(&[
        ("caller.hgl".into(), "module caller\nuse defaults::{Parent}\nstruct Child<T>:Parent<list<T>> {}\nstruct Box<T> {wrong:bool}\ntest inherited {assert len(Child<i64>().inner.value)==0}".into()),
        ("defaults.hgl".into(), "module defaults\nexport struct Box<T> {value:T=[]}\nexport abstract struct Parent<U> {inner:Box<U> = Box<U>()}".into()),
    ], 1)
}

#[test]
fn type_parameter_names_do_not_defer_closed_callable_defaults() {
    for declaration in [
        "struct Unused<seed> {flag:bool=any(seed())<any(seed())}",
        "const fn unused<seed>(value:seed,flag:bool=any(seed())<any(seed()))->bool=>flag",
    ] {
        let errors = diagnostics(&format!(
            "const fn seed()->map<i64,i64> =>map<i64,i64>(items:[])\n{declaration}"
        ));
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].issue.code, Some("value.constant_capability"));
        assert_eq!(errors[0].source, "defaults.hgl");
    }
}

#[test]
fn concrete_inherited_defaults_check_unused_declarations_in_ancestor_type_scope() {
    let sources = vec![
        ("caller.hgl".into(), "module caller\nuse defaults::{Parent}\nstruct Child:Parent<list<i64>> {}".into()),
        ("defaults.hgl".into(), "module defaults\nexport struct Box<T> {value:T=[]}\nexport abstract struct Parent<U> {inner:Box<U> = Box<U>()}".into()),
    ];
    let errors = hgl_program::diagnostics(&sources);
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn type_position_defaults_remain_deferred_and_preserve_error_origin() {
    for declaration in [
        "export struct Deferred<T> {flag:bool=any(set<T>(items:[]))<any(set<T>(items:[]))}",
        "export const fn deferred<T>(value:T,flag:bool=any(set<T>(items:[]))<any(set<T>(items:[])))->bool=>flag",
    ] {
        let source = format!("module defaults\n{declaration}");
        assert!(hgl_program::diagnostics(&[("defaults.hgl".into(), source.clone())]).is_empty());
        let target = if declaration.contains("struct") {
            "Deferred<i64>(flag:false).flag"
        } else {
            "deferred(1,flag:false)"
        };
        let imported = if declaration.contains("struct") {
            "Deferred"
        } else {
            "deferred"
        };
        let sources = vec![
            (
                "caller.hgl".into(),
                format!(
                    "module caller\nuse defaults::{{{imported}}}\ntest concrete {{assert !{target}}}"
                ),
            ),
            ("defaults.hgl".into(), source),
        ];
        let errors = hgl_program::diagnostics(&sources);
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].issue.code, Some("value.constant_capability"));
        assert_eq!(
            errors[0].source, "defaults.hgl",
            "{declaration}: {errors:?}"
        );
        let text = &sources[1].1;
        assert_eq!(
            &text[errors[0].issue.span.clone()],
            "any(set<T>(items:[]))<any(set<T>(items:[]))"
        );
    }
}

#[test]
fn nested_constructor_default_type_scope_restores_the_callers_bindings() -> Result<(), String> {
    execute_defaults(&[("defaults.hgl".into(), "module defaults\nstruct Box<T> {value:T}\nstruct Outer<T> {inner:Box<T> = Box<T>(value:7)}\nconst fn keep<T>(value:T)->T {let ignored=Outer<i64>()\nlet retained=Box<T>(value:value)\nreturn retained.value}\ntest scopes {assert keep(true)\nassert keep(3)==3}".into())], 2)
}
