//! A node's scheduler in its P1 form — one pending request — and the graph
//! schedule it writes to (specification: Node, "The scheduler"; Graph,
//! "Scheduling a node").

use hgl_kernel::{Ctx, Graph, Node, NodeResult, NodeSlot, Phase};
use hgl_store::{InputId, Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

const START: EngineTime = at(1);

const fn at(micros: i64) -> EngineTime {
    EngineTime::from_micros(micros)
}

fn node_type(name: &'static str, uses_scheduler: bool, schedule_on_start: bool) -> NodeType {
    NodeType {
        name,
        inputs: Vec::new(),
        output: None,
        scalars: Vec::new(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler,
        schedule_on_start,
    }
}

/// Asks for `in_start` when it starts and for `in_eval[n]` in its n-th
/// evaluation, as delays in microseconds. Records when it was evaluated and
/// whether its own request was why.
#[derive(Default)]
struct Timer {
    in_start: Option<i64>,
    in_eval: Vec<Option<i64>>,
    evals: Vec<(EngineTime, bool)>,
}

fn ask(ctx: &mut Ctx<'_>, delay: Option<i64>) -> NodeResult {
    match delay {
        Some(delay) => ctx.schedule_in(EngineDelta::from_micros(delay)),
        None => Ok(()),
    }
}

impl Node for Timer {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ask(ctx, self.in_start)
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let delay = self.in_eval.get(self.evals.len()).copied().flatten();
        self.evals
            .push((ctx.evaluation_time(), ctx.is_scheduled_now()));
        ask(ctx, delay)
    }
}

/// Ticks at each of `at`, given as delays from the start time, ascending.
struct Tick {
    out: Out<i64>,
    at: Vec<i64>,
    ticked: usize,
}

impl Node for Tick {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ask(ctx, self.at.first().copied())
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.set(self.out, 1);
        self.ticked += 1;
        let next = self.at.get(self.ticked);
        ask(ctx, next.map(|next| next - self.at[self.ticked - 1]))
    }
}

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

    fn timer(&mut self, in_start: Option<i64>, in_eval: &[Option<i64>]) -> NodeId {
        let timer = Timer {
            in_start,
            in_eval: in_eval.to_vec(),
            evals: Vec::new(),
        };
        self.add(timer, node_type("timer", true, false), Vec::new())
    }

    /// A timer that an input also wakes, at each of `woken_at` after the start
    /// time.
    fn woken_timer(&mut self, woken_at: &[i64], in_start: i64, in_eval: &[Option<i64>]) -> NodeId {
        let out = self.store.add_output(self.next());
        let tick = Tick {
            out,
            at: woken_at.to_vec(),
            ticked: 0,
        };
        self.add(tick, node_type("tick", true, false), Vec::new());
        let input = self.store.add_input::<i64>(self.next(), true);
        assert_eq!(self.store.bind(input.id(), out.id()), Ok(()));
        self.timer(Some(in_start), in_eval)
    }

    fn started(self) -> (Graph, Store) {
        let mut store = self.store;
        let mut graph = Graph::new("test".to_owned(), self.slots);
        assert_eq!(graph.start(&mut store, START), Ok(()));
        (graph, store)
    }
}

/// Evaluate at each next scheduled time until nothing is scheduled. Bounded,
/// so that a schedule that never empties fails a test instead of hanging it.
fn run(graph: &mut Graph, store: &mut Store) -> Vec<EngineTime> {
    let mut cycles = Vec::new();
    while graph.next_scheduled_time() != EngineTime::FOREVER && cycles.len() < 20 {
        let now = graph.next_scheduled_time();
        assert_eq!(graph.evaluate(store, now), Ok(()));
        cycles.push(now);
    }
    cycles
}

fn evals(graph: &Graph, timer: NodeId) -> Vec<(EngineTime, bool)> {
    let timer = graph.node::<Timer>(timer);
    timer.map(|timer| timer.evals.clone()).unwrap_or_default()
}

