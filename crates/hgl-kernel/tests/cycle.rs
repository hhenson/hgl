//! The evaluation cycle: which nodes a pass reaches, in what order, and which
//! of them it admits (specification: Graph, "The evaluation cycle"; Node,
//! "Admission").

use hgl_kernel::{Ctx, Graph, Node, NodeResult, NodeSlot};
use hgl_store::{In, InputId, Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

fn at(micros: i64) -> EngineTime {
    EngineTime::from_micros(micros)
}

fn node_type(name: &'static str, uses_scheduler: bool) -> NodeType {
    NodeType {
        name,
        inputs: Vec::new(),
        output: None,
        scalars: Vec::new(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler,
        schedule_on_start: false,
    }
}

/// A store and the slots of a graph, put together by hand as hgl-describe
/// will: a node's handles are made first, then the node is added.
#[derive(Default)]
struct Wiring {
    store: Store,
    slots: Vec<NodeSlot>,
    added: u32,
}

impl Wiring {
    /// The id the next node added will have.
    fn next(&self) -> NodeId {
        NodeId(self.added)
    }

    fn output(&mut self) -> Out<i64> {
        self.store.add_output(self.next())
    }

    /// An input of the next node, bound to `from`.
    fn input(&mut self, from: Out<i64>, active: bool) -> In<i64> {
        let input = self.store.add_input(self.next(), active);
        assert_eq!(self.store.bind(input.id(), from.id()), Ok(()));
        input
    }

    fn add(&mut self, node: impl Node, node_type: NodeType, required: Vec<InputId>) -> NodeId {
        let id = self.next();
        self.added += 1;
        self.slots.push(NodeSlot {
            node: Box::new(node),
            label: node_type.name.to_owned(),
            node_type,
            required,
        });
        id
    }

    /// A source ticking `values`, one a cycle, a step apart, the first `first`
    /// after the start time.
    fn source(&mut self, first: i64, values: &[i64]) -> Out<i64> {
        let out = self.output();
        let source = Source {
            out,
            first: EngineDelta::from_micros(first),
            values: values.to_vec(),
            ticked: 0,
        };
        self.add(source, node_type("source", true), Vec::new());
        out
    }

    fn add_one(&mut self, from: Out<i64>) -> (NodeId, Out<i64>) {
        let input = self.input(from, true);
        let out = self.output();
        let node = AddOne {
            input,
            out,
            evals: 0,
        };
        let id = self.add(node, node_type("add_one", false), vec![input.id()]);
        (id, out)
    }

    /// Started at time 1.
    fn started(self) -> (Graph, Store) {
        let mut store = self.store;
        let mut graph = Graph::new("test".to_owned(), self.slots);
        assert_eq!(graph.start(&mut store, at(1)), Ok(()));
        (graph, store)
    }
}

struct Source {
    out: Out<i64>,
    first: EngineDelta,
    values: Vec<i64>,
    ticked: usize,
}

impl Node for Source {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(self.first)
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, self.values[self.ticked]);
        self.ticked += 1;
        if self.ticked < self.values.len() {
            ctx.schedule_in(EngineDelta::STEP)?;
        }
        Ok(())
    }
}

struct AddOne {
    input: In<i64>,
    out: Out<i64>,
    evals: u32,
}

impl Node for AddOne {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.evals += 1;
        ctx.set(self.out, ctx.get(self.input) + 1);
        Ok(())
    }
}

/// Records every sum it computes, so a test sees each evaluation and what the
/// inputs read at that moment.
struct Sum {
    lhs: In<i64>,
    rhs: In<i64>,
    out: Out<i64>,
    seen: Vec<i64>,
}

impl Node for Sum {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let sum = ctx.get(self.lhs) + ctx.get(self.rhs);
        self.seen.push(sum);
        ctx.set(self.out, sum);
        Ok(())
    }
}

fn sum_of(wiring: &mut Wiring, lhs: Out<i64>, rhs: Out<i64>) -> NodeId {
    let lhs = wiring.input(lhs, true);
    let rhs = wiring.input(rhs, true);
    let out = wiring.output();
    let node = Sum {
        lhs,
        rhs,
        out,
        seen: Vec::new(),
    };
    wiring.add(node, node_type("sum", false), vec![lhs.id(), rhs.id()])
}

fn seen(graph: &Graph, sum: NodeId) -> Vec<i64> {
    let sum = graph.node::<Sum>(sum);
    sum.map(|sum| sum.seen.clone()).unwrap_or_default()
}

