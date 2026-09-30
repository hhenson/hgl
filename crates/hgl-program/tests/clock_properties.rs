//! Clock property syntax and injected receiver checking.
use hgl_program::compile_tests;

fn sources(definition: &str) -> Vec<(String, String)> {
    vec![
        (
            "clock.hgl".into(),
            format!("module example\n{definition}\ntest properties {{ eval(f,[1]) }}"),
        ),
        (
            "replay.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ]
}

#[test]
fn properties_reject_calls_writes_and_capability_escapes() {
    for (body, diagnostic) in [
        (
            "return clock.evaluation_time()",
            "properties cannot be invoked",
        ),
        (
            "return clock.next_cycle_evaluation_time()",
            "properties cannot be invoked",
        ),
        ("return clock.now()", "properties cannot be invoked"),
        (
            "return evaluation_time(clock)",
            "unknown capability operation",
        ),
        (
            "return next_cycle_evaluation_time(clock)",
            "unknown capability operation",
        ),
        ("return now(clock)", "unknown capability operation"),
        ("return clock.lag", "unknown property lag"),
        (
            "return clock.now",
            "wall-clock observations are not supported",
        ),
        ("clock.evaluation_time = @1970-01-01T00:00:00Z", "read-only"),
        ("clock.next_cycle_evaluation_time += 1us", "read-only"),
        ("clock.now = @1970-01-01T00:00:00Z", "read-only"),
        (
            "let alias = clock\nreturn alias.evaluation_time",
            "cannot escape",
        ),
        (
            "let clock = value\nreturn clock.evaluation_time",
            "missing inject clock",
        ),
        ("return value.evaluation_time", "direct injected clock"),
        (
            "return clock::evaluation_time",
            "unknown value clock::evaluation_time",
        ),
    ] {
        let input = sources(&format!(
            "fn f(value:i64)->datetime {{ inject clock\nwhen {{ {body} }} }}"
        ));
        let error = compile_tests(&input).unwrap_err();
        assert!(error.contains(diagnostic), "{body}: {error}");
    }
}

#[test]
fn spelling_does_not_supply_an_injected_receiver_or_global_property() {
    for definition in [
        "fn f(value:i64)->datetime { when { return clock.evaluation_time } }",
        "fn f(clock:i64)->datetime { when { return clock.evaluation_time } }",
        "fn f(value:i64)->datetime { inject clock\nwhen { return evaluation_time } }",
    ] {
        assert!(compile_tests(&sources(definition)).is_err(), "{definition}");
    }
}

#[test]
fn ordinary_value_and_function_names_are_not_reserved_clock_aliases() {
    for definition in [
        "fn f(value:i64)->datetime { inject clock\nwhen { let evaluation_time = clock.evaluation_time\nlet now = evaluation_time\nreturn now } }",
        "native const fn evaluation_time(value:i64)->datetime\nnative const fn evaluation_time(value:i64)->datetime {}\nfn f(value:i64)->datetime { when { return evaluation_time(value) } }",
    ] {
        let result = compile_tests(&sources(definition));
        assert!(result.is_ok(), "{definition}: {result:?}");
    }
}
