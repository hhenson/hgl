//! A cycle costs what is ready, not what exists (card, "Done when";
//! `docs/explorations/0009-designing-for-speed.md`, banned on the per-tick
//! path, 5). In a file of its own so that no other test runs beside the
//! clock.

use std::time::{Duration, Instant};

use hgl_kernel::{Ctx, Graph, Node, NodeResult, NodeSlot};
use hgl_store::{Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

const SMALL: u32 = 100;
const LARGE: u32 = 100_000;
const CYCLES: u32 = 5_000;
const PAIRS: usize = 201;

/// The one node that is ever ready: it ticks and asks for the next step.
struct Pulse {
    out: Out<i64>,
    ticks: i64,
}

impl Node for Pulse {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(0))
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.ticks += 1;
        ctx.set(self.out, self.ticks);
        ctx.schedule_in(EngineDelta::STEP)
    }
}

/// Exists, and is never scheduled.
struct Idle;

impl Node for Idle {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

fn slot(node: impl Node, uses_scheduler: bool) -> NodeSlot {
    let node_type = NodeType {
        name: "test",
        inputs: Vec::new(),
        output: None,
        scalars: Vec::new(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler,
        schedule_on_start: false,
        child_graphs: 0,
    };
    NodeSlot {
        node: Box::new(node),
        node_type,
        label: "test".to_owned(),
        required: Vec::new(),
    }
}

/// A started graph of `nodes` nodes of which one, in the middle of the rank
/// order, is ready every cycle. In the middle, so that a scan from either end
/// of the ranks would have half of them to cross.
fn one_ready_among(nodes: u32) -> (Graph, Store) {
    let mut store = Store::new();
    let busy = NodeId(nodes / 2);
    let out = store.add_output(busy);
    let slots = (0..nodes).map(|rank| {
        if NodeId(rank) == busy {
            slot(Pulse { out, ticks: 0 }, true)
        } else {
            slot(Idle, false)
        }
    });
    let mut graph = Graph::new("scaling".to_owned(), slots.collect());
    assert_eq!(graph.start(&mut store, EngineTime::MIN_START), Ok(()));
    (graph, store)
}

fn time_cycles(graph: &mut Graph, store: &mut Store) -> Duration {
    let begun = Instant::now();
    for _ in 0..CYCLES {
        let now = graph.next_scheduled_time();
        assert_eq!(graph.evaluate(store, now), Ok(()));
    }
    begun.elapsed()
}

// Card, "Done when": a cycle with one ready node costs the same in a graph
// of 100 nodes and of 100,000 (within 20%).
//
// Wall-clock, so made robust. The two graphs are timed in adjacent pairs of
// short samples, so that whatever the machine is doing at that moment falls
// on both, and the verdict is the median of the pairs' ratios, which a few
// disturbed pairs cannot move. What the larger graph does pay is one more
// level of the ready set, about 6% in debug.
#[test]
fn one_ready_node_costs_the_same_among_a_hundred_nodes_and_a_hundred_thousand() {
    let (mut small, mut small_store) = one_ready_among(SMALL);
    let (mut large, mut large_store) = one_ready_among(LARGE);

    let mut ratios = Vec::with_capacity(PAIRS);
    for pair in 0..PAIRS {
        // Each graph goes first in half the pairs, so neither gains by its place.
        let (small_time, large_time) = if pair % 2 == 0 {
            let first = time_cycles(&mut small, &mut small_store);
            (first, time_cycles(&mut large, &mut large_store))
        } else {
            let first = time_cycles(&mut large, &mut large_store);
            (time_cycles(&mut small, &mut small_store), first)
        };
        ratios.push(large_time.as_secs_f64() / small_time.as_secs_f64());
    }
    ratios.sort_by(f64::total_cmp);

    let (lowest, median, highest) = (ratios[0], ratios[PAIRS / 2], ratios[PAIRS - 1]);
    println!(
        "{LARGE} nodes over {SMALL}, per pair of {CYCLES} cycles: median {median:.3}, from {lowest:.3} to {highest:.3}"
    );
    assert!(median <= 1.2, "median ratio {median:.3}");
    let cycles = i64::from(CYCLES) * i64::try_from(PAIRS).unwrap();
    assert_eq!(
        large.node::<Pulse>(NodeId(LARGE / 2)).unwrap().ticks,
        cycles
    );
}
