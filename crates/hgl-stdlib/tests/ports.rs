//! NAT-4/5: reasoned tick traces, actual input admission and independent history.
mod support {
    include!("support/cases.rs");
}
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_describe::{Buildable, Builder, Registry, instantiate_complete};
use hgl_kernel::{NodeError, RunConfig, run_simulation};
use hgl_stdlib::{DedupI64, SampleI64, register_all};
use hgl_store::Store;
use hgl_testkit::{Record, Replay};
use hgl_types::{EngineTime, NodeId, ScalarValue};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn registry() -> Result<Registry, String> {
    let mut registry = Registry::new();
    register_all(&mut registry).map_err(debug)?;
    registry.register::<Replay>().map_err(debug)?;
    registry.register::<Record>().map_err(debug)?;
    Ok(registry)
}

#[test]
fn nat4_reasoned_cases_match_rust() -> Result<(), String> {
    let registry = registry().map_err(debug)?;
    for case in support::CASES {
        hgl_testkit::run(case, &registry).map_err(debug)?;
    }
    Ok(())
}

#[test]
fn nat4_sample_activation_and_validity_are_distinct() -> Result<(), String> {
    let signature = SampleI64::node_type();
    assert_eq!(signature.active_inputs, Some(vec![0]));
    assert_eq!(signature.valid_inputs, None);
    let registry = registry().map_err(debug)?;
    let mut builder = Builder::new("sampling", &registry);
    builder.node(signature.name, &[]).map_err(debug)?;
    let mut store = Store::new();
    let built = instantiate_complete(&builder.finish().map_err(debug)?, &registry, &mut store)
        .map_err(debug)?;
    assert!(store.bindings().input(built.inputs[0][0]).active);
    assert!(!store.bindings().input(built.inputs[0][1]).active);
    Ok(())
}

#[test]
fn nat5_instances_keep_independent_history_without_tick_allocations() -> Result<(), String> {
    let registry = registry().map_err(debug)?;
    let mut builder = Builder::new("independent dedup", &registry);
    for slot in 0..2 {
        let source = builder
            .node("testkit.replay", &[("slot", ScalarValue::I64(slot))])
            .map_err(debug)?;
        let node = builder
            .node(DedupI64::node_type().name, &[])
            .map_err(debug)?;
        let sink = builder
            .node("testkit.record", &[("cycles", ScalarValue::I64(4))])
            .map_err(debug)?;
        builder.connect(source, node, "ts").map_err(debug)?;
        builder.connect(node, sink, "in").map_err(debug)?;
    }
    let description = builder.finish().map_err(debug)?;
    let mut store = Store::new();
    let mut built = instantiate_complete(&description, &registry, &mut store).map_err(debug)?;
    for (position, node) in description.nodes.iter().enumerate() {
        if node.implementation != "testkit.replay" {
            continue;
        }
        let id = u32::try_from(position).map_err(debug)?;
        let replay = built
            .graph
            .node_mut::<Replay>(NodeId(id))
            .ok_or_else(|| NodeError::new("missing replay"))
            .map_err(debug)?;
        replay.load(&[
            Some(ScalarValue::I64(0)),
            Some(ScalarValue::I64(0)),
            Some(ScalarValue::I64(1)),
        ]);
    }
    let config = RunConfig {
        start_time: EngineTime::MIN_START,
        end_time: EngineTime::MAX_END,
    };

    let (result, allocations) = count_in(|| run_simulation(&mut built.graph, &mut store, &config));
    result.map_err(debug)?;
    assert_eq!(allocations, 0);
    for (position, node) in description.nodes.iter().enumerate() {
        if node.implementation != "testkit.record" {
            continue;
        }
        let id = u32::try_from(position).map_err(debug)?;
        let record = built
            .graph
            .node::<Record>(NodeId(id))
            .ok_or_else(|| NodeError::new("missing record"))
            .map_err(debug)?;
        assert_eq!(
            &record.seen()[..3],
            &[Some(ScalarValue::I64(0)), None, Some(ScalarValue::I64(1))]
        );
    }
    Ok(())
}

fn debug(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
