//! Exact atomic boundary admission, inference and ordinary payload checking.
use hgl_program::{compile_tests, emit_tests};

fn sources(body: &str) -> Vec<(String, String)> {
    vec![
        ("atomic.hgl".into(), format!("module atomic\n{body}")),
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
fn reject(body: &str, diagnostic: &str) {
    let error = compile_tests(&sources(body)).err().unwrap_or_default();
    assert!(error.contains(diagnostic), "{error}");
}
#[test]
fn constant_annotations_keep_grammar_before_atomic_normalization() {
    reject(
        "fn target(const value:atomic<i64>)->i64 {when {return value}}\ntest bad {eval(target,value:1)}",
        "value_type",
    );
}
#[test]
fn atomic_scalar_generic_arguments_and_patterns_canonicalize() {
    let suite = compile_tests(&sources(
        "struct Box<T>{value:T}\nfn pass<T>(value:atomic<T>)->T {when {return delta_value(value)}}\nconst fn seed()->i64 {let b:Box<i64> = Box<atomic<i64>>(value:1)\nreturn b.value}\ntest works {assert eval(pass,[seed(),_,2])==[1,_,2]}",
    ));
    assert!(suite.is_ok(), "{suite:?}");
}
#[test]
fn complete_composites_do_not_infer_a_temporal_boundary() {
    reject(
        "use hgraph.std::{TimedValue}\nstruct Payload {value:i64}\nconst fn invalid()->i64 {let sample=TimedValue(time:@1970-01-01T00:00:00.000001Z,value:Payload(value:1))\nreturn 1}\nfn target(value:i64)->i64 {when {return value}}\ntest bad {eval(target,[invalid()])}",
        "unresolved temporal shape",
    );
}
#[test]
fn atomic_shape_profile_rejects_excluded_payloads() {
    for (declaration, shape) in [
        ("", "atomic<ref<i64>>"),
        ("", "atomic<delta<list<i64,2>>>"),
        ("struct Optional {value:ref<i64>=null}", "atomic<Optional>"),
        ("struct Recursive {value:Recursive}", "atomic<Recursive>"),
    ] {
        let body = format!(
            "{declaration}\nfn pass(value:{shape})->{shape} {{when {{return delta_value(value)}}}}\ntest bad {{eval(pass,[])}}"
        );
        assert!(compile_tests(&sources(&body)).is_err(), "{shape}");
    }
}
#[test]
fn generic_occurrences_intersect_and_unused_parameters_stay_ordinary() {
    for declaration in [
        "struct Mixed<T> {value:T\npublication:delta<T>}",
        "struct Unused<T> {value:i64}",
        "struct Publication<T> {value:delta<T>}\nstruct Mixed<T> {values:list<Publication<T>>\nvalue:T}",
    ] {
        let family = if declaration.contains("Unused") {
            "Unused"
        } else {
            "Mixed"
        };
        let body = format!(
            "{declaration}\nconst fn invalid()->i64 {{let x:{family}<atomic<list<i64>>> = null\nreturn 1}}\nfn pass(value:i64)->i64 {{when {{return value}}}}\ntest bad {{eval(pass,[invalid()])}}"
        );
        reject(&body, "ordinary value type");
    }
}
#[test]
fn forwarded_delta_parameters_preserve_exact_atomic_shape() {
    let body = "struct Publication<T>{value:delta<T>}\nstruct Batch<T>{values:list<Publication<T>>}\nfn pass(value:atomic<Batch<atomic<list<i64>>>>)->atomic<Batch<atomic<list<i64>>>> {when {return delta_value(value)}}\ntest works {assert eval(pass,[Batch<atomic<list<i64>>>(values:[])])==[Batch<atomic<list<i64>>>(values:[])]}";
    let suite = compile_tests(&sources(body)).expect("delta-only argument forwards through Batch");
    assert!(emit_tests(&suite).contains("set_atomic"));
}
#[test]
fn empty_nominal_atomic_publications_keep_their_boundary() {
    let body = "struct Empty {}\nfn pass(value:atomic<Empty>)->atomic<Empty> {when {return delta_value(value)}}\ntest works {assert eval(pass,[Empty(),_,Empty(),Empty()])==[Empty(),_,Empty(),Empty()]}";
    let suite = compile_tests(&sources(body)).expect("empty complete nominal is a present value");
    assert!(emit_tests(&suite).contains("shapes::Atomic"));
}
#[test]
fn atomic_delta_constructor_and_unguarded_observation_reject() {
    reject(
        "fn pass(value:atomic<list<i64>>)->atomic<list<i64>> {when {return delta<atomic<list<i64>>>()}}\ntest bad {eval(pass,[[]])}",
        "structural publication shape",
    );
    reject(
        "fn pass(value:atomic<list<i64>>,trigger:i64)->atomic<list<i64>> {when {return delta_value(value)}}\ntest bad {eval(pass,[[]],[1])}",
        "valid and modified",
    );
}

#[test]
fn ordinary_test_bindings_preserve_readonly_authority() {
    reject(
        "test bad {let values:list<i64> = []\npush(values,1)}",
        "writable",
    );
    reject("test bad {let value=1\nvalue=2}", "writable");
    reject(
        "test bad {let value=1\nlet value=2}",
        "duplicate test local",
    );
}
#[test]
fn missing_atomic_fields_are_not_completed_from_other_slots() {
    reject(
        "struct Payload {first:i64\nsecond:str}\nfn pass(value:atomic<Payload>)->atomic<Payload> {when {return delta_value(value)}}\ntest bad {eval(pass,[Payload(first:1,second:\"old\"),Payload(first:2)])}",
        "missing or wrong-type argument",
    );
}
#[test]
fn equivalent_scalar_overloads_do_not_become_distinct_candidates() {
    reject(
        "fn pass(value:i64)->i64 {when {return value}}\nfn pass(value:atomic<i64>)->i64 {when {return value}}\ntest bad {eval(pass,[1])}",
        "found 2",
    );
}

#[test]
fn nested_value_annotations_reject_before_normalization() {
    for body in [
        "fn target(const value:tuple<atomic<i64>,i64>)->i64 {when {return 1}}\ntest bad {eval(target,value:(1,2))}",
        "const fn seed(value:atomic<i64>)->i64 {return value}\nfn target(value:i64)->i64 {when {return value}}\ntest bad {eval(target,[seed(1)])}",
        "fn target(value:atomic<list<atomic<i64>>>)->i64 {when {return 1}}\ntest bad {eval(target,[])}",
        "struct Box<T>{value:delta<T>}\ntest bad {let b = Box<atomic<list<atomic<i64>>>>(value:[])}",
    ] {
        reject(body, "value_type");
    }
}
#[test]
fn atomic_activity_rejects_without_backend_panic() {
    reject(
        "fn target(value:atomic<list<i64>>)->i64 {when {passivate(value)\nreturn 1}}\ntest bad {eval(target,[[]])}",
        "scalar input",
    );
}

#[test]
fn key_and_member_annotations_require_source_value_types() {
    for shape in [
        "set<atomic<i64>>",
        "map<atomic<i64>,i64>",
        "rolling<atomic<i64>,2>",
        "atomic<signal>",
    ] {
        reject(
            &format!(
                "fn target(value:{shape})->i64 {{when {{return 1}}}}\ntest bad {{eval(target,[])}}"
            ),
            "value_type",
        );
    }
}

#[test]
fn composite_boundaries_do_not_become_ordinary_result_values() {
    reject(
        "const fn bad()->atomic<list<i64>> {let values:list<i64> = []\nreturn values}\ntest bad_result {let values=bad()}",
        "node return type mismatch",
    );
    assert!(
        compile_tests(&sources(
            "test bad_local {let values:atomic<list<i64>> = []}"
        ))
        .is_err()
    );
}

#[test]
fn atomic_collection_shapes_are_admitted() {
    for shape in ["atomic<set<i64>>", "atomic<map<i64,i64>>"] {
        let body = format!(
            "fn pass(value:{shape})->{shape} {{when {{return delta_value(value)}}}}\ntest empty {{eval(pass,[])}}"
        );
        let result = compile_tests(&sources(&body));
        assert!(result.is_ok(), "{shape}: {result:?}");
    }
}