// GRF-11, GRF-15; card, "Done when": rank-order evaluation within one cycle.
// The source wakes `sum` (rank 2) before `add_one` (rank 1), because `sum`'s
// input is the first bound to it. Rank order still evaluates `add_one` first,
// so `sum` never reads the value `add_one` held in the cycle before.
#[test]
fn grf11_grf15_a_pass_is_in_rank_order_whatever_order_nodes_were_woken_in() {
    let mut wiring = Wiring::default();
    let ticks = wiring.source(0, &[10, 20]);
    let sum_lhs = wiring.store.add_input::<i64>(NodeId(2), true);
    wiring.store.bind(sum_lhs.id(), ticks.id()).unwrap();
    let (_, plus_one) = wiring.add_one(ticks);
    let sum_rhs = wiring.input(plus_one, true);
    let out = wiring.output();
    let sum = Sum {
        lhs: sum_lhs,
        rhs: sum_rhs,
        out,
        seen: Vec::new(),
    };
    let required = vec![sum_lhs.id(), sum_rhs.id()];
    let sum = wiring.add(sum, node_type("sum", false), required);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(1)), Ok(()));
    assert_eq!(graph.evaluate(&mut store, at(2)), Ok(()));

    assert_eq!(seen(&graph, sum), [10 + 11, 20 + 21]);
}

// GRF-16, NOD-3; card, "Done when": a node woken twice is evaluated once.
#[test]
fn grf16_nod3_a_node_woken_twice_in_a_cycle_is_evaluated_once() {
    let mut wiring = Wiring::default();
    let ticks = wiring.source(0, &[10, 20]);
    let (_, left) = wiring.add_one(ticks);
    let (_, right) = wiring.add_one(ticks);
    let sum = sum_of(&mut wiring, left, right);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(1)), Ok(()));
    assert_eq!(seen(&graph, sum), [22]);
    assert_eq!(graph.evaluate(&mut store, at(2)), Ok(()));
    assert_eq!(seen(&graph, sum), [22, 42]);
}

/// Records, for each evaluation, its time, which inputs ticked in it, and
/// when `rhs` last did.
struct Pair {
    lhs: In<i64>,
    rhs: In<i64>,
    seen: Vec<(EngineTime, bool, bool, EngineTime)>,
}

impl Node for Pair {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let (lhs, rhs) = (ctx.modified(self.lhs), ctx.modified(self.rhs));
        let rhs_ticked = ctx.last_modified(self.rhs);
        self.seen
            .push((ctx.evaluation_time(), lhs, rhs, rhs_ticked));
        Ok(())
    }
}

// GRF-16, NOD-3: two sources tick in one cycle, each notifying one of the
// node's active inputs, so the schedule is told about the node twice. It is
// evaluated once in that cycle, and sees both inputs modified.
#[test]
fn grf16_nod3_a_node_whose_two_inputs_both_tick_is_evaluated_once() {
    let mut wiring = Wiring::default();
    let left = wiring.source(0, &[1, 2, 3]);
    let right = wiring.source(0, &[10, 20]);
    let lhs = wiring.input(left, true);
    let rhs = wiring.input(right, true);
    let seen = Vec::new();
    let pair = wiring.add(Pair { lhs, rhs, seen }, node_type("pair", false), vec![]);
    let (mut graph, mut store) = wiring.started();

    for cycle in 1..=3 {
        assert_eq!(graph.evaluate(&mut store, at(cycle)), Ok(()));
    }

    let expected = [
        (at(1), true, true, at(1)),
        (at(2), true, true, at(2)),
        (at(3), true, false, at(2)),
    ];
    assert_eq!(graph.node::<Pair>(pair).unwrap().seen, expected);
}

// NOD-2; card, "Done when": a node whose required input is not valid is
// skipped. Validity is checked afresh: the node is admitted once it holds.
#[test]
fn nod2_a_node_whose_required_input_is_not_valid_is_skipped() {
    let mut wiring = Wiring::default();
    let early = wiring.source(0, &[1, 2, 3]);
    let late = wiring.source(1, &[10, 20]);
    let sum = sum_of(&mut wiring, early, late);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(1)), Ok(()));
    assert!(
        seen(&graph, sum).is_empty(),
        "woken by `early`, not admitted"
    );

    assert_eq!(graph.evaluate(&mut store, at(2)), Ok(()));
    assert_eq!(graph.evaluate(&mut store, at(3)), Ok(()));
    assert_eq!(seen(&graph, sum), [2 + 10, 3 + 20]);
}

