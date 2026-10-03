use hgl_describe::{Registry, instantiate_complete};
use hgl_kernel::{RunConfig, run_simulation};
use hgl_store::Store;
use hgl_types::EngineTime;
mod graph;
mod fail_assign;
mod fail_set;
mod evaluations;
struct Provider;
impl graph::Native for Provider {
    fn observe_i64_str(amount: i64, text: &str) -> i64 {
        assert_eq!(text, "edited whole");
        amount
    }
}

fn config() -> RunConfig {
    RunConfig { start_time: EngineTime::MIN_START, end_time: EngineTime::from_micros(10) }
}
fn integer(store: &mut Store, key: &str, expected: i64) {
    let entry = store.bind_global::<i64>(key).unwrap();
    assert_eq!(store.global_get(entry).unwrap(), expected, "{key}");
}
fn text(store: &mut Store, key: &str, expected: &str) {
    let entry = store.bind_global::<String>(key).unwrap();
    assert_eq!(store.global_get(entry).unwrap(), expected, "{key}");
}
fn lifecycle() {
    let mut registry = Registry::new();
    graph::register(&mut registry).unwrap();
    let description = graph::main(&registry).unwrap();
    for _ in 0..2 {
        let mut store = Store::new();
        store.provision_global_state();
        let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
        run_simulation(&mut built.graph, &mut store, &config()).unwrap();
        for (key, expected) in [
            ("start", 2), ("primitive", 2), ("live", 11), ("constructor_copy", 11),
            ("owned", 2), ("field_owned", 2), ("retained_amount", 4),
            ("wrapped_amount", 2), ("nested_replaced", 4), ("before_replace", 14), ("stop", 31),
            ("scalar_local", 104), ("scalar_stored", 4), ("native_fields", 11)
        ] { integer(&mut store, key, expected); }
        for (key, expected) in [
            ("start_text", "initial"), ("live_text", "edited whole"),
            ("owned_text", "initial"), ("field_owned_text", "initial"),
            ("retained_text", "initial"), ("nested_replaced_text", "initial"), ("stop_text", "after scopes")
        ] { text(&mut store, key, expected); }
        let valid = store.bind_global::<bool>("wide_valid").unwrap();
        assert!(store.global_get(valid).unwrap());
    }
}
fn replacement_failure() {
    for assignment in [false, true] {
        let mut registry = Registry::new();
        let description = if assignment {
            fail_assign::register(&mut registry).unwrap();
            fail_assign::main(&registry).unwrap()
        } else {
            fail_set::register(&mut registry).unwrap();
            fail_set::main(&registry).unwrap()
        };
        let mut store = Store::new();
        store.provision_global_state();
        let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
        assert!(run_simulation(&mut built.graph, &mut store, &config()).is_err());
        integer(&mut store, "failure_amount", if assignment { 22 } else { 20 });
        text(&mut store, "failure_text", "preserved");
        let after = store.bind_global::<bool>("after_failure").unwrap();
        assert!(store.global_get(after).is_err());
    }
}
fn main() {
    lifecycle();
    replacement_failure();
    evaluations::main();
    missing_values();
}
