//! Contextual scalar construction stays outside topology and node hooks.
use hgl_program::{compile, compile_tests};

#[test]
fn ordinary_contextual_literals_are_checked_without_provider_lookup() {
    let source = "module example\ntest values { let zone:timezone = @[NoSuch/Zone]\nassert zone == zone\nlet civil:civil_datetime = @2024-02-29T12:30\nassert civil < @2024-02-29T12:31 }";
    assert!(compile_tests(&[("test.hgl".into(), source.into())]).is_ok());
}
#[test]
fn zones_and_zoned_instants_have_equality_but_no_order() {
    for value in ["@[UTC]", "@2026-01-15T12:30Z[Etc/UTC]", "@12:30[UTC]"] {
        let equal = format!("module example\ntest values {{assert {value} == {value}}}");
        assert!(compile_tests(&[("test.hgl".into(), equal)]).is_ok());
        for operator in ["<", ">", "<=", ">="] {
            let ordered =
                format!("module example\ntest values {{assert {value} {operator} {value}}}");
            assert!(compile_tests(&[("test.hgl".into(), ordered)]).is_err());
        }
    }
}
#[test]
fn hook_construction_and_offset_bearing_zoned_time_report_the_supported_boundary() {
    let contextual = "module example\nfn main()->timezone {when {return @[UTC]}}";
    let error = compile(&[("test.hgl".into(), contextual.into())], "main").unwrap_err();
    assert!(
        error.contains("node-hook construction is unsupported"),
        "{error}"
    );
    let excluded = "module example\nfn main()->zoned_time {when {return @12:30Z[UTC]}}";
    assert!(compile(&[("test.hgl".into(), excluded.into())], "main").is_err());
}

#[test]
fn deferred_helper_calls_bind_once_and_keep_written_argument_order() {
    let source = "module example\nconst fn id(const zone:timezone)->timezone {return zone}\nconst fn choose(a:timezone,b:timezone)->timezone {return a}\ntest t {let zone=id(@[UTC])\nlet result=choose(b:@[Missing/First],a:@[Missing/Second])}";
    let suite = compile_tests(&[("test.hgl".into(), source.into())]).unwrap();
    let emitted = hgl_program::emit_tests(&suite);
    assert_eq!(
        emitted
            .matches("TemporalLiteral::TimeZone(\"UTC\".into())")
            .count(),
        1
    );
    assert!(emitted.find("Missing/First").unwrap() < emitted.find("Missing/Second").unwrap());
}
#[test]
fn contextual_defaults_remain_recipes_and_supplied_values_suppress_them() {
    let source = "module example\nstruct Box {zone:timezone = @[Missing/Default]}\nconst fn id(zone:timezone = @[UTC])->timezone {return zone}\ntest t {let a=id()\nlet b=Box(zone:@[Etc/UTC])\nassert a != b.zone}";
    let suite = compile_tests(&[("test.hgl".into(), source.into())]).unwrap();
    let emitted = hgl_program::emit_tests(&suite);
    assert!(emitted.contains("TemporalLiteral::TimeZone(\"UTC\".into())"));
    assert!(!emitted.contains("Missing/Default"));
    let source = "module example\nstruct Box {zone:timezone = @[UTC]}\nfn main()->timezone {when {let value=Box()\nreturn value.zone}}";
    assert!(
        compile(&[("test.hgl".into(), source.into())], "main")
            .unwrap_err()
            .contains("contextual temporal default")
    );
}
#[test]
fn eval_defaults_are_prepared_even_when_unused_and_composition_aliases_propagate() {
    for source in [
        "fn target(const zone:timezone = @[Missing/Default]) {when {}}\ntest t {eval(target)}",
        "fn sink(const zone:timezone) {start {let retained=zone}\nwhen {}}\nfn target(const zone:timezone) {let alias=zone\nsink(alias)}\ntest t {eval(target,zone:@[UTC])}",
    ] {
        let source = format!("module example\n{source}");
        let suite = compile_tests(&[("test.hgl".into(), source)]).unwrap();
        let emitted = hgl_program::emit_tests(&suite);
        assert!(emitted.contains("TemporalLiteral::TimeZone"));
    }
    let source = "module example\nfn sink(const zone:timezone) {when {}}\nfn target() {sink(@[UTC])}\ntest t {eval(target)}";
    assert!(
        compile_tests(&[("test.hgl".into(), source.into())])
            .unwrap_err()
            .contains("requires run preparation")
    );
}

#[test]
fn contextual_composition_defaults_cannot_bypass_the_preparation_boundary() {
    for declarations in [
        "struct Box {zone:timezone = @[UTC]}\nfn sink(const value:Box) {when {}}\nfn wrapper() {sink(Box())}",
        "const fn id(zone:timezone = @[UTC])->timezone {return zone}\nfn sink(const zone:timezone) {when {}}\nfn wrapper() {sink(id())}",
    ] {
        let source = format!("module example\n{declarations}\ntest t {{eval(wrapper)}}");
        let error = compile_tests(&[("test.hgl".into(), source)]).unwrap_err();
        assert!(error.contains("run preparation"), "{error}");
    }
}

#[test]
fn contextual_assertions_defer_whole_expression_and_closed_branches_keep_short_circuiting() {
    let helper = "module example\nconst fn fail()->bool {let values:list<i64> = []\nreturn values[0] == 0}\n";
    for expression in [
        "fail() && @[Missing/Zone] == @[UTC]",
        "@[Missing/Zone] == @[UTC] && fail()",
        "true || fail()",
        "false && fail()",
    ] {
        let source = format!("{helper}test t {{assert {expression}}}");
        assert!(
            compile_tests(&[("test.hgl".into(), source)]).is_ok(),
            "{expression}"
        );
    }
}

#[test]
fn assertions_using_harness_locals_defer_and_dead_constant_branches_are_skipped() {
    for setup in ["let values:list<i64> = []", "var values:list<i64> = []"] {
        let source = format!("module example\ntest t {{{setup}\nassert values[0] == 0}}");
        assert!(compile_tests(&[("test.hgl".into(), source)]).is_ok());
    }
    let source = "module example\nconst fn safe()->bool {if false {let values:list<i64> = []\nreturn values[0] == 0}\nreturn true}\ntest t {assert safe()}";
    assert!(compile_tests(&[("test.hgl".into(), source.into())]).is_ok());
}

#[test]
fn zoned_time_keeps_exact_type_and_existing_publication_restrictions() {
    for body in [
        "let value:time = @09:30[UTC]",
        "let value:zoned_time = @09:30",
        "assert @09:30[UTC] == @[UTC]",
        "let value=@09:30[UTC] + 1h",
        "let value:delta<set<zoned_time>> = {}",
        "let value:delta<map<zoned_time,i64>> = {}",
    ] {
        let source = format!("module example\ntest invalid {{{body}}}");
        assert!(
            compile_tests(&[("test.hgl".into(), source)]).is_err(),
            "{body}"
        );
    }
    let source = "module example\nfn main(value:zoned_time,trigger:i64)->zoned_time {when modified(trigger) {return delta_value(value)}}\ntest bad {eval(main,value:[@09:30[UTC]],trigger:[1])}";
    let error = compile_tests(&[
        ("test.hgl".into(), source.into()),
        (
            "replay_record.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/replay_record.hgl").into(),
        ),
        (
            "replay_record_impl.hgl".into(),
            include_str!("../../../external/hgraph_std/hgl/hgraph/impl/replay_record.hgl").into(),
        ),
    ])
    .unwrap_err();
    assert!(error.contains("valid and modified"), "{error}");
}
