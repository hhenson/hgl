//! Rejection matching must preserve normal semantic failures after resolution.
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
fn fixture(text: &str) -> Result<(), String> {
    let path = std::env::temp_dir().join(format!(
        "hgl-rejection-{}-{}.hgl",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    let sources = vec![(path.display().to_string(), text.into())];
    let plan = hgl_reject::Plan::prepare(sources, 1)?;
    let result = hgl_program::compile_suite(&plan.sources).and_then(|_| {
        plan.check()
            .into_iter()
            .try_for_each(|outcome| outcome.result)
    });
    std::fs::remove_file(path).map_err(|e| e.to_string())?;
    result
}
#[test]
fn resolved_constant_function_bound_keeps_rolling_code() -> Result<(), String> {
    fixture(
        "module sample\nconst fn period() -> duration => 5m\n# expect-error(type, \"rolling.size_kind\")\nfn consume(value: rolling<f64, period(), 3>) { when { } }\n",
    )
}
#[test]
fn instantiated_generic_yield_keeps_original_operand_location() {
    let sources=vec![("generic.hgl".into(),"module sample\nfn source<T>(const at:T)->i64 {\n yield at:1\n}\nfn main()->i64 {source(1)}\n".into())];
    let errors = hgl_program::diagnostics(&sources);
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].source, "generic.hgl");
    assert_eq!(errors[0].line, 3);
    assert_eq!(errors[0].issue.category, "type");
    assert_eq!(errors[0].issue.code, Some("yield.time_type"));
}

#[test]
fn unexpected_uncoded_test_error_is_not_hidden_by_expected_type_error() {
    let result = fixture(
        "module sample\n# expect-error(type, \"rolling.size_kind\")\nfn consume(value: rolling<f64, 5m, 3>) { when { } }\ntest additional_error { assert 1 }\n",
    );
    assert!(
        result.is_err(),
        "uncoded semantic failure must remain unexpected"
    );
}

#[test]
fn valid_constant_function_bound_is_not_rejected() {
    let sources = vec![("valid.hgl".into(), "module sample\nconst fn period() -> duration => 5m\nfn consume(value: rolling<f64, period(), 3m>) { when { } }\n".into())];
    let errors = hgl_program::diagnostics(&sources);
    assert!(errors.is_empty(), "{errors:?}");
}
#[test]
fn valid_generic_duration_yield_compiles() -> Result<(), String> {
    let sources = vec![("valid.hgl".into(), "module sample\nfn source<T>(const at: T) -> i64 { yield at: 1 }\nfn main() -> i64 { source(1us) }\n".into())];
    let errors = hgl_program::diagnostics(&sources);
    assert!(errors.is_empty(), "{errors:?}");
    hgl_program::compile(&sources, "main").map(|_| ())
}
#[test]
fn nominal_field_yield_type_keeps_operand_origin() -> Result<(), String> {
    fixture(
        "module sample\nstruct Item { time: i64 }\nfn source(const item: Item) -> i64 {\n # expect-error(type, \"yield.time_type\")\n yield item.time: 1\n}\n",
    )
}
