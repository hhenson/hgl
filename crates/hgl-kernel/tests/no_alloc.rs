//! A cycle allocates nothing (card, "Done when";
//! `docs/explorations/0009-designing-for-speed.md`, banned on the per-tick
//! path, 1). In a file of its own because it replaces the global allocator.

use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_kernel::{Ctx, Graph, Node, NodeResult, NodeSlot};
use hgl_store::{In, Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const WARM_UP: i64 = 10;
const CYCLES: i64 = 100_000;

/// Ticks 1, 2, 3, ... a step apart: a heap pop and a heap push every cycle.
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

struct AddOne {
    input: In<i64>,
    out: Out<i64>,
}

impl Node for AddOne {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, ctx.get(self.input) + 1);
        Ok(())
    }
}

struct Total {
    input: In<i64>,
    sum: i64,
    evals: i64,
}

impl Node for Total {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.sum += ctx.get(self.input);
        self.evals += 1;
        Ok(())
    }
}

/// Asks in start to be evaluated a second later, and never again.
struct Timer;

impl Node for Timer {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(1_000_000))
    }

    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

fn slot(node: impl Node, uses_scheduler: bool, required: &[In<i64>]) -> NodeSlot {
    let node_type = NodeType {
        name: "test",
        inputs: Vec::new(),
        output: None,
        scalars: Vec::new(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler,
        schedule_on_start: false,
    };
    NodeSlot {
        node: Box::new(node),
        node_type,
        label: "test".to_owned(),
        required: required.iter().map(|input| input.id()).collect(),
    }
}

/// Warm `graph` up, then run `cycles` cycles, each at its next scheduled time,
/// and return the allocations they made. Every cycle must succeed.
fn allocations_after_warm_up(graph: &mut Graph, store: &mut Store, cycles: i64) -> u64 {
    let mut cycle = |graph: &mut Graph| {
        let now = graph.next_scheduled_time();
        assert_ne!(now, EngineTime::FOREVER);
        graph.evaluate(store, now)
    };
    for _ in 0..WARM_UP {
        assert_eq!(cycle(graph), Ok(()));
    }
    let (failures, allocations) =
        count_in(|| (0..cycles).filter(|_| cycle(graph).is_err()).count());
    assert_eq!(failures, 0);
    allocations
}

// GRF-14; card, "Known limits": a node whose request stands pushes nothing.
// A timer woken by an input in every cycle, its own request a second ahead,
// makes no allocation in 10,000 cycles.
#[test]
fn grf14_a_timer_an_input_wakes_every_cycle_allocates_nothing() {
    let mut store = Store::new();
    let ticks = store.add_output(NodeId(0));
    let wakes = store.add_input::<i64>(NodeId(1), true);
    store.bind(wakes.id(), ticks.id()).unwrap();
    let pulse = Pulse {
        out: ticks,
        ticks: 0,
    };
    let slots = vec![slot(pulse, true, &[]), slot(Timer, true, &[])];
    let mut graph = Graph::new("timer".to_owned(), slots);
    assert_eq!(graph.start(&mut store, EngineTime::MIN_START), Ok(()));

    assert_eq!(allocations_after_warm_up(&mut graph, &mut store, 10_000), 0);
}

// Card, "Done when": 100,000 cycles of a three-node graph make zero
// allocations after warm-up.
#[test]
fn a_hundred_thousand_cycles_of_a_three_node_graph_allocate_nothing() {
    let mut store = Store::new();
    let ticks = store.add_output(NodeId(0));
    let to_add = store.add_input(NodeId(1), true);
    let sums = store.add_output(NodeId(1));
    let to_total = store.add_input(NodeId(2), true);
    store.bind(to_add.id(), ticks.id()).unwrap();
    store.bind(to_total.id(), sums.id()).unwrap();
    let pulse = Pulse {
        out: ticks,
        ticks: 0,
    };
    let add_one = AddOne {
        input: to_add,
        out: sums,
    };
    let total = Total {
        input: to_total,
        sum: 0,
        evals: 0,
    };
    let slots = vec![
        slot(pulse, true, &[]),
        slot(add_one, false, &[to_add]),
        slot(total, false, &[to_total]),
    ];
    let mut graph = Graph::new("three nodes".to_owned(), slots);
    assert_eq!(graph.start(&mut store, EngineTime::MIN_START), Ok(()));

    assert_eq!(allocations_after_warm_up(&mut graph, &mut store, CYCLES), 0);

    let total = graph.node::<Total>(NodeId(2)).unwrap();
    let cycles = WARM_UP + CYCLES;
    assert_eq!(total.evals, cycles);
    assert_eq!(total.sum, cycles * (cycles + 1) / 2 + cycles);
}
