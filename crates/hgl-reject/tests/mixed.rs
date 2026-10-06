//! Mixed rejection ownership, selection and dependency admission.
use hgl_reject::Plan;
fn prepare(text: &str) -> Result<Plan, String> {
    Plan::prepare(vec![("mixed.hgl".into(), text.into())], 1)
}
#[test]
fn owned_test_is_excluded_including_statements_before_annotation() {
    let plan = prepare(
        "module m\ntest reject {assert false\n# expect-error(phase, \"test.statement_phase\")\ninject clock\n}\ntest live {assert true}\n",
    ).unwrap();
    let suite = hgl_program::compile_suite(&plan.sources).unwrap();
    assert_eq!(suite.tests.len(), 1);
    assert_eq!(suite.tests[0].name, "m::live");
    let outcomes = plan.check();
    assert_eq!(outcomes.len(), 1);
    assert!(outcomes[0].result.is_ok(), "{outcomes:?}");
}
#[test]
fn unselected_named_rejections_stay_excluded_but_metadata_is_global() {
    let mut plan = prepare(
        "module m\ntest wrong {\n# expect-error(type, \"rolling.size_kind\")\nassert true\n}\ntest live {assert true}\n",
    ).unwrap();
    plan.select(&["live"], &["m::live".into()]).unwrap();
    assert!(plan.check().is_empty());
    assert!(!plan.sources[0].1.contains("test wrong"));
    assert!(
        Plan::prepare(
            vec![(
                "x.hgl".into(),
                "module m\ntest wrong {\n# expect-error(type, \"unknown\")\nassert true\n}".into()
            )],
            1
        )
        .is_err()
    );
}
#[test]
fn orphaned_and_context_header_annotations_fail_admission() {
    for text in [
        "# expect-error(parse, \"syntax.expected_token\")\nmodule m\n",
        "module m\n# expect-error(parse, \"syntax.expected_token\")\ntest {fn helper()->i64=>1}\n",
        "module m\n# expect-error(type, \"rolling.size_kind\")\n\nfn f(x:i64){}\n",
    ] {
        assert!(Plan::prepare(vec![("x.hgl".into(), text.into())], 1).is_err());
    }
}
#[test]
fn imported_annotations_never_suppress_dependency_errors() {
    let sources=vec![("root.hgl".into(),"module root\ntest live {assert true}\n".into()),("dependency.hgl".into(),"module dependency\n# expect-error(type, \"rolling.size_kind\")\nfn bad(value:rolling<f64,5m,3>){when{}}\n".into())];
    let plan = Plan::prepare(sources, 1).unwrap();
    assert!(plan.check().is_empty());
    assert!(hgl_program::check_sources(&plan.sources).is_err());
}
#[test]
fn concrete_parameterized_survivor_cannot_reference_excluded_function() {
    for caller in [
        "fn caller(value:i64)->i64=>excluded()",
        "const fn caller(value:i64)->i64=>excluded()",
        "fn caller<T>(value:T)->T=>excluded(value)",
        "fn caller(const setting:i64)->i64=>excluded()",
        "fn caller<T>(value:T,const unused:i64=excluded())->T=>value",
        "fn caller(const first:i64,const unused:i64=excluded())->i64 {when{return first}}",
        "fn caller<T>(value:list<T,excluded()>)->list<T,excluded()>=>value",
        "struct NeedsBound {items:list<i64,excluded()>}",
        "fn caller<T>(value:T)->T {let saved:list<i64,excluded()> = [1]\nreturn value}",
    ] {
        let plan = prepare(&format!(
            "module m\nfn excluded()->i64 {{\n# expect-error(type, \"yield.time_type\")\nyield 1:1\n}}\n{caller}\ntest must_not_run {{assert true}}\n"
        )).unwrap();
        let diagnostics = hgl_program::diagnostics(&plan.sources);
        assert!(!diagnostics.is_empty(), "admitted {caller}");
        assert!(
            diagnostics.iter().any(|d| d.issue.code.is_none()),
            "missing dependency must remain uncoded for {caller}: {diagnostics:?}"
        );
    }
}
#[test]
fn each_probe_sees_no_other_rejection_owner() {
    let plan = prepare(
        "module m\n# expect-error(type, \"rolling.size_kind\")\nfn other(value:rolling<f64,5m,3>){when{}}\n# expect-error(type, \"yield.time_type\")\nfn caller()->i64=>other()\n",
    ).unwrap();
    let outcomes = plan.check();
    assert_eq!(outcomes.len(), 2);
    assert!(outcomes[0].result.is_ok(), "{outcomes:?}");
    assert!(
        outcomes[1].result.is_err(),
        "excluded dependency cannot stand in for expected error"
    );
}