// NOD-12, NOD-14; card, "Done when": `schedule_in` re-arms and is consumed.
// During start the start time itself may be asked for.
#[test]
fn nod12_schedule_in_rearms_and_is_consumed() {
    let mut wiring = Wiring::default();
    let timer = wiring.timer(Some(0), &[Some(5), None]);
    let (mut graph, mut store) = wiring.started();
    assert_eq!(graph.next_scheduled_time(), at(1));

    assert_eq!(graph.evaluate(&mut store, at(1)), Ok(()));
    assert_eq!(graph.next_scheduled_time(), at(6), "re-armed");

    assert_eq!(graph.evaluate(&mut store, at(6)), Ok(()));
    assert_eq!(graph.next_scheduled_time(), EngineTime::FOREVER, "consumed");
    assert_eq!(evals(&graph, timer), [(at(1), true), (at(6), true)]);
}

// NOD-4: nothing is scheduled by default.
#[test]
fn nod4_a_graph_in_which_nothing_asks_has_nothing_scheduled() {
    let mut wiring = Wiring::default();
    wiring.timer(None, &[]);
    let (graph, _store) = wiring.started();

    assert_eq!(graph.next_scheduled_time(), EngineTime::FOREVER);
}

// NOD-4: schedule-on-start schedules the node for the start time. It is not
// the node's own request, and the node type need not use a scheduler.
#[test]
fn nod4_schedule_on_start_schedules_the_node_for_the_start_time() {
    let mut wiring = Wiring::default();
    let node = wiring.add(Timer::default(), node_type("constant", false, true), vec![]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [START]);
    assert_eq!(evals(&graph, node), [(START, false)]);
}

/// What start and the first cycle come to, for a timer that is node 1, and the
/// next scheduled time they leave.
fn outcome_of(in_start: Option<i64>, in_eval: &[Option<i64>]) -> (NodeResult, EngineTime) {
    let mut wiring = Wiring::default();
    wiring.timer(None, &[]);
    wiring.timer(in_start, in_eval);
    let mut graph = Graph::new("test".to_owned(), wiring.slots);
    let started = graph.start(&mut wiring.store, START);
    let outcome = started.and_then(|()| graph.evaluate(&mut wiring.store, START));
    (outcome, graph.next_scheduled_time())
}

// GRF-12, NOD-14; card, "Done when": scheduling into the past fails. The
// failure names the node and the hook.
#[test]
fn grf12_nod14_scheduling_into_the_past_fails() {
    let (outcome, next) = outcome_of(Some(0), &[Some(-1)]);
    let error = outcome.unwrap_err();

    assert_eq!((error.node, error.phase), (NodeId(1), Phase::Eval));
    assert_eq!(error.label, "timer");
    assert!(error.message.contains("GRF-12"), "{}", error.message);
    assert_eq!(next, EngineTime::FOREVER, "and nothing was scheduled");
}

// NOD-14: once started a node may ask only for the future, so not for the
// evaluation time itself; during start it may not ask for before the start.
#[test]
fn nod14_a_started_node_asks_only_for_the_future() {
    let in_eval = outcome_of(Some(0), &[Some(0)]).0.unwrap_err();
    assert_eq!((in_eval.node, in_eval.phase), (NodeId(1), Phase::Eval));
    assert!(in_eval.message.contains("NOD-14"), "{}", in_eval.message);

    let in_start = outcome_of(Some(-1), &[]).0.unwrap_err();
    assert_eq!((in_start.node, in_start.phase), (NodeId(1), Phase::Start));
    assert!(in_start.message.contains("NOD-14"), "{}", in_start.message);
}

// ENG-16: no time is after forever.
#[test]
fn eng16_a_request_for_after_forever_fails() {
    let outcome = outcome_of(Some(EngineTime::FOREVER.micros()), &[]).0;
    let error = outcome.unwrap_err();

    assert!(error.message.contains("ENG-16"), "{}", error.message);
}

