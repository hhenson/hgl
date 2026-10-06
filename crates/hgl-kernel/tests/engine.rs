//! The simulation engine: when cycles run, and how a run ends
//! (specification: Execution engine, "The evaluation loop", "Ending").

use hgl_kernel::{
    Ctx, EngineError, Graph, Lifecycle, Node, NodeError, NodeResult, NodeSlot, Phase, RunConfig,
    run_simulation,
};
use hgl_store::{In, Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

const fn at(micros: i64) -> EngineTime {
    EngineTime::from_micros(micros)
}

fn slot(label: &str, node: impl Node, required: &[In<i64>]) -> NodeSlot {
    let node_type = NodeType {
        name: "test",
        inputs: Vec::new(),
        output: None,
        scalars: Vec::new(),
        active_inputs: None,
        valid_inputs: None,
        uses_scheduler: true,
        schedule_on_start: false,
        uses_global_state: false,
        global_entries: Vec::new(),
        child_graphs: 0,
    };
    NodeSlot {
        node: Box::new(node),
        node_type,
        label: label.to_owned(),
        required: required.iter().map(|input| input.id()).collect(),
    }
}

/// What a `Pulse` does besides ticking, and in which of its evaluations,
/// counted from zero.
#[derive(Default, Clone, Copy, PartialEq)]
enum Also {
    #[default]
    Nothing,
    RequestStopInStart,
    RequestStopIn(i64),
    FailIn(i64),
    FailInStop,
}

/// Ticks 0, 1, 2, ... every `every` microseconds from the start time, and
/// records the evaluation time each of its hooks read.
#[derive(Default)]
struct Pulse {
    out: Option<Out<i64>>,
    every: i64,
    also: Also,
    ticks: i64,
    hooks: Vec<(Phase, EngineTime)>,
}

impl Node for Pulse {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.hooks.push((Phase::Start, ctx.evaluation_time()));
        if self.also == Also::RequestStopInStart {
            ctx.request_stop();
        }
        ctx.schedule_in(EngineDelta::from_micros(0))
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.hooks.push((Phase::Eval, ctx.evaluation_time()));
        if let Some(out) = self.out {
            ctx.set(out, self.ticks);
        }
        if self.also == Also::RequestStopIn(self.ticks) {
            ctx.request_stop();
        }
        if self.also == Also::FailIn(self.ticks) {
            return Err(NodeError::new("pulse failed"));
        }
        self.ticks += 1;
        ctx.schedule_in(EngineDelta::from_micros(self.every))
    }

    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.hooks.push((Phase::Stop, ctx.evaluation_time()));
        if self.also == Also::FailInStop {
            return Err(NodeError::new("pulse failed to stop"));
        }
        Ok(())
    }
}

/// Records every tick it is woken by.
struct Follower {
    input: In<i64>,
    ticks: Vec<(EngineTime, i64)>,
}

impl Node for Follower {
    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.ticks
            .push((ctx.evaluation_time(), ctx.get(self.input)));
        Ok(())
    }
}

/// Node 0 is a pulse; node 1 follows it.
fn pulse_and_follower(every: i64, also: Also) -> (Graph, Store) {
    let mut store = Store::new();
    let out = store.add_output(NodeId(0));
    let input = store.add_input(NodeId(1), true);
    assert_eq!(store.bind(input.id(), out.id()), Ok(()));
    let pulse = Pulse {
        out: Some(out),
        every,
        also,
        ..Pulse::default()
    };
    let ticks = Vec::new();
    let slots = vec![
        slot("pulse", pulse, &[]),
        slot("follower", Follower { input, ticks }, &[input]),
    ];
    (Graph::new("test".to_owned(), slots), store)
}

/// Pulses alone, none with an output, one for each `(every, also)`.
fn pulses(each: &[(i64, Also)]) -> (Graph, Store) {
    let slots = each.iter().map(|&(every, also)| {
        let pulse = Pulse {
            every,
            also,
            ..Pulse::default()
        };
        slot("pulse", pulse, &[])
    });
    (Graph::new("test".to_owned(), slots.collect()), Store::new())
}

fn run(graph: &mut Graph, store: &mut Store, start: i64, end: i64) -> Result<u64, EngineError> {
    let config = RunConfig {
        start_time: at(start),
        end_time: at(end),
    };
    run_simulation(graph, store, &config)
}

fn hooks(graph: &Graph, node: u32) -> Vec<(Phase, EngineTime)> {
    let pulse = graph.node::<Pulse>(NodeId(node));
    pulse.map(|pulse| pulse.hooks.clone()).unwrap_or_default()
}

/// The times of a pulse's evaluations.
fn evals(graph: &Graph, node: u32) -> Vec<i64> {
    let hooks = hooks(graph, node);
    let evals = hooks.iter().filter(|(phase, _)| *phase == Phase::Eval);
    evals.map(|(_, time)| time.micros()).collect()
}

