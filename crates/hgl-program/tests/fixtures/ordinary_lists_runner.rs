use hgl_describe::{Registry, instantiate_complete};
use hgl_kernel::{RunConfig, run_simulation};
use hgl_store::Store;
use hgl_types::EngineTime;
mod graph;
mod fail_append;
mod fail_replace;
mod evaluations;
mod phases;

fn config() -> RunConfig {
    RunConfig { start_time: EngineTime::MIN_START, end_time: EngineTime::from_micros(10) }
}
fn integer(store: &mut Store, key: &str, expected: i64) {
    let entry = store.bind_global::<i64>(key).unwrap();
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
            ("start_length", 2), ("first", 1), ("before", 2), ("after", 5),
            ("last", 1), ("nested_first", 2), ("nested_second", 1),
            ("indexed_copy", 2), ("readonly_child", 4), ("struct_child", 7),
            ("owned_length", 2), ("retained_length", 2),
            ("stop_length", 6), ("stop_last", 8)
        ] { integer(&mut store, key, expected); }
    }
}
fn failure() {
    for replacement in [false, true] {
        let mut registry = Registry::new();
        let description = if replacement {
            fail_replace::register(&mut registry).unwrap();
            fail_replace::main(&registry).unwrap()
        } else {
            fail_append::register(&mut registry).unwrap();
            fail_append::main(&registry).unwrap()
        };
        let mut store = Store::new();
        store.provision_global_state();
        let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
        assert!(run_simulation(&mut built.graph, &mut store, &config()).is_err());
        integer(&mut store, "failure_length", 2);
        integer(&mut store, "failure_first", 10);
        integer(&mut store, "failure_second", 20);
        let after = store.bind_global::<bool>("after_failure").unwrap();
        assert!(store.global_get(after).is_err());
    }
}
fn ordinary_phases() {
    let mut registry = Registry::new();
    phases::register(&mut registry).unwrap();
    let description = phases::main(&registry).unwrap();
    let mut store = Store::new();
    store.provision_global_state();
    let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
    run_simulation(&mut built.graph, &mut store, &config()).unwrap();
    for (key, expected) in [("wiring", 23), ("constant", 24), ("contextual_parameter", 0), ("parameter_copy", 1), ("value_body", 25), ("config_source",13), ("config_copy",99), ("config_nested",7), ("config_nested_copy",88)] {
        integer(&mut store, key, expected);
    }
    for (name, expected) in [("negative",2.0_f64), ("negative_divisor",-2.0), ("negative_zero",-0.0), ("positive_zero",0.0), ("infinity",1.0), ("tiny",1.0 % 1e-308)] {
        for phase in ["wiring", "runtime"] {
            let entry=store.bind_global::<f64>(&format!("mod_{phase}_{name}")).unwrap();
            assert_eq!(store.global_get(entry).unwrap().to_bits(),expected.to_bits(),"{phase} {name}");
        }
    }
}
fn main() {
    lifecycle();
    failure();
    evaluations::main();
    ordinary_phases();
    bounds_and_presence();
}
