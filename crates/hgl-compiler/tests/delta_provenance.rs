//! Publication requirements retain their written context after scalar reduction.
fn errors(text: &str) -> Vec<hgl_source::diagnostics::Diagnostic> {
    hgl_program::diagnostics(&[("provenance.hgl".into(), format!("module controls\n{text}"))])
}
#[test]
fn written_scalar_delta_parameter_and_result_requirements_are_coded() {
    for text in [
        "const fn need(value:delta<i64>)->i64=>value\ntest bad {let x=need(false)}",
        "const fn bad()->delta<i64> =>false",
        "const fn need<T>(seed:T,value:delta<T>)->T=>seed\ntest bad {let x=need(1,false)}",
        "const fn bad<T>(seed:T)->delta<T> =>false\ntest bad {let x=bad(1)}",
    ] {
        let diagnostics = errors(text);
        assert_eq!(diagnostics.len(), 1, "{text}: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].issue.code,
            Some("delta.type_mismatch"),
            "{text}: {diagnostics:?}"
        );
    }
}
#[test]
fn concrete_unsupported_nested_type_locates_written_child() {
    for annotation in [
        "delta<\n signal\n>",
        "list<delta<\n signal\n>>",
        "tuple<i64,delta<\n ref<i64>\n>>",
    ] {
        let text = format!("fn unused(value:{annotation}) {{when {{}}}}");
        let diagnostics = errors(&text);
        assert_eq!(diagnostics.len(), 1, "{text}: {diagnostics:?}");
        assert_eq!(diagnostics[0].issue.code, Some("delta.unsupported_shape"));
        assert_eq!(diagnostics[0].line, 3, "{text}: {diagnostics:?}");
    }
}
#[test]
fn formation_precedes_destination_and_entry_types_retain_formation_code() {
    for (text, code) in [
        (
            "test bad {let x:delta<list<i64,3>> = delta<list<i64,2>>(items:[2:1])}",
            "delta.index_bounds",
        ),
        (
            "test bad {let x:delta<set<i64>> = delta<set<i64>>(added:[false])}",
            "delta.entry_type",
        ),
    ] {
        let diagnostics = errors(text);
        assert_eq!(diagnostics.len(), 1, "{text}: {diagnostics:?}");
        assert_eq!(
            diagnostics[0].issue.code,
            Some(code),
            "{text}: {diagnostics:?}"
        );
    }
}
#[test]
fn ordinary_scalar_destination_does_not_inherit_delta_requirement() {
    let diagnostics =
        errors("test bad {var source:delta<i64> = 1\nvar ordinary:i64=source\nordinary=false}");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_ne!(diagnostics[0].issue.code, Some("delta.type_mismatch"));
}

#[test]
fn family_field_admission_uses_declared_common_schema() {
    let text = "abstract struct Event {label:str}
struct Trade:Event {price:i64}
struct Message:Event {message:str}
const fn wrong(value:Event)->i64=>value.price\ntest field {let value:Event=Trade(label:\"t\",price:1)\nlet x=wrong(value)}";
    let diagnostics = errors(text);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(
        diagnostics[0].issue.message.contains("field"),
        "{diagnostics:?}"
    );
    let diagnostics = errors(&text.replace("->i64=>value.price", "->str=>value.label"));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}
