//! A typed graph declaration gains a connection only through real assignment.
use hgl_program::{compile, emit_rust};
fn checked(body: &str) -> Result<String, String> {
    let source = format!(
        "module declaration\nfn source()->i64 {{yield 0us:7}}\nfn route(value:ref<i64>)->ref<i64> {{when {{return value}}}}\nfn select(value:i64)->ref<i64> {{ {body} }}\nfn main()->ref<i64> => select(source())"
    );
    compile(&[("declaration.hgl".into(), source)], "main").map(|program| emit_rust(&program))
}
#[test]
fn direct_assignment_creates_the_first_real_reference_binding() {
    let code = checked("var selected:ref<i64>\nselected = route(value)\nselected").unwrap();
    let initialized = checked("var selected:ref<i64> = route(value)\nselected").unwrap();
    assert_eq!(code, initialized);
}
#[test]
fn typed_declaration_and_compound_assignment_cannot_supply_an_initial_value() {
    for body in [
        "var selected:ref<i64>\nselected",
        "var selected:ref<i64>\nselected += route(value)",
        "var selected:ref<i64>\nselected = selected",
    ] {
        let error = checked(body).unwrap_err();
        assert!(error.contains("before definite assignment"), "{error}");
    }
}
#[test]
fn first_assignment_checks_type_and_duplicate_declarations() {
    let error = checked("var selected:ref<str>\nselected = route(value)\nselected").unwrap_err();
    assert!(error.contains("type mismatch"), "{error}");
    let error =
        checked("var selected:ref<i64>\nvar selected:ref<i64>\nselected = route(value)\nselected")
            .unwrap_err();
    assert!(error.contains("duplicate local selected"), "{error}");
}
