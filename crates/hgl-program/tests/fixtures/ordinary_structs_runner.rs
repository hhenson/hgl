use hgl_describe::{Registry, instantiate_complete};
use hgl_kernel::{RunConfig, run_simulation};
use hgl_store::Store;
use hgl_types::EngineTime;
mod graph;
mod evaluations;
mod order_success;
mod order_failure;
struct Provider;
static TRACE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
fn mark(value: i64) -> Result<i64, Box<hgl_types::NodeError>> {
    TRACE.fetch_update(std::sync::atomic::Ordering::SeqCst, std::sync::atomic::Ordering::SeqCst, |trace| Some(trace * 10 + u64::try_from(value).unwrap())).unwrap();
    if value == 9 { Err(hgl_types::NodeError::new("constructor argument failure")) } else { Ok(value) }
}
impl order_success::Native for Provider {
    fn mark_i64(value: i64) -> Result<i64, Box<hgl_types::NodeError>> { mark(value) }
}
impl order_failure::Native for Provider {
    fn mark_i64(value: i64) -> Result<i64, Box<hgl_types::NodeError>> { mark(value) }
}
fn constructor_order() {
    for failure in [false, true] {
        TRACE.store(0, std::sync::atomic::Ordering::SeqCst);
        let mut registry = Registry::new();
        let description = if failure {
            order_failure::register(&mut registry).unwrap();
            order_failure::main(&registry).unwrap()
        } else {
            order_success::register(&mut registry).unwrap();
            order_success::main(&registry).unwrap()
        };
        let mut store = Store::new();
        store.global_state().provision();
        let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
        let result = run_simulation(&mut built.graph, &mut store, &RunConfig {
            start_time: EngineTime::MIN_START, end_time: EngineTime::from_micros(10)
        });
        if failure {
            assert!(format!("{:?}", result.unwrap_err()).contains("constructor argument failure"));
            assert_eq!(TRACE.load(std::sync::atomic::Ordering::SeqCst), 9);
            let after = store.global_state().bind::<i64>("after").unwrap();
            assert!(store.global_state().get(after).is_err());
        } else {
            result.unwrap();
            assert_eq!(TRACE.load(std::sync::atomic::Ordering::SeqCst), 21436587);
            let fallback = store.global_state().bind::<String>("fallback").unwrap();
            assert_eq!(store.global_state().get(fallback).unwrap(), "default");
            for (key, expected) in [("pair", 12), ("nested", 5634), ("projected", 7)] {
                let handle = store.global_state().bind::<i64>(key).unwrap();
                assert_eq!(store.global_state().get(handle).unwrap(), expected);
            }
        }
        let before = store.global_state().bind::<i64>("before").unwrap();
        assert_eq!(store.global_state().get(before).unwrap(), 1);
    }
}
fn main() {
    evaluations::main();
    constructor_order();
    let mut registry = Registry::new();
    graph::register(&mut registry).unwrap();
    let description = graph::main(&registry).unwrap();
    let mut store = Store::new();
    store.global_state().provision();
    let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
    run_simulation(&mut built.graph, &mut store, &RunConfig {
        start_time: EngineTime::MIN_START, end_time: EngineTime::from_micros(10)
    }).unwrap();
    for (name, expected) in [
        ("start", 2), ("constructor_get", 2), ("frozen", 7), ("copy", 8), ("retained", 7),
        ("nested_original", 7), ("nested_copy", 12), ("field_retained", 9),
        ("self_field", 10), ("self_whole", 101), ("whole_retained", 8),
        ("shadow", 301), ("after_shadow", 8), ("child_copy", 10),
        ("temporary_field", 42), ("stop", 3)
    ] {
        let handle = store.global_state().bind::<i64>(name).unwrap();
        assert_eq!(store.global_state().get(handle).unwrap(), expected, "{name}");
    }
    for (name, expected) in [
        ("frozen_text", "original"), ("copy_text", "original changed"),
        ("retained_text", "original"), ("nested_original_text", "original"),
        ("field_retained_text", "new"), ("whole_retained_text", "original changed"),
        ("child_text", "new")
    ] {
        let handle = store.global_state().bind::<String>(name).unwrap();
        assert_eq!(store.global_state().get(handle).unwrap(), expected, "{name}");
    }
}
