use hgl_describe::{Builder,Registry,instantiate_complete};
use hgl_kernel::{run_simulation,RunConfig};
use hgl_store::Store;
use hgl_types::{EngineTime,ScalarValue,NodeId};

fn config() -> RunConfig {
    RunConfig { start_time: EngineTime::MIN_START, end_time: EngineTime::from_micros(10) }
}
fn run(registry: &Registry, description: hgl_describe::GraphDescription, value: i64, time: i64) {
    assert_eq!(description.nodes.len(),2);
    let mut store=Store::new();
    let mut built=instantiate_complete(&description,registry,&mut store).unwrap();
    let output=built.outputs[0].unwrap();
    assert_eq!(store.output_value_erased(output),None);
    assert_eq!(run_simulation(&mut built.graph,&mut store,&config()).unwrap(),1);
    assert_eq!(store.output_value_erased(output),Some(ScalarValue::I64(value)));
    assert!(store.output_modified(output,EngineTime::from_micros(time)));
    assert!(!store.output_modified(output,EngineTime::from_micros(time+1)));
    assert_eq!(built.graph.next_scheduled_time(),EngineTime::FOREVER);
}
fn replay(registry: &mut Registry) {
    let mut builder=Builder::new("replay",registry);
    let replay=builder.node("testkit.replay",&[("slot",ScalarValue::I64(0))]).unwrap();
    let sink=builder.node("hgraph.std::debug_print#1",&[]).unwrap();
    builder.connect(replay,sink,"ts").unwrap();
    let mut store=Store::new();
    let mut built=instantiate_complete(&builder.finish().unwrap(),registry,&mut store).unwrap();
    let values=[None,Some(42),None,Some(42),Some(-7)].map(|v|v.map(ScalarValue::I64));
    built.graph.node_mut::<hgl_testkit::Replay>(NodeId(0)).unwrap().load(&values);
    run_simulation(&mut built.graph,&mut store,&config()).unwrap();
}