/// Requires neither input, and asks instead.
struct Either {
    lhs: In<i64>,
    rhs: In<i64>,
    seen: Vec<(bool, bool)>,
}

impl Node for Either {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.seen.push((ctx.valid(self.lhs), ctx.valid(self.rhs)));
        Ok(())
    }
}

// NOD-2: only the inputs listed as required are looked at.
#[test]
fn nod2_an_input_that_is_not_required_does_not_hold_the_node_back() {
    let mut wiring = Wiring::default();
    let early = wiring.source(0, &[1, 2]);
    let late = wiring.source(1, &[10]);
    let lhs = wiring.input(early, true);
    let rhs = wiring.input(late, true);
    let seen = Vec::new();
    let either = wiring.add(
        Either { lhs, rhs, seen },
        node_type("either", false),
        vec![],
    );
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(1)), Ok(()));
    assert_eq!(graph.evaluate(&mut store, at(2)), Ok(()));

    let either = graph.node::<Either>(either).unwrap();
    assert_eq!(either.seen, [(true, false), (true, true)]);
}

/// Counts its evaluations and reads its input without being woken by it.
struct Watcher {
    input: In<i64>,
    evals: u32,
}

impl Node for Watcher {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.evals += 1;
        ctx.set_active(self.input, false);
        Ok(())
    }
}

// GRF-17, NOD-4: nothing schedules a node but an active input, its scheduler
// or schedule-on-start. A passive input does not, from the start or from the
// moment a node makes it passive.
#[test]
fn grf17_nod4_a_node_nothing_schedules_is_not_evaluated() {
    let mut wiring = Wiring::default();
    let ticks = wiring.source(0, &[1, 2, 3]);
    let input = wiring.input(ticks, false);
    let passive = wiring.add(
        Watcher { input, evals: 0 },
        node_type("passive", false),
        vec![],
    );
    let input = wiring.input(ticks, true);
    let once = wiring.add(
        Watcher { input, evals: 0 },
        node_type("once", false),
        vec![],
    );
    let (mut graph, mut store) = wiring.started();

    for cycle in 1..=3 {
        assert_eq!(graph.evaluate(&mut store, at(cycle)), Ok(()));
    }

    assert_eq!(graph.node::<Watcher>(passive).unwrap().evals, 0);
    assert_eq!(graph.node::<Watcher>(once).unwrap().evals, 1);
}

/// Ticks only the even values it reads.
struct Evens {
    input: In<i64>,
    out: Out<i64>,
}

impl Node for Evens {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let value = ctx.get(self.input);
        if value % 2 == 0 {
            ctx.set(self.out, value);
        }
        Ok(())
    }
}

// NOD-5: an eval that writes nothing causes no tick, so nothing downstream
// is woken by it.
#[test]
fn nod5_an_eval_that_writes_nothing_causes_no_tick() {
    let mut wiring = Wiring::default();
    let ticks = wiring.source(0, &[1, 2, 3, 4]);
    let input = wiring.input(ticks, true);
    let evens = wiring.output();
    wiring.add(
        Evens { input, out: evens },
        node_type("evens", false),
        vec![input.id()],
    );
    let (after, _) = wiring.add_one(evens);
    let (mut graph, mut store) = wiring.started();

    let mut ticked = Vec::new();
    for cycle in 1..=4 {
        assert_eq!(graph.evaluate(&mut store, at(cycle)), Ok(()));
        ticked.push(store.output_modified(evens.id(), at(cycle)));
    }

    assert_eq!(ticked, [false, true, false, true]);
    assert_eq!(graph.node::<AddOne>(after).unwrap().evals, 2);
}

/// Writes an output that belongs to another node.
#[cfg(debug_assertions)]
struct Trespasser {
    theirs: Out<i64>,
}

#[cfg(debug_assertions)]
impl Node for Trespasser {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(0))
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.theirs, 7);
        Ok(())
    }
}

// NOD-5: a node writes only its own output. `Ctx::set` names the node it was
// made for as the writer, and the store asserts that the writer is the owner.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "TS-21")]
fn nod5_writing_another_nodes_output_asserts_in_debug() {
    let mut wiring = Wiring::default();
    let theirs = wiring.source(5, &[1]);
    wiring.add(Trespasser { theirs }, node_type("trespasser", true), vec![]);
    let (mut graph, mut store) = wiring.started();

    let _panics = graph.evaluate(&mut store, at(1));
}

