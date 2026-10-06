//! Scalar-key source formation keeps exact types and defers provider identity.
use hgl_program::compile_tests;
fn checked(body: &str) -> Result<(), String> {
    let source = format!(
        "module keys\nenum E {{first=-7, second=11}}\nenum Other {{first=-7}}\ntest data {{{body}}}"
    );
    compile_tests(&[("keys.hgl".into(), source)]).map(|_| ())
}
#[test]
fn every_scalar_and_enum_forms_keyed_map_and_set_data() {
    for (ty, member) in [
        ("bool", "false"),
        ("i64", "-7"),
        ("f64", "-1.5"),
        ("str", "\"a: b\""),
        ("date", "@2024-02-29"),
        ("time", "@12:30"),
        ("datetime", "@2026-01-01T12:30Z"),
        ("duration", "1us"),
        ("civil_datetime", "@2026-01-01T12:30"),
        ("timezone", "@[UTC]"),
        ("zoned_time", "@12:30[UTC]"),
        ("zoned_datetime", "@2026-01-01T12:30Z[UTC]"),
        ("E", "E::first"),
    ] {
        let body = format!(
            "let set=delta<set<{ty}>>(added:[{member}])\nlet map=delta<map<{ty},i64>>(upsert:[{member}: 0])"
        );
        let result = checked(&body);
        assert!(result.is_ok(), "{ty}: {result:?}");
    }
}
#[test]
fn known_duplicates_overlap_and_wrong_types_fail_during_checking() {
    for (body, message) in [
        ("let p=delta<set<f64>>(added:[0.0,-0.0])", "duplicate"),
        (
            "let p=delta<map<f64,i64>>(upsert:[0.0:1],remove:[-0.0])",
            "overlap",
        ),
        ("let p=delta<map<f64,i64>>(upsert:[1:1])", "type mismatch"),
        ("let p=delta<set<E>>(added:[Other::first])", "type mismatch"),
        ("let p=delta<map<E,i64>>(upsert:[-7:1])", "type mismatch"),
        (
            "let p=delta<map<str,i64>>(upsert:[\"same\":1,\"same\":2])",
            "duplicate",
        ),
        ("let p=delta<tuple<i64,i64>>(items:[0.0:1])", "constant i64"),
    ] {
        let error = checked(body).unwrap_err();
        assert!(error.contains(message), "{body}: {error}");
    }
}
#[test]
fn provider_names_do_not_establish_checking_time_key_identity() {
    for body in [
        "let p=delta<set<timezone>>(added:[@[UTC],@[UTC]])",
        "let p=delta<map<timezone,i64>>(upsert:[@[UTC]:1],remove:[@[UTC]])",
        "let p=delta<set<timezone>>(added:[@[Missing/Zone]])",
    ] {
        let result = checked(body);
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn immutable_key_aliases_preserve_exact_identity() {
    for body in [
        "let key=@[UTC]\nlet alias=key\nlet p=delta<map<timezone,i64>>(upsert:[alias:1])",
        "let key=1\nlet alias=key\nlet p=delta<set<i64>>(added:[alias])",
        "let key=E::first\nlet alias=key\nlet p=delta<set<E>>(added:[alias])",
    ] {
        assert!(checked(body).is_ok(), "{body}");
    }
    for (body, message) in [
        (
            "var key=1\nlet alias=key\nlet p=delta<set<i64>>(added:[alias])",
            "constant",
        ),
        (
            "let key=0.0\nlet alias=key\nlet p=delta<set<f64>>(added:[alias,-0.0])",
            "duplicate",
        ),
        (
            "let key=1\nlet p=delta<map<f64,i64>>(upsert:[key:1])",
            "type mismatch",
        ),
        (
            "let key=1\nlet p=delta<map<i64,i64>>(upsert:[key:1],remove:[key])",
            "overlap",
        ),
        (
            "let key=1\nif true {var key=2\nlet p=delta<set<i64>>(added:[key])}",
            "constant",
        ),
    ] {
        let error = checked(body).unwrap_err();
        assert!(error.contains(message), "{body}: {error}");
    }
}
