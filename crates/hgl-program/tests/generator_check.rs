//! Pinned ADR0015 source admission and phase diagnostics.
use hgl_program::compile;

fn source(declaration: &str, entry: &str) -> String {
    format!("module generator_check\n{declaration}\nexport fn main() {{{entry}}}")
}
fn checked(declaration: &str, entry: &str) -> Result<(), String> {
    compile(
        &[("generator_check.hgl".into(), source(declaration, entry))],
        "main",
    )
    .map(|_| ())
}

#[test]
fn generators_are_recognized_at_any_admitted_statement_depth() {
    for body in [
        "yield 0us:1",
        "if true {yield 0us:1} else {yield 1us:2}",
        "var i=0\nwhile i<2 {yield 1us:i\ni+=1}",
        "while {if true {yield 1us:1\nreturn}}",
        "var value=1\nif true {let value=2\nyield 1us:value}\nyield 1us:value",
        "let alarm=1\nlet scheduler=2\nlet out=3\nyield 0us:alarm+scheduler+out",
        "inject clock,logger\ninfo(logger,\"running\")\nyield clock.evaluation_time:1",
    ] {
        let result = checked(&format!("fn produce()->i64 {{{body}}}"), "produce()");
        assert!(result.is_ok(), "{body}: {result:?}");
    }
}

#[test]
fn all_eight_scalar_outputs_and_configuration_are_admitted() {
    for (ty, value) in [
        ("bool", "true"),
        ("i64", "1"),
        ("f64", "1.5"),
        ("str", "\"text\""),
        ("duration", "1us"),
        ("date", "@2026-10-03"),
        ("time", "@12:34:56"),
        ("datetime", "@2026-10-03T12:34:56Z"),
    ] {
        let declaration = "fn produce<T>(const payload:T)->T {yield 1us:payload}";
        let result = checked(declaration, &format!("produce({value})"));
        assert!(result.is_ok(), "{ty}: {result:?}");
    }
    let result = checked(
        "struct TimedValue<T>{time:datetime\nvalue:T}\nfn produce(const values:list<TimedValue<i64>>)->i64 {var index=0\nwhile index<len(values) {yield values[index].time:values[index].value\nindex+=1}}",
        "var values:list<TimedValue<i64>> =[]\npush(values,TimedValue(time:@2026-10-03T00:00:00Z,value:1))\nproduce(values)",
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn generator_context_rejects_each_forbidden_construct() {
    for (body, fragment) in [
        ("state x:i64=1\nyield 0us:x", "does not admit state"),
        ("cache x:i64=1\nyield 0us:x", "does not admit cache"),
        ("when {yield 0us:1}", "does not admit when"),
        ("start {}\nyield 0us:1", "does not admit start"),
        ("stop {}\nyield 0us:1", "does not admit stop"),
        ("inject out\nyield 0us:1", "injectable out"),
        ("inject alarm\nyield 0us:1", "injectable alarm"),
        ("inject scheduler\nyield 0us:1", "injectable scheduler"),
        (
            "inject global_state\nyield 0us:1",
            "injectable global_state",
        ),
        (
            "inject replay_input\nyield 0us:1",
            "injectable replay_input",
        ),
        (
            "for value in values {yield 0us:value}",
            "does not admit for",
        ),
        ("yield 0us:1\nreturn 2", "return cannot carry a value"),
        ("yield 1:1", "yield time requires datetime or duration"),
        ("yield 0us:true", "yield payload type mismatch"),
        ("while 1 {yield 0us:1}", "while condition requires bool"),
    ] {
        let error = checked(&format!("fn produce()->i64 {{{body}}}"), "produce()").unwrap_err();
        assert!(error.contains(fragment), "{body}: {error}");
    }
    let error = checked(
        "fn input()->i64{yield 0us:1}\nfn produce(value:i64)->i64{yield 0us:value}",
        "produce(input())",
    )
    .unwrap_err();
    assert!(
        error.contains("generator sources cannot have temporal parameters"),
        "{error}"
    );
    for declaration in [
        "fn produce(){yield 0us:1}",
        "fn produce()->list<i64>{yield 0us:[]}",
    ] {
        let error = checked(declaration, "produce()").unwrap_err();
        assert!(
            error.contains("generator requires a declared publication output type"),
            "{error}"
        );
    }
}

#[test]
fn while_does_not_silently_change_composition_or_const_phase() {
    for declaration in [
        "fn produce(){while false {}}",
        "fn produce(){if true {while false {}}}",
        "const fn produce()->i64{while false {}\nreturn 1}",
        "const fn produce()->i64{yield 0us:1}",
    ] {
        let error = checked(declaration, "produce()").unwrap_err();
        assert!(
            error.contains("runtime body") && (error.contains("while") || error.contains("yield")),
            "{error}"
        );
    }
    let result = checked(
        "fn produce()->i64{inject alarm\nstart{schedule(alarm,0us)}\nwhen{var value=0\nwhile value<2 {value+=1}\nreturn value}}",
        "produce()",
    );
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn time_operand_arithmetic_uses_only_the_admitted_checked_table() {
    for time in [
        "at+delay",
        "delay+at",
        "at-delay",
        "at-at",
        "delay+delay",
        "delay-delay",
        "-delay",
    ] {
        let result = checked(
            &format!("fn produce(const at:datetime,const delay:duration)->i64{{yield {time}:1}}"),
            "produce(@2026-10-03T00:00:00Z,1us)",
        );
        assert!(result.is_ok(), "{time}: {result:?}");
    }
    let error = checked(
        "fn produce(const at:datetime)->i64{yield at+at:1}",
        "produce(@2026-10-03T00:00:00Z)",
    )
    .unwrap_err();
    assert!(error.contains("unsupported binary operation +"), "{error}");
}

#[test]
fn text_containing_yield_does_not_select_generator_phase() {
    for (declaration, entry) in [
        ("", "let text=\"yield\""),
        ("const fn text()->str{return \"yield\"}", "let value=text()"),
        (
            "fn text()->str{inject alarm\nstart{schedule(alarm,0us)}\nwhen{return \"yield\"}}",
            "text()",
        ),
    ] {
        let result = checked(declaration, entry);
        assert!(result.is_ok(), "{declaration}: {result:?}");
    }
}

#[test]
fn runtime_helpers_propagate_admitted_services_without_creating_nodes() {
    let helpers = "const fn mark(value:i64)->i64{inject logger\ninfo(logger,\"payload\")\nreturn value}\nconst fn nested(value:i64)->i64{return mark(value)}\nconst fn current()->datetime{inject clock\nreturn clock.evaluation_time}";
    let result = checked(
        &format!("{helpers}\nfn produce()->i64{{yield current():nested(1)}}"),
        "produce()",
    );
    assert!(result.is_ok(), "{result:?}");
    let error = checked(helpers, "let value=nested(1)").unwrap_err();
    assert!(
        error.contains("requires a supported runtime service context"),
        "{error}"
    );
    for service in ["out", "alarm", "scheduler", "global_state", "replay_input"] {
        let error = checked(&format!("const fn helper()->i64{{inject {service}\nreturn 1}}\nfn produce()->i64{{yield 0us:helper()}}"), "produce()").unwrap_err();
        assert!(
            error.contains(&format!("ordinary helper injectable {service}")),
            "{error}"
        );
    }
}
