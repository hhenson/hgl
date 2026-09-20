//! A cycle of these nodes allocates nothing once warm
//! (`docs/explorations/0009-designing-for-speed.md`, banned on the per-tick
//! path, 1). In a file of its own because it replaces the global allocator.

use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_describe::{BuildError, Builder, Registry, instantiate};
use hgl_kernel::{Graph, NodeResult};
use hgl_proto_nodes::{Checksum, register_all};
use hgl_store::Store;
use hgl_types::{EngineTime, NodeId, ScalarValue};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const WARM_UP: u64 = 10;
const CYCLES: u64 = 1_000;

/// Every node that ticks each cycle, `checksum` last:
/// `pulse` into `add_one` into `shift`, and into `add_const` into
/// `running_sum`; the two joined by `sum`.
fn graph(registry: &Registry, store: &mut Store) -> Result<Graph, BuildError> {
    let mut builder = Builder::new("every node, every cycle", registry);
    let pulse = builder.node("pulse", &[("count", ScalarValue::I64(i64::MAX))])?;
    let add_one = builder.node("add_one", &[])?;
    let add_const = builder.node("add_const", &[("k", ScalarValue::I64(3))])?;
    let shift = builder.node("shift", &[("delta", ScalarValue::I64(2))])?;
    let running_sum = builder.node("running_sum", &[])?;
    let sum = builder.node("sum", &[])?;
    let checksum = builder.node("checksum", &[])?;
    builder.connect(pulse, add_one, "in")?;
    builder.connect(pulse, add_const, "in")?;
    builder.connect(add_one, shift, "in")?;
    builder.connect(add_const, running_sum, "in")?;
    builder.connect(shift, sum, "lhs")?;
    builder.connect(running_sum, sum, "rhs")?;
    builder.connect(sum, checksum, "in")?;
    instantiate(&builder.finish()?, registry, store)
}

fn cycle(graph: &mut Graph, store: &mut Store) -> NodeResult {
    let now = graph.next_scheduled_time();
    graph.evaluate(store, now)
}

// Card, "Speed": no allocation in an eval body.
#[test]
fn a_cycle_of_every_ticking_node_allocates_nothing() {
    let mut registry = Registry::new();
    register_all(&mut registry).unwrap();
    let mut store = Store::new();
    let mut graph = graph(&registry, &mut store).unwrap();
    graph.start(&mut store, EngineTime::MIN_START).unwrap();
    for _ in 0..WARM_UP {
        cycle(&mut graph, &mut store).unwrap();
    }

    let (outcome, allocations) =
        count_in(|| (0..CYCLES).try_for_each(|_| cycle(&mut graph, &mut store)));

    assert_eq!(outcome, Ok(()));
    assert_eq!(allocations, 0);
    let checksum = graph.node::<Checksum>(NodeId(6)).unwrap();
    assert_eq!(checksum.evals(), WARM_UP + CYCLES);
}
