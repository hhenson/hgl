//! Structured source-origin diagnostics and conservative declaration admission.
use hgl_semantics::source_check::validate_sources;

fn check(text: &str) -> Vec<hgl_source::diagnostics::Diagnostic> {
    validate_sources(&[("case.hgl".into(), text.into())])
}
#[test]
fn errors_are_emitted_at_rule_origins_in_unreachable_declarations() {
    let cases = [
        (
            "fn bad(value: rolling<f64, 5m,\n 3>) { when { } }",
            "rolling.size_kind",
            3,
        ),
        (
            "fn bad(value: rolling<f64, 3,\n 4>) { when { } }",
            "rolling.size_bounds",
            3,
        ),
        ("fn bad() -> i64 {\n yield 1: 1\n}", "yield.time_type", 3),
        (
            "fn bad(value: i64 -> i64 => value",
            "syntax.expected_token",
            2,
        ),
        ("test bad {\n inject clock\n}", "test.statement_phase", 3),
        (
            "test bad {\n assert raises(\"unknown\") { }\n}",
            "test.raises_code",
            3,
        ),
    ];
    for (body, code, line) in cases {
        let errors = check(&format!("module sample\n{body}\n"));
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert_eq!(errors[0].issue.code, Some(code));
        assert_eq!(errors[0].source, "case.hgl");
        assert_eq!(errors[0].line, line);
    }
}
#[test]
fn test_contexts_preserve_original_source_locations() {
    let errors = check("module sample\n\ntest {\n fn bad() -> i64 {\n  yield 1: 1\n }\n}\n");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].line, 5);
    assert_eq!(errors[0].issue.code, Some("yield.time_type"));
}
#[test]
fn raises_requires_literal_source_form_before_folding() {
    for argument in [
        "(\"yield.negative_duration\")",
        "\"yield.\" + \"negative_duration\"",
        "code",
    ] {
        let errors = check(&format!(
            "module sample\ntest bad {{\n assert raises({argument}) {{ }}\n}}\n"
        ));
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].issue.code, Some("test.raises_code"));
        assert_eq!(errors[0].line, 3);
    }
    let errors = check("module sample\ntest bad { assert raises(\"unknown\", 1) { } }\n");
    assert_eq!(errors[0].issue.code, Some("syntax.expected_token"));
}
#[test]
fn multiple_declarations_preserve_uncatalogued_errors() {
    let errors = check(
        "module sample\nfn first(value: rolling<f64, 5m, 3>) { when { } }\nconst fn second(value: atomic<list<i64>>) -> i64 => 1\n",
    );
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].issue.code, Some("rolling.size_kind"));
    assert_eq!(errors[1].issue.code, None);
}
#[test]
fn valid_generic_and_overloaded_source_is_not_guessed_invalid() {
    let errors = check(
        "module sample\noperator pack<T>(values: ...T) -> T\noperator sized<T, const size: i64>(value: list<T, size>) -> T\nconst fn delay(value: str) -> i64 => 1\nconst fn delay(value: i64) -> duration => 1us\nfn valid_source() -> i64 { yield delay(1): 1 }\n",
    );
    assert!(errors.is_empty(), "{errors:?}");
}
#[test]
fn nested_type_error_uses_the_nested_size_argument_line() {
    let errors =
        check("module sample\nfn bad(value: tuple<i64, rolling<f64, 5m,\n 3>>) { when { } }\n");
    assert_eq!(errors[0].issue.code, Some("rolling.size_kind"));
    assert_eq!(errors[0].line, 3);
}

#[test]
fn nominal_field_time_type_uses_resolved_schema() {
    let errors = check(
        "module sample\nstruct Item { time: i64 }\nfn bad(const item: Item) -> i64 {\n yield item.time: 1\n}\n",
    );
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert_eq!(errors[0].issue.code, Some("yield.time_type"));
    assert_eq!(errors[0].line, 4);
}

#[test]
fn missing_identifier_and_context_delimiter_keep_parser_origin() {
    let errors = check("module sample\nfn bad( : i64) -> i64 => 1\n");
    assert_eq!(errors[0].issue.code, Some("syntax.expected_token"));
    assert_eq!(errors[0].line, 2);
    let errors = check("module sample\ntest {\n fn helper() -> i64 => 1\n");
    assert_eq!(errors[0].issue.code, Some("syntax.expected_token"));
    assert_eq!(errors[0].line, 4);
}