#[test]
fn deferred_valid_calls_and_value_shadowing_remain_admitted() {
    let plan = prepare("module m\n# expect-error(type, \"rolling.size_kind\")\nfn excluded(value:rolling<f64,5m,3>){when{}}\nfn identity<T>(value:T)->T=>value\nfn survivor<T>(value:T)->T { let excluded=value\nreturn identity(excluded) }\nfn configured(const setting:i64)->i64 {when {return setting}}\nfn defaulted(const setting:i64=1)->i64 {when {return setting}}\ntest live {assert true}\n").unwrap();
    assert!(
        hgl_program::diagnostics(&plan.sources).is_empty(),
        "{:?}",
        hgl_program::diagnostics(&plan.sources)
    );
    assert!(plan.check().iter().all(|case| case.result.is_ok()));
}
#[test]
fn deferred_names_resolve_imports_and_test_context_shadowing() {
    let sources = vec![("main.hgl".into(), "module m\nuse helper as h\nfn caller<T>(value:T)->T=>h::identity(value)\nfn identity<T>(value:T)->T=>value\ntest { fn identity<T>(value:T)->T=>value\nfn caller<T>(value:T)->T=>identity(value)\ntest live {assert true} }\n".into()),("helper.hgl".into(),"module helper\nexport fn identity<T>(value:T)->T=>value\n".into())];
    assert!(
        hgl_program::diagnostics(&sources).is_empty(),
        "{:?}",
        hgl_program::diagnostics(&sources)
    );
}

#[test]
fn nominal_signature_dependency_fails_before_rejection_matching() {
    let plan = prepare("module m\n# expect-error(type, \"rolling.size_kind\")\nstruct Excluded {value:i64}\nfn survivor<T>(value:Excluded,fallback:T)->T=>fallback\ntest live {assert true}\n").unwrap();
    let errors = hgl_program::diagnostics(&plan.sources);
    assert!(errors.iter().any(|d| {
        d.issue
            .message
            .contains("unresolved ordinary type Excluded")
    }));
}
#[test]
fn signals_resolved_native_intrinsics_and_implicit_void_remain_valid() {
    let sources=vec![("control.hgl".into(),"module m\nuse hgraph.native as native\nfn observe(value:signal)->bool {when{return valid(value)}}\nfn f<T>(x:set<T>)->bool {when{return native::bound(x)}}\nfn sink(value:i64){when{}}\n".into())];
    assert!(
        hgl_program::diagnostics(&sources).is_empty(),
        "{:?}",
        hgl_program::diagnostics(&sources)
    );
}

#[test]
fn local_nominal_annotation_cannot_reference_an_excluded_type() {
    let plan = prepare("module m\n# expect-error(type, \"rolling.size_kind\")\nstruct Excluded {value:i64}\nfn survivor<T>(x:T)->T {let y:Excluded=x\nreturn x}\ntest live {assert true}\n").unwrap();
    assert!(hgl_program::diagnostics(&plan.sources).iter().any(|d| {
        d.issue
            .message
            .contains("unresolved ordinary type Excluded")
    }));
}

#[test]
fn qualified_enum_values_require_surviving_nominal_owner() {
    let plan = prepare("module m\n# expect-error(type, \"rolling.size_kind\")\nenum Excluded {A}\nfn survivor<T>(x:T)->T {let y=Excluded::A\nreturn x}\ntest live {assert true}\n").unwrap();
    assert!(
        hgl_program::diagnostics(&plan.sources)
            .iter()
            .any(|d| d.issue.message.contains("unknown value Excluded::A"))
    );
    let valid=vec![("enum.hgl".into(),"module m\nenum Kept {A}\nfn f<T>(x:T)->T {let y=Kept::A\nreturn x}\nfn shadow<T>(Excluded:T)->T=>Excluded\n".into())];
    assert!(hgl_program::diagnostics(&valid).is_empty());
}

#[test]
fn applied_arguments_and_native_requirement_types_resolve_nominal_names() {
    for function in [
        "fn f<T>(x:T)->T {let y=Box<Excluded>(value:x)\nreturn x}",
        "fn f<T>(x:T)->T requires native::accept(Excluded)->T {return x}",
        "native const fn f<T>(x:Excluded)->T",
        "operator f<T>(x:Excluded)->T",
    ] {
        let plan=prepare(&format!("module m\n# expect-error(type, \"rolling.size_kind\")\nstruct Excluded {{value:i64}}\nstruct Box<T>{{value:T}}\n{function}\ntest live {{assert true}}\n")).unwrap();
        let errors = hgl_program::diagnostics(&plan.sources);
        assert!(
            errors.iter().any(|d| d
                .issue
                .message
                .contains("unresolved ordinary type Excluded")),
            "{function}: {errors:?}"
        );
    }
}
#[test]
fn interface_defaults_resolve_callable_dependencies() {
    for function in [
        "native const fn f(const value:i64=excluded())->i64",
        "operator f(const value:i64=excluded())->i64",
    ] {
        let plan=prepare(&format!("module m\n# expect-error(type, \"yield.time_type\")\nfn excluded()->i64{{yield 1:1}}\n{function}\ntest live {{assert true}}\n")).unwrap();
        let errors = hgl_program::diagnostics(&plan.sources);
        assert!(
            errors
                .iter()
                .any(|d| d.issue.message.contains("unknown callable excluded")),
            "{function}: {errors:?}"
        );
    }
}

#[test]
fn struct_field_defaults_resolve_qualified_enum_members() {
    let plan=prepare("module m\n# expect-error(type, \"rolling.size_kind\")\nenum Excluded {A}\nstruct Box<T>{value:T=Excluded::A}\ntest live {assert true}\n").unwrap();
    assert!(
        hgl_program::diagnostics(&plan.sources)
            .iter()
            .any(|d| d.issue.message.contains("unknown value Excluded::A"))
    );
}