fn follower_ticks(graph: &Graph) -> Vec<(EngineTime, i64)> {
    let follower = graph.node::<Follower>(NodeId(1));
    follower
        .map(|follower| follower.ticks.clone())
        .unwrap_or_default()
}

// ENG-3; card, "Done when": the end time is exclusive. The start time is
// inclusive.
#[test]
fn eng3_the_start_time_is_inclusive_and_the_end_time_is_exclusive() {
    let (mut graph, mut store) = pulses(&[(1, Also::Nothing)]);

    assert_eq!(run(&mut graph, &mut store, 10, 13), Ok(3));

    assert_eq!(evals(&graph, 0), [10, 11, 12]);
}

// ENG-4: a cycle at every scheduled time before the end, none skipped and no
// two merged. A time two nodes share is one cycle.
#[test]
fn eng4_every_scheduled_time_is_one_cycle() {
    let (mut graph, mut store) = pulses(&[(2, Also::Nothing), (3, Also::Nothing)]);

    assert_eq!(run(&mut graph, &mut store, 10, 17), Ok(5));

    assert_eq!(evals(&graph, 0), [10, 12, 14, 16]);
    assert_eq!(evals(&graph, 1), [10, 13, 16]);
}

/// Started and stopped, and never scheduled.
struct Idle;

impl Node for Idle {
    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

// ENG-5: a simulation with nothing scheduled before the end time is finished.
#[test]
fn eng5_the_run_ends_when_nothing_is_scheduled_before_the_end_time() {
    let mut idle = Graph::new("test".to_owned(), vec![slot("idle", Idle, &[])]);
    assert_eq!(run(&mut idle, &mut Store::new(), 10, 1_000_000), Ok(0));
    assert_eq!(idle.lifecycle(), Lifecycle::Stopped);

    let (mut graph, mut store) = pulses(&[(600_000, Also::Nothing)]);
    assert_eq!(run(&mut graph, &mut store, 10, 1_000_000), Ok(2));
    assert_eq!(evals(&graph, 0), [10, 600_010]);
}

// ENG-9, INJ-10; card, "Done when": `request_stop` lets the cycle finish. The
// follower is evaluated in the cycle that asked, and no cycle follows it.
#[test]
fn eng9_inj10_request_stop_lets_the_cycle_finish_and_prevents_the_next() {
    let (mut graph, mut store) = pulse_and_follower(1, Also::RequestStopIn(1));

    assert_eq!(run(&mut graph, &mut store, 10, 100), Ok(2));

    assert!(graph.stop_requested());
    assert_eq!(follower_ticks(&graph), [(at(10), 0), (at(11), 1)]);
    assert_eq!(hooks(&graph, 0).last(), Some(&(Phase::Stop, at(11))));
}

// ENG-9: a stop asked for during start leaves no cycle to run (as hgraph,
// which looks at the request before each cycle, the first included).
#[test]
fn eng9_a_stop_requested_in_start_runs_no_cycle() {
    let (mut graph, mut store) = pulse_and_follower(1, Also::RequestStopInStart);

    assert_eq!(run(&mut graph, &mut store, 10, 100), Ok(0));

    assert_eq!(
        hooks(&graph, 0),
        [(Phase::Start, at(10)), (Phase::Stop, at(10))]
    );
}

// ENG-10: a failure ends the run; every node that started is stopped, and
// then the failure is reported.
#[test]
fn eng10_a_failing_eval_stops_every_node_then_is_reported() {
    let (mut graph, mut store) = pulses(&[(1, Also::Nothing), (1, Also::FailIn(2))]);

    let Err(EngineError::Node(error)) = run(&mut graph, &mut store, 10, 100) else {
        panic!("the run must fail");
    };

    assert_eq!((error.node, error.phase), (NodeId(1), Phase::Eval));
    assert_eq!(error.message, "pulse failed");
    assert_eq!(evals(&graph, 1), [10, 11, 12]);
    assert_eq!(hooks(&graph, 0).last(), Some(&(Phase::Stop, at(12))));
    assert_eq!(hooks(&graph, 1).last(), Some(&(Phase::Stop, at(12))));
    assert_eq!(graph.lifecycle(), Lifecycle::Stopped);
}

// ENG-10: of a failing eval and a failing stop, the first is what is reported.
#[test]
fn eng10_the_first_of_several_failures_is_reported() {
    let (mut graph, mut store) = pulses(&[(1, Also::FailInStop), (1, Also::FailIn(0))]);

    let Err(EngineError::Node(error)) = run(&mut graph, &mut store, 10, 100) else {
        panic!("the run must fail");
    };

    assert_eq!((error.node, error.phase), (NodeId(1), Phase::Eval));
    assert_eq!(error.cleanup.len(), 1);
    assert_eq!(error.cleanup[0].phase, Phase::Stop);
    assert_eq!(error.cleanup[0].message, "pulse failed to stop");
    assert_eq!(hooks(&graph, 0).last(), Some(&(Phase::Stop, at(10))));
}

// ENG-10: a run that ends normally reports a failure to stop.
#[test]
fn eng10_a_failing_stop_is_reported() {
    let (mut graph, mut store) = pulses(&[(1, Also::FailInStop)]);

    let Err(EngineError::Node(error)) = run(&mut graph, &mut store, 10, 12) else {
        panic!("the run must fail");
    };

    assert_eq!((error.node, error.phase), (NodeId(0), Phase::Stop));
}

/// Cannot start.
struct Broken;

impl Node for Broken {
    fn start(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Err(NodeError::new("broken"))
    }

    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        Ok(())
    }
}

