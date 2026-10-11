//! Generic substitutions precede required defaults and dependent shape formation.
fn execute(source: &str, assertions: usize) -> Result<(), String> {
    let suite = hgl_program::compile_module_suite(&[("dependent.hgl".into(), source.into())])?;
    assert_eq!(suite.tests.len(), 1);
    let count = hgl_semantics::harness::execute(
        &suite.tests[0],
        |_| {
            Err(hgl_semantics::value_eval::EvalError::Unsupported(
                "unexpected context".into(),
            ))
        },
        |_, _| Err("unexpected graph evaluation".into()),
    )?;
    assert_eq!(count, assertions);
    Ok(())
}
#[test]
fn inferred_generic_const_default_forms_later_parameter_size() -> Result<(), String> {
    execute(
        "module dependent\nconst fn width<T>(items:set<T>)->i64=>1\nconst fn sized<T>(value:T,const n:i64=width(set<T>(items:[])),values:list<i64,n> = [9])->i64=>values[0]\ntest sizes {assert sized(true)==9}",
        1,
    )
}
#[test]
fn sized_argument_inference_and_explicit_overrides_keep_exact_shapes() -> Result<(), String> {
    execute(
        "module dependent\nconst fn width<T>(items:set<T>)->i64=>1\nconst fn sized<T>(items:list<T,n>,const n:i64=width(set<T>(items:[])))->T=>items[0]\ntest sizes {let one:list<bool,1> = [true]\nassert sized(one)\nlet two:list<bool,2> = [false,true]\nassert sized(n:2,items:two)==false}",
        2,
    )
}
#[test]
fn inferred_default_failures_retain_declaring_source() {
    let sources = vec![
        ("caller.hgl".into(), "module caller\nuse owner::{sized}\ntest rejected {assert sized(true)==9}".into()),
        ("owner.hgl".into(), "module owner\nconst fn index(value:bool)->i64=>1\nexport const fn sized<T>(value:T,const n:i64=index(any(set<T>(items:[]))<any(set<T>(items:[]))),values:list<i64,n> = [9])->i64=>values[0]".into()),
    ];
    let errors = hgl_program::diagnostics(&sources);
    assert!(!errors.is_empty());
    assert!(
        errors.iter().any(|error| error.source == "owner.hgl"
            && error.line == 3
            && error.issue.code == Some("value.constant_capability")),
        "{errors:?}"
    );
}
#[test]
fn inferred_sizes_do_not_admit_wrong_width_or_element_type() {
    for items in ["list<bool,2> = [true,false]", "list<i64,1> = [1]"] {
        let source = format!(
            "module dependent\nconst fn width<T>(items:set<T>)->i64=>1\nconst fn sized<T>(value:T,const n:i64=width(set<T>(items:[])),items:list<T,n> = [true])->T=>items[0]\ntest rejected {{let input:{items}\nassert sized(true,items:input)}}"
        );
        let error =
            hgl_program::compile_module_suite(&[("dependent.hgl".into(), source)]).unwrap_err();
        assert!(
            error.contains("mismatch") || error.contains("inference"),
            "{error}"
        );
        assert!(!error.contains("unknown value n"), "{error}");
    }
}