// INJ-2: a node whose type does not ask for a scheduler has none.
#[test]
fn inj2_a_node_type_without_a_scheduler_cannot_schedule() {
    let mut wiring = Wiring::default();
    let timer = Timer {
        in_start: Some(0),
        ..Timer::default()
    };
    wiring.add(timer, node_type("timer", false, false), Vec::new());
    let mut graph = Graph::new("test".to_owned(), wiring.slots);

    let error = graph.start(&mut wiring.store, START).unwrap_err();

    assert!(error.message.contains("INJ-2"), "{}", error.message);
    assert_eq!(graph.next_scheduled_time(), EngineTime::FOREVER);
}

// NOD-12: a node woken early by an input has not used its request, which
// still falls due at its time.
#[test]
fn nod12_a_node_woken_early_by_an_input_keeps_its_request() {
    let mut wiring = Wiring::default();
    let timer = wiring.woken_timer(&[4], 10, &[]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [at(5), at(11)]);
    assert_eq!(evals(&graph, timer), [(at(5), false), (at(11), true)]);
}

// GRF-16, NOD-3: a node its own request and an input both wake in one cycle
// is evaluated once, and its request is why.
#[test]
fn grf16_nod3_a_node_woken_by_its_scheduler_and_an_input_at_once_is_evaluated_once() {
    let mut wiring = Wiring::default();
    let timer = wiring.woken_timer(&[4], 4, &[]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [at(5)]);
    assert_eq!(evals(&graph, timer), [(at(5), true)]);
}

// GRF-14: an entry moves earlier. The entry it replaces is stale, and is
// never reported as the next scheduled time.
#[test]
fn grf14_an_earlier_request_moves_the_entry_earlier_and_the_old_one_is_dropped() {
    let mut wiring = Wiring::default();
    let timer = wiring.woken_timer(&[4], 10, &[Some(2)]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(5)), Ok(()));
    assert_eq!(graph.next_scheduled_time(), at(7), "moved earlier from 11");

    assert_eq!(graph.evaluate(&mut store, at(7)), Ok(()));
    assert_eq!(graph.next_scheduled_time(), EngineTime::FOREVER, "not 11");
    assert_eq!(evals(&graph, timer), [(at(5), false), (at(7), true)]);
}

// GRF-14: the stale entry does not wake its node either, when another node's
// live entry brings a cycle at that time. The other node ranks first, so its
// live entry sits above the stale one and both are taken as the cycle begins.
#[test]
fn grf14_a_stale_entry_does_not_wake_its_node() {
    let mut wiring = Wiring::default();
    let other = wiring.timer(Some(10), &[]);
    let timer = wiring.woken_timer(&[4], 10, &[Some(2)]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [at(5), at(7), at(11)]);
    assert_eq!(evals(&graph, timer), [(at(5), false), (at(7), true)]);
    assert_eq!(evals(&graph, other), [(at(11), true)]);
}

// GRF-14: however many stale entries lie above the next live one, none is
// reported as the next scheduled time, so none brings a cycle of its own.
#[test]
fn grf14_every_stale_entry_above_the_next_live_one_is_dropped() {
    let mut wiring = Wiring::default();
    wiring.woken_timer(&[4], 10, &[Some(2)]);
    wiring.woken_timer(&[4], 11, &[Some(2)]);
    wiring.timer(Some(19), &[]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [at(5), at(7), at(20)]);
}

// GRF-14: a stale entry lying above its node's own later live entry is not
// reported as the next scheduled time either. Woken at 5, the timer moves its
// request from 11 to 7; at 7 it asks for 17.
#[test]
fn grf14_a_stale_entry_above_its_nodes_later_entry_is_dropped() {
    let mut wiring = Wiring::default();
    wiring.woken_timer(&[4], 10, &[Some(2), Some(10)]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [at(5), at(7), at(17)]);
}

