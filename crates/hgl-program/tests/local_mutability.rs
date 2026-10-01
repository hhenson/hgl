//! Scalar binding authority is independent of payload type and lexical spelling.
use hgl_program::compile;

fn check(body: &str) -> Result<(), String> {
    let source = format!(
        "module locals\nfn tick()->i64 {{ inject alarm\nstart {{ schedule(alarm,0s) }}\nwhen {{return 1}} }}\nfn f(input:i64)->i64 {{ when {{ {body} }} }}\nexport fn main()->i64 {{ return f(tick()) }}"
    );
    compile(&[("locals.hgl".into(), source)], "main").map(|_| ())
}

#[test]
fn all_scalar_types_support_exact_owned_assignment() {
    for (ty, first, next) in [
        ("bool", "false", "true"),
        ("i64", "0", "1"),
        ("f64", "0.0", "1.5"),
        ("str", "\"first\"", "\"next\""),
        ("date", "@2026-01-01", "@2026-02-01"),
        ("time", "@00:00:00", "@00:00:01"),
        ("datetime", "@2026-01-01T00:00:00Z", "@2026-02-01T00:00:00Z"),
        ("duration", "0us", "1us"),
    ] {
        let body = format!("var item:{ty}={first}\nitem={next}\nlet saved=item\nreturn 1");
        assert!(check(&body).is_ok(), "{body}");
    }
    for body in [
        "var n=1\nn+=2\nreturn n",
        "var n=1.5\nn+=2\nreturn 1",
        "var n=\"a\"\nn+=\"b\"\nreturn 1",
    ] {
        assert!(check(body).is_ok(), "{body}");
    }
}

#[test]
fn readonly_bindings_wrong_types_and_shadowed_authority_fail() {
    for body in [
        "let n=1\nn=2",
        "let n=1\nn+=2",
        "input=2",
        "input+=2",
        "var n=1\nn=false",
        "var n=1\nn+=1.5",
        "var n=false\nn+=true",
        "var n=1\nlet alias=n\nalias=2",
        "var n=1\nif true { let n=2\nn=3 }",
        "let n=1\nif true { var n=2\nn=3 }\nn=4",
        "var n=1\nif true { var hidden=2 }\nhidden=3",
        "var n:mut i64=1",
    ] {
        assert!(check(body).is_err(), "{body}");
    }
    assert!(check("var n=1\nif true {let n=2}\nn=3\nreturn n").is_ok());
    assert!(check("let n=1\nif true {var n=2\nn+=3}\nreturn n").is_ok());
}

#[test]
fn loop_bindings_and_output_are_not_new_increment_targets() {
    for mutation in ["item=1", "item+=1"] {
        let source = format!(
            "module locals\nfn source()->set<i64> {{when {{}}}}\nfn f(input:set<i64>) {{ when {{ for item in elements(input,added) {{ {mutation} }} }} }}\nexport fn main() {{f(source())}}"
        );
        let error = compile(&[("locals.hgl".into(), source)], "main").unwrap_err();
        assert!(error.contains("writable var"), "{error}");
    }
    let source = "module locals\nfn f()->i64 {inject out\nwhen {out+=1}}\nexport fn main()->i64 {return f()}";
    assert!(
        compile(&[("locals.hgl".into(), source.into())], "main")
            .unwrap_err()
            .contains("writable var")
    );
}

#[test]
fn ordinary_value_bodies_accept_vars_but_not_parameter_assignment() {
    for (body, accepted) in [
        ("var saved=value\nsaved+=2\nreturn saved", true),
        ("value=2\nreturn value", false),
    ] {
        let source = format!(
            "module locals\nfn source()->i64 {{when {{return 1}}}}\nconst fn helper(value:i64)->i64 {{ {body} }}\nexport fn main()->i64 {{return helper(source())}}"
        );
        let result = compile(&[("locals.hgl".into(), source)], "main");
        assert_eq!(result.is_ok(), accepted, "{result:?}");
    }
}

#[test]
fn mut_qualifier_is_not_an_input_or_value_type() {
    for annotation in ["mut i64", "mut atomic<i64>", "atomic<mut i64>"] {
        let source = format!(
            "module locals\nfn source()->i64 {{when {{return 1}}}}\nfn f(value:{annotation})->i64 {{when {{return 1}}}}\nexport fn main()->i64 {{return f(source())}}"
        );
        assert!(
            compile(&[("locals.hgl".into(), source)], "main").is_err(),
            "{annotation}"
        );
    }
}
