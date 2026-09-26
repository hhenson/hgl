use hgl_describe::{Builder, Registry, instantiate_complete};
use hgl_kernel::{RunConfig, run_simulation};
use hgl_store::Store;
use hgl_types::{EngineTime, NodeId, ScalarValue};

mod generated;
mod renamed;
mod dormant;
mod nested;
struct Provider;
impl generated::Native for Provider { fn print_i64(value: i64) { println!("{value}"); } }
impl renamed::Native for Provider { fn print_i64(value: i64) { println!("{value}"); } }
impl dormant::Native for Provider { fn print_i64(value: i64) { println!("{value}"); } }
impl nested::Native for Provider { fn print_i64(value: i64) { println!("{value}"); } }

fn config() -> RunConfig {
    RunConfig { start_time: EngineTime::MIN_START, end_time: EngineTime::from_micros(10) }
}
fn run(description: hgl_describe::GraphDescription, registry: &Registry, expected: Option<i64>) {
    let mut store = Store::new();
    let mut built = instantiate_complete(&description, registry, &mut store).unwrap();
    assert_eq!(description.nodes.len(), 2);
    let output = built.outputs[0].unwrap();
    assert_eq!(store.output_value_erased(output), None);
    let cycles = run_simulation(&mut built.graph, &mut store, &config()).unwrap();
    assert_eq!(cycles, 1);
    assert_eq!(store.output_value_erased(output), expected.map(ScalarValue::I64));
    assert_eq!(store.output_modified(output, EngineTime::MIN_START), expected.is_some());
    assert!(!store.output_modified(output, EngineTime::from_micros(2)));
    assert_eq!(built.graph.next_scheduled_time(), EngineTime::FOREVER);
}
fn main() {
    let mut registry = Registry::new();
    generated::register(&mut registry).unwrap();
    run(generated::main(&registry).unwrap(), &registry, Some(42));
    run(generated::main(&registry).unwrap(), &registry, Some(42));
    let mut registry = Registry::new();
    renamed::register(&mut registry).unwrap();
    run(renamed::main(&registry).unwrap(), &registry, Some(-7));
    let mut registry = Registry::new();
    dormant::register(&mut registry).unwrap();
    run(dormant::main(&registry).unwrap(), &registry, None);
    let mut registry = Registry::new();
    nested::register(&mut registry).unwrap();
    let description = nested::main(&registry).unwrap();
    assert_eq!(description.nodes.len(), 4);
    let mut store = Store::new();
    let mut built = instantiate_complete(&description, &registry, &mut store).unwrap();
    assert_eq!(run_simulation(&mut built.graph, &mut store, &config()).unwrap(), 1);

    let mut registry = Registry::new();
    generated::register(&mut registry).unwrap();
    registry.register::<hgl_testkit::Replay>().unwrap();
    for ticks in [vec![None, Some(42), None, Some(42), Some(-7)], vec![None, None]] {
        let mut builder = Builder::new("sink", &registry);
        let replay = builder.node("testkit.replay", &[("slot", ScalarValue::I64(0))]).unwrap();
        let sink = builder.node("examples.const_debug.debug_print", &[]).unwrap();
        builder.connect(replay, sink, "value").unwrap();
        let mut store = Store::new();
        let mut built = instantiate_complete(&builder.finish().unwrap(), &registry, &mut store).unwrap();
        let values: Vec<_> = ticks.into_iter().map(|tick| tick.map(ScalarValue::I64)).collect();
        built.graph.node_mut::<hgl_testkit::Replay>(NodeId(0)).unwrap().load(&values);
        run_simulation(&mut built.graph, &mut store, &config()).unwrap();
    }
}