// GRF-13 (debug assertion): a node of rank 0 bound to the output of rank 1 is
// woken after the scan has passed it.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "GRF-13")]
fn grf13_waking_a_node_the_scan_has_passed_asserts_in_debug() {
    let mut wiring = Wiring::default();
    let backwards = wiring.store.add_input::<i64>(NodeId(0), true);
    let evals = 0;
    wiring.add(
        Watcher {
            input: backwards,
            evals,
        },
        node_type("early", false),
        vec![],
    );
    let ticks = wiring.source(0, &[1]);
    wiring.store.bind(backwards.id(), ticks.id()).unwrap();
    let (mut graph, mut store) = wiring.started();

    let _panics = graph.evaluate(&mut store, at(1));
}

/// Ticks its output, which is bound to its own active input.
#[cfg(debug_assertions)]
struct Echo {
    out: Out<i64>,
}

#[cfg(debug_assertions)]
impl Node for Echo {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(0))
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, 1);
        Ok(())
    }
}

// GRF-13 (debug assertion): a node that wakes itself is woken after the scan
// has passed it, as surely as one woken by a node of higher rank.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "GRF-13")]
fn grf13_a_node_that_wakes_itself_asserts_in_debug() {
    let mut wiring = Wiring::default();
    let out = wiring.output();
    wiring.input(out, true);
    wiring.add(Echo { out }, node_type("echo", true), vec![]);
    let (mut graph, mut store) = wiring.started();

    let _panics = graph.evaluate(&mut store, at(1));
}

/// Records the clock as each evaluation reads it.
struct ClockReader {
    input: In<i64>,
    out: Out<i64>,
    read: Vec<(EngineTime, EngineTime)>,
}

impl Node for ClockReader {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.read
            .push((ctx.evaluation_time(), ctx.next_cycle_evaluation_time()));
        ctx.set(self.out, ctx.get(self.input));
        Ok(())
    }
}

fn clock_reader(wiring: &mut Wiring, from: Out<i64>) -> (NodeId, Out<i64>) {
    let input = wiring.input(from, true);
    let out = wiring.output();
    let read = Vec::new();
    let node = ClockReader { input, out, read };
    let id = wiring.add(node, node_type("clock_reader", false), vec![input.id()]);
    (id, out)
}

// ENG-2, INJ-7: evaluation time, and the next cycle's, read the same for
// every node in a cycle, and are the time the owner gave.
#[test]
fn eng2_inj7_every_node_in_a_cycle_reads_the_same_evaluation_time() {
    let mut wiring = Wiring::default();
    let ticks = wiring.source(0, &[1, 2]);
    let (first, passed_on) = clock_reader(&mut wiring, ticks);
    let (second, _) = clock_reader(&mut wiring, passed_on);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(1)), Ok(()));
    assert_eq!(graph.evaluate(&mut store, at(2)), Ok(()));

    let expected = [(at(1), at(2)), (at(2), at(3))];
    assert_eq!(graph.node::<ClockReader>(first).unwrap().read, expected);
    assert_eq!(graph.node::<ClockReader>(second).unwrap().read, expected);
}

// ENG-2 (debug assertion): a cycle is later than the one before it.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "ENG-2")]
fn eng2_a_cycle_no_later_than_the_last_asserts_in_debug() {
    let mut wiring = Wiring::default();
    wiring.source(0, &[1, 2]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(1)), Ok(()));
    let _panics = graph.evaluate(&mut store, at(1));
}

// ENG-2, ENG-3 (debug assertion): once a graph has started, no cycle is
// before its start time.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "ENG-2")]
fn eng2_eng3_a_cycle_before_the_start_time_asserts_in_debug() {
    let mut wiring = Wiring::default();
    wiring.source(0, &[1]);
    let mut graph = Graph::new("test".to_owned(), wiring.slots);
    assert_eq!(graph.start(&mut wiring.store, at(10)), Ok(()));

    let _panics = graph.evaluate(&mut wiring.store, at(5));
}

// NOD-1 (debug assertion): eval is called only while started.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "NOD-1")]
fn nod1_evaluating_a_graph_that_has_not_started_asserts_in_debug() {
    let mut wiring = Wiring::default();
    wiring.source(0, &[1]);
    let mut graph = Graph::new("test".to_owned(), wiring.slots);

    let _panics = graph.evaluate(&mut wiring.store, at(1));
}