// GRF-14: a visit uses the node's entry, whatever woke it. Woken by an input
// at 5, the timer replaces its request for 11 with one for 25; it is next
// evaluated at 25, and not also at 11 (as hgraph: an input's wake overwrites
// the entry, and the request is written back after the eval).
#[test]
fn grf14_a_visit_uses_the_entry_so_a_later_request_replaces_it() {
    let mut wiring = Wiring::default();
    let timer = wiring.woken_timer(&[4], 10, &[Some(20)]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [at(5), at(25)]);
    assert_eq!(evals(&graph, timer), [(at(5), false), (at(25), true)]);
}

// GRF-14: a debounce, re-armed five steps ahead on each input tick, runs once
// after the last tick and at none of the times it was re-armed away from.
#[test]
fn grf14_a_debounce_runs_once_after_its_last_tick() {
    let mut wiring = Wiring::default();
    let timer = wiring.woken_timer(&[1, 3, 5, 7], 5, &[Some(5); 4]);
    let (mut graph, mut store) = wiring.started();

    let cycles = run(&mut graph, &mut store);

    assert_eq!(cycles, [at(2), at(4), at(6), at(8), at(13)]);
    assert_eq!(evals(&graph, timer).last(), Some(&(at(13), true)));
}

// NOD-4, GRF-14: schedule-on-start holds the start time even when the node
// asks in start for a later one, and the request keeps its own time.
#[test]
fn nod4_schedule_on_start_is_not_moved_by_a_later_request_made_in_start() {
    let mut wiring = Wiring::default();
    let timer = Timer {
        in_start: Some(5),
        ..Timer::default()
    };
    let node = wiring.add(timer, node_type("timer", true, true), vec![]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [START, at(6)]);
    assert_eq!(evals(&graph, node), [(START, false), (at(6), true)]);
}

// NOD-12, GRF-14: a failing eval cuts the pass short, and leaves the schedule
// as a completed pass would. The node it did not reach loses the cycle and
// what it had asked for in it, and no stale entry is the next time.
#[test]
fn nod12_a_pass_cut_short_by_a_failure_leaves_nothing_due_and_nothing_stale() {
    let mut wiring = Wiring::default();
    wiring.woken_timer(&[4], 10, &[Some(2)]);
    wiring.timer(Some(6), &[Some(-1)]);
    let unreached = wiring.timer(Some(6), &[]);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(graph.evaluate(&mut store, at(5)), Ok(()));
    assert!(
        graph.evaluate(&mut store, at(7)).is_err(),
        "asks for the past"
    );

    assert!(evals(&graph, unreached).is_empty());
    assert_eq!(
        graph.next_scheduled_time(),
        EngineTime::FOREVER,
        "not 7 or 11"
    );
}

// NOD-12: a node scheduled but not admitted still uses up its request, so no
// further cycle is brought at that time.
#[test]
fn nod12_a_node_scheduled_but_not_admitted_uses_up_its_request() {
    let mut wiring = Wiring::default();
    let never_valid = wiring.store.add_input::<i64>(NodeId(0), true);
    let timer = Timer {
        in_start: Some(0),
        ..Timer::default()
    };
    let required = vec![never_valid.id()];
    let timer = wiring.add(timer, node_type("timer", true, false), required);
    let (mut graph, mut store) = wiring.started();

    assert_eq!(run(&mut graph, &mut store), [START]);
    assert!(evals(&graph, timer).is_empty());
}

// ENG-4 (debug assertion): a graph's owner does not evaluate past work that
// is due.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "ENG-4")]
fn eng4_a_cycle_that_skips_a_scheduled_time_asserts_in_debug() {
    let mut wiring = Wiring::default();
    wiring.timer(Some(3), &[]);
    let (mut graph, mut store) = wiring.started();

    let _panics = graph.evaluate(&mut store, at(9));
}
