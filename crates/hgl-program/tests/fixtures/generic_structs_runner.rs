use hgl_describe::{Registry, instantiate_complete};
use hgl_kernel::{RunConfig, run_simulation};
use hgl_store::{Scalar, Store};
use hgl_types::{Date, EngineDelta, EngineTime, Time};
mod graph;
mod get_first;
mod get_last;
struct Provider;
static TRACE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
impl graph::Native for Provider {
    fn mark_i64(value: i64) -> Result<i64, Box<hgl_types::NodeError>> {
        TRACE
            .fetch_update(
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
                |trace| Some(trace * 10 + u64::try_from(value).unwrap()),
            )
            .unwrap();
        Ok(value)
    }
}
impl get_first::Native for Provider {
    fn mark_i64(value: i64) -> hgl_types::NodeResult<i64> {
        <Self as graph::Native>::mark_i64(value)
    }
}
impl get_last::Native for Provider {
    fn mark_i64(value: i64) -> hgl_types::NodeResult<i64> {
        <Self as graph::Native>::mark_i64(value)
    }
}
fn contextual_get_order() {
    let mut first = Registry::new();
    get_first::register(&mut first).unwrap();
    let mut last = Registry::new();
    get_last::register(&mut last).unwrap();
    for (registry, description, expected_trace) in [
        (&first, get_first::main(&first).unwrap(), 0),
        (&last, get_last::main(&last).unwrap(), 1),
    ] {
        TRACE.store(0, std::sync::atomic::Ordering::SeqCst);
        let mut store = Store::new();
        store.provision_global_state();
        let mut built = instantiate_complete(&description, registry, &mut store).unwrap();
        let result = run_simulation(&mut built.graph, &mut store, &RunConfig {
            start_time: EngineTime::MIN_START,
            end_time: EngineTime::from_micros(10),
        });
        let error = format!("{:?}", result.expect_err("the global entry is absent"));
        assert!(error.contains("missing value for key") && error.contains("missing"), "{error}");
        assert_eq!(TRACE.load(std::sync::atomic::Ordering::SeqCst), expected_trace);
    }
}
fn read<T: Scalar>(store: &mut Store, key: &str, expected: &T) {
    let entry = store.bind_global::<T>(key).unwrap();
    assert_eq!(&store.global_get(entry).unwrap(), expected, "{key}");
}
fn main() {
    contextual_get_order();
    let mut registry = Registry::new();
    graph::register(&mut registry).unwrap();
    let description = graph::main(&registry).unwrap();
    for _ in 0..2 {
        TRACE.store(0, std::sync::atomic::Ordering::SeqCst);
        let mut store = Store::new();
        store.provision_global_state();
        let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
        let config = RunConfig {
            start_time: EngineTime::MIN_START,
            end_time: EngineTime::from_micros(10),
        };
        run_simulation(&mut built.graph, &mut store, &config).unwrap();
        assert_eq!(TRACE.load(std::sync::atomic::Ordering::SeqCst), 1234);
        for prefix in [
            "configured",
            "observed",
            "indexed",
            "captured",
            "helper",
            "helper_list",
        ] {
            read(&mut store, &format!("{prefix}_flag"), &true);
            read(&mut store, &format!("{prefix}_integer"), &7_i64);
            read(&mut store, &format!("{prefix}_real"), &2.5);
            read(&mut store, &format!("{prefix}_text"), &"payload".to_owned());
            read(&mut store, &format!("{prefix}_day"), &Date(0));
            read(&mut store, &format!("{prefix}_time"), &Time(2_000_000));
            read(
                &mut store,
                &format!("{prefix}_instant"),
                &EngineTime::from_micros(3_000_000),
            );
            read(
                &mut store,
                &format!("{prefix}_elapsed"),
                &EngineDelta::from_micros(2),
            );
        }
        read(&mut store, "capture_time", &EngineTime::MIN_START);
        for (key, expected) in [
            ("capture_count", 1),
            ("inferred_fields", 21),
            ("reverse_fields", 43),
            ("configuration_original", 7),
            ("configuration_independent", 99),
            ("wrapped_length", 1),
            ("original_length", 2),
            ("source_length", 2),
            ("borrowed_initial", 1),
            ("borrowed_updated", 3),
            ("owning_length", 2),
            ("retained_length", 1),
            ("phantom_sum", 3),
            ("contextual_return", 3),
            ("contextual_parameter", 6),
            ("nested_repeated", 15),
            ("constrained", 9),
            ("stop_length", 3),
        ] {
            read(&mut store, key, &expected);
        }
    }
}
