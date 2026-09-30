//! The real library is indexed unchanged; reachable contracts are checked.
#[cfg(test)]
#[path = "support/helpers.rs"]
mod support;
use hgl_program::{compile, emit_rust};
use support::{main_source, sources};

#[test]
fn imports_defaults_and_named_arguments_select_library_bodies() {
    let source = main_source(
        "debug_print(\"answer\", const(42, delay: 2us), print_delta: false, sample: 2)",
    );
    let program = compile(&sources(&source), "main").unwrap();
    let code = emit_rust(&program);
    assert!(code.contains("Publish ``value`` once"));
    assert!(code.contains("hgraph.std::const"));
    assert!(code.contains("hgraph.std::debug_print"));
    assert!(code.contains("fn as_str_i64"));
    assert!(code.contains("fn print_line_str"));
}

#[test]
fn invalid_calls_fail_before_emission() {
    for (body, message) in [
        ("debug_print(\"bad\", 42)", "temporal argument"),
        ("debug_print(42, const(1))", "type mismatch"),
        (
            "debug_print(\"bad\", const(42), missing: 2)",
            "unknown argument",
        ),
        (
            "debug_print(\"bad\", const(42), sample: 2, sample: 3)",
            "duplicate",
        ),
        ("debug_print(\"bad\", const(42, delay: 2))", "type mismatch"),
        (
            "debug_print(\"bad\", const(42, delay: -1us))",
            "negative alarm",
        ),
        ("debug_print(\"bad\", const(const(42)))", "fixed argument"),
        (
            "debug_print(\"bad\", const(42), sample: true)",
            "type mismatch",
        ),
    ] {
        let error = compile(&sources(&main_source(body)), "main").unwrap_err();
        assert!(error.contains(message), "{body}: {error}");
    }
}

#[test]
fn native_parts_and_instantiations_are_required() {
    let main = main_source("debug_print(\"answer\", const(42))");
    let mut input = sources(&main);
    input.remove(1);
    assert!(
        compile(&input, "main")
            .unwrap_err()
            .contains("selected native")
    );
    let mut input = sources(&main);
    input[1].1 = input[1].1.replace("value: i64", "other: i64");
    assert!(
        compile(&input, "main")
            .unwrap_err()
            .contains("selected native")
    );
    let mut input = sources(&main);
    for (_, s) in &mut input {
        *s = s.replace("const<i64>", "const<unsupported>");
    }
    assert!(
        compile(&input, "main")
            .unwrap_err()
            .contains("instantiated implementation")
    );
}

#[test]
fn source_alarm_rules_are_checked() {
    for (body, message) in [
        ("inject alarm\nwhen scheduled() { return 1 }", "expected {"),
        (
            "start { alarm.schedule(0s) }\nwhen { return 1 }",
            "inject alarm",
        ),
        ("inject alarm\nwhen { return true }", "return type"),
    ] {
        let source = format!(
            "module test\nfn source() -> i64 {{\n{body}\n}}\nexport fn main() {{ source() }}"
        );
        assert!(
            compile(&[("test.hgl".into(), source)], "main")
                .unwrap_err()
                .contains(message)
        );
    }
    let main = main_source("debug_print(\"answer\", const(42))");
    let mut input = sources(&main);
    for (_, s) in &mut input {
        *s=s.replace("impl fn debug_print(const label: str, ts: i64, const print_delta: bool = true, const sample: i64 = -1) {", "impl fn debug_print(const label: str, ts: i64, const print_delta: bool = true, const sample: i64 = -1) {\n inject alarm");
    }
    assert!(
        compile(&input, "main")
            .unwrap_err()
            .contains("alarm is admitted only on sources")
    );
}

#[test]
fn unsupported_set_shapes_fail_before_emission() {
    for ty in [
        "set<ref<i64>>",
        "set<set<i64>>",
        "set<void>",
        "set<str>",
        "set<f64>",
        "ref<set<ref<i64>>>",
    ] {
        let text = format!(
            "module example\nfn invalid() -> {ty} {{ when {{}} }}\nexport fn main() {{ invalid() }}"
        );
        let error = compile(&[("invalid.hgl".into(), text)], "main").unwrap_err();
        assert!(
            error.contains("set elements currently require bool or i64"),
            "{ty}: {error}"
        );
    }
}

#[test]
fn set_equality_is_rejected_before_scalar_lowering() {
    for ty in ["i64", "bool"] {
        for op in ["==", "!="] {
            let text = format!(
                "module example\nfn source() -> set<{ty}> {{ when {{}} }}\nfn compare(lhs: set<{ty}>, rhs: set<{ty}>) -> bool {{ when {{ return lhs {op} rhs }} }}\nexport fn main() {{ compare(source(), source()) }}"
            );
            let error = compile(&[("set.hgl".into(), text)], "main").unwrap_err();
            assert!(
                error.contains("unsupported binary operation"),
                "{ty}: {error}"
            );
        }
    }
}

#[test]
fn increment_checks_both_state_and_operand_types() {
    for kind in ["state", "cache"] {
        for (ty, initial, operand) in [
            ("str", "\"x\"", "1"),
            ("f64", "1.0", "1"),
            ("bool", "true", "1"),
            ("duration", "1us", "1"),
            ("date", "@2026-01-01", "1"),
            ("time", "@12:00", "1"),
            ("datetime", "@2026-01-01T12:00Z", "1"),
            ("i64", "0", "1.0"),
        ] {
            let text = format!(
                "module example\nfn bad() {{ {kind} saved: {ty} = {initial}\nwhen {{ saved += {operand} }} }}\nexport fn main() {{ bad() }}"
            );
            let error = compile(&[("state.hgl".into(), text)], "main").unwrap_err();
            assert!(
                error.contains("cache increment requires i64 target and value"),
                "{kind}/{ty}: {error}"
            );
        }
    }
}

#[test]
fn reference_target_must_match_consumer() {
    let text = "module example\nfn source() -> ref<i64> { when {} }\nfn consume(ts: f64) { when {} }\nexport fn main() { consume(source()) }";
    assert!(
        compile(&[("reference.hgl".into(), text.into())], "main")
            .unwrap_err()
            .contains("type mismatch")
    );
}

#[test]
fn nonfinite_literals_fail_before_emission() {
    for literal in ["1e999", "-1e999"] {
        let node = format!(
            "module example\nfn bad() -> f64 {{ when {{ return {literal} }} }}\nexport fn main() {{ bad() }}"
        );
        assert!(
            compile(&[("literal.hgl".into(), node)], "main")
                .unwrap_err()
                .contains("finite f64 range")
        );
        for assertion in [
            format!("assert eval(id, [1.0]) == [{literal}]"),
            format!("assert eval(id, [{literal}]) == [1.0]"),
        ] {
            let input = format!(
                "module example\nfn id(x: f64) -> f64 {{ when {{ return x }} }}\ntest bad {{ {assertion} }}"
            );
            assert!(
                hgl_program::compile_tests(&[("literal.hgl".into(), input)])
                    .unwrap_err()
                    .contains("finite f64 range")
            );
        }
    }
}