// ENG-10: a node that fails to start ends the run before any cycle; the
// nodes that did start are stopped.
#[test]
fn eng10_a_start_failure_is_reported_and_no_cycle_runs() {
    let slots = vec![
        slot("pulse", Pulse::default(), &[]),
        slot("broken", Broken, &[]),
    ];
    let mut graph = Graph::new("test".to_owned(), slots);

    let Err(EngineError::Node(error)) = run(&mut graph, &mut Store::new(), 10, 100) else {
        panic!("the run must fail");
    };

    assert_eq!((error.node, error.phase), (NodeId(1), Phase::Start));
    assert_eq!(error.label, "broken");
    assert_eq!(
        hooks(&graph, 0),
        [(Phase::Start, at(10)), (Phase::Stop, at(10))]
    );
}

// Specification, "Run configuration": the start time is the earliest start
// or later; the end time is later than it, and the latest end or earlier.
// The graphs are empty, so that a run wrongly accepted ends at once.
#[test]
fn a_run_configured_outside_the_permitted_times_is_refused() {
    let latest = EngineTime::MAX_END.micros();
    for (start, end) in [(0, 10), (10, 10), (10, 9), (10, latest + 1)] {
        let (mut graph, mut store) = pulses(&[]);

        let refused = run(&mut graph, &mut store, start, end);

        assert_eq!(refused, Err(EngineError::BadTimes), "{start}..{end}");
        assert_eq!(graph.lifecycle(), Lifecycle::Instantiated);
    }
    let (mut graph, mut store) = pulses(&[]);
    assert_eq!(run(&mut graph, &mut store, 1, latest), Ok(0), "the widest");
}

// ENG-1: a run takes its graph through start, cycles and stop once. There is
// no restart.
#[test]
fn eng1_a_graph_is_run_once() {
    let (mut graph, mut store) = pulses(&[(1, Also::Nothing)]);
    assert_eq!(run(&mut graph, &mut store, 10, 12), Ok(2));
    let first_run = hooks(&graph, 0);

    let Err(EngineError::Node(error)) = run(&mut graph, &mut store, 20, 22) else {
        panic!("the second run must fail");
    };

    assert!(error.message.contains("GRF-20"), "{}", error.message);
    assert_eq!(hooks(&graph, 0), first_run, "no hook ran again");
}

// ENG-11, INJ-6: only the engine advances the clock; every hook reads what
// the engine set. Start reads the start time, stop the last cycle's (as
// hgraph's executor, which leaves the loop before advancing the clock). That
// a node cannot move the clock is held by `Ctx`'s signature, which has
// nothing that writes it; no test of that half can fail.
#[test]
fn eng11_inj6_nodes_observe_the_clock_the_engine_advances() {
    let (mut graph, mut store) = pulses(&[(5, Also::Nothing)]);

    assert_eq!(run(&mut graph, &mut store, 10, 21), Ok(3));

    let expected = [
        (Phase::Start, at(10)),
        (Phase::Eval, at(10)),
        (Phase::Eval, at(15)),
        (Phase::Eval, at(20)),
        (Phase::Stop, at(20)),
    ];
    assert_eq!(hooks(&graph, 0), expected);
}

// ENG-15: the same graph, start time and end time give the same cycles and
// the same ticks.
#[test]
fn eng15_a_simulation_is_deterministic() {
    let (mut first, mut first_store) = pulse_and_follower(3, Also::RequestStopIn(4));
    let (mut second, mut second_store) = pulse_and_follower(3, Also::RequestStopIn(4));

    let first_cycles = run(&mut first, &mut first_store, 10, 100);
    let second_cycles = run(&mut second, &mut second_store, 10, 100);

    assert_eq!(first_cycles, Ok(5));
    assert_eq!(first_cycles, second_cycles);
    assert_eq!(follower_ticks(&first).len(), 5);
    assert_eq!(follower_ticks(&first), follower_ticks(&second));
    assert_eq!(hooks(&first, 0), hooks(&second, 0));
}
