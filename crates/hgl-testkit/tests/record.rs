//! The record's span, driven through a graph rather than through a case.
//!
//! [`hgl_testkit::run`] sizes a record to cover every cycle the case can
//! reach, so a case cannot show what happens past it. A node that ticks later
//! than the harness was told to expect must be refused: the alternative is a
//! `seen` sized from engine time, which runs to 10^16 steps.

use hgl_describe::{BuildError, Builder, Registry, instantiate};
use hgl_kernel::Graph;
use hgl_store::Store;
use hgl_testkit::{Record, Replay};
use hgl_types::{EngineTime, NodeId, ScalarValue};

/// How many cycles the record is told to expect, of the four the replay has.
const SPAN: i64 = 2;

fn graph(store: &mut Store) -> Result<Graph, BuildError> {
    let mut registry = Registry::new();
    registry.register::<Replay>()?;
    registry.register::<Record>()?;

    let mut builder = Builder::new("replay, record", &registry);
    let replay = builder.node("testkit.replay", &[("slot", ScalarValue::I64(0))])?;
    let record = builder.node("testkit.record", &[("cycles", ScalarValue::I64(SPAN))])?;
    builder.connect(replay, record, "in")?;
    let description = builder.finish()?;
    instantiate(&description, &registry, store)
}

/// One cycle, at its own scheduled time so that none is skipped (ENG-4).
fn cycle(graph: &mut Graph, store: &mut Store) -> Result<(), String> {
    let now = graph.next_scheduled_time();
    assert_ne!(now, EngineTime::FOREVER);
    graph
        .evaluate(store, now)
        .map_err(|error| error.message.clone())
}

#[test]
fn the_cycles_within_the_span_are_noted_and_the_next_tick_is_refused() {
    let mut store = Store::new();
    let mut graph = graph(&mut store).unwrap();
    let ticks: Vec<Option<ScalarValue>> = (0..4).map(|n| Some(ScalarValue::I64(n))).collect();
    graph.node_mut::<Replay>(NodeId(0)).unwrap().load(&ticks);
    graph.start(&mut store, EngineTime::MIN_START).unwrap();

    for _ in 0..SPAN {
        assert_eq!(cycle(&mut graph, &mut store), Ok(()));
    }
    let seen = graph.node::<Record>(NodeId(1)).unwrap().seen();
    let expected = [Some(ScalarValue::I64(0)), Some(ScalarValue::I64(1))];
    assert_eq!(seen, expected);

    let refused = cycle(&mut graph, &mut store);
    assert_eq!(
        refused,
        Err("testkit.record: a tick after the last cycle".to_owned())
    );
}
