//! A graph's one life: start in rank order, stop in reverse, and what a
//! failure in either leaves behind (specification: Graph, "Start and stop";
//! Node, "Start", "Stop").

use std::cell::RefCell;
use std::rc::Rc;

use hgl_kernel::{Ctx, Graph, Lifecycle, Node, NodeError, NodeResult, NodeSlot, Phase};
use hgl_store::{Out, Store};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

const START: EngineTime = EngineTime::from_micros(1);

fn slot(label: &str, node: impl Node) -> NodeSlot {
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
        required: Vec::new(),
    }
}

/// Every hook of every node of a graph, in the order they ran. Shared, which
/// the tick path bans and a test of ordering across nodes needs.
type Log = Rc<RefCell<Vec<String>>>;

/// Logs its hooks, and fails in the ones it is told to. Evaluated once, at
/// the start time.
struct Logged {
    name: &'static str,
    log: Log,
    fails_in: Vec<Phase>,
}

impl Logged {
    fn ran(&self, hook: &str, phase: Phase) -> NodeResult {
        self.log.borrow_mut().push(format!("{hook} {}", self.name));
        if self.fails_in.contains(&phase) {
            return Err(NodeError::new(format!("{} failed", self.name)));
        }
        Ok(())
    }
}

impl Node for Logged {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        ctx.schedule_in(EngineDelta::from_micros(0))?;
        self.ran("start", Phase::Start)
    }

    fn eval(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        self.ran("eval", Phase::Eval)
    }

    fn stop(&mut self, _ctx: &mut Ctx<'_>) -> NodeResult {
        self.ran("stop", Phase::Stop)
    }
}

/// A graph of `Logged` nodes `a`, `b`, `c`, ..., each failing as given.
fn logged(fails_in: &[&[Phase]]) -> (Graph, Store, Log) {
    let log = Log::default();
    let names = ["a", "b", "c", "d"];
    let slots = names.iter().zip(fails_in).map(|(&name, &fails_in)| {
        let node = Logged {
            name,
            log: Rc::clone(&log),
            fails_in: fails_in.to_vec(),
        };
        slot(name, node)
    });
    let graph = Graph::new("test".to_owned(), slots.collect());
    (graph, Store::new(), log)
}

fn entries(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

// GRF-18, NOD-1: start in rank order, stop in reverse, each once.
#[test]
fn grf18_nodes_start_in_rank_order_and_stop_in_reverse() {
    let (mut graph, mut store, log) = logged(&[&[], &[], &[]]);
    assert_eq!(graph.lifecycle(), Lifecycle::Instantiated);

    assert_eq!(graph.start(&mut store, START), Ok(()));
    assert_eq!(graph.lifecycle(), Lifecycle::Started);
    assert_eq!(graph.evaluate(&mut store, START), Ok(()));
    assert_eq!(graph.lifecycle(), Lifecycle::Started);
    assert_eq!(graph.stop(&mut store, START), Ok(()));
    assert_eq!(graph.lifecycle(), Lifecycle::Stopped);

    let expected = [
        "start a", "start b", "start c", "eval a", "eval b", "eval c", "stop c", "stop b", "stop a",
    ];
    assert_eq!(entries(&log), expected);
}

// GRF-18, NOD-11; card, "Done when": start failure rolls back in reverse
// without stopping the failed node.
#[test]
fn grf18_nod11_a_start_failure_stops_the_started_nodes_in_reverse_and_not_the_failed_one() {
    let (mut graph, mut store, log) = logged(&[&[], &[], &[Phase::Start], &[]]);

    let error = graph.start(&mut store, START).unwrap_err();

    assert_eq!((error.node, error.phase), (NodeId(2), Phase::Start));
    assert_eq!(
        (error.label.as_str(), error.message.as_str()),
        ("c", "c failed")
    );
    let expected = ["start a", "start b", "start c", "stop b", "stop a"];
    assert_eq!(entries(&log), expected);
    assert_eq!(graph.lifecycle(), Lifecycle::Stopped);
    let nothing = EngineTime::FOREVER;
    assert_eq!(graph.next_scheduled_time(), nothing, "a and b had asked");

    assert_eq!(graph.stop(&mut store, START), Ok(()));
    assert_eq!(entries(&log), expected, "nothing is left to stop");
}

// GRF-18, NOD-11: the start failure is what the owner receives, even when a
// node stopped in the rollback fails as well.
#[test]
fn grf18_nod11_a_start_failure_is_reported_over_a_failing_rollback() {
    let (mut graph, mut store, log) = logged(&[&[Phase::Stop], &[Phase::Start]]);

    let error = graph.start(&mut store, START).unwrap_err();

    assert_eq!((error.node, error.phase), (NodeId(1), Phase::Start));
    assert_eq!(entries(&log), ["start a", "start b", "stop a"]);
}

// GRF-18, NOD-11, ENG-10; card, "Done when": every started node is stopped
// once. A failing stop does not spare the others theirs, and the first
// failure is the one returned.
#[test]
fn grf18_nod11_every_started_node_is_stopped_once_and_the_first_failure_is_returned() {
    let (mut graph, mut store, log) = logged(&[&[Phase::Stop], &[], &[Phase::Stop]]);
    assert_eq!(graph.start(&mut store, START), Ok(()));

    let error = graph.stop(&mut store, START).unwrap_err();

    assert_eq!((error.node, error.phase), (NodeId(2), Phase::Stop));
    assert_eq!(error.message, "c failed");
    assert_eq!(entries(&log)[3..], ["stop c", "stop b", "stop a"]);

    assert_eq!(graph.stop(&mut store, START), Ok(()));
    assert_eq!(entries(&log).len(), 6, "stopped once, however often asked");
}

// GRF-20, ENG-1: a graph is started once.
#[test]
fn grf20_a_graph_is_never_started_again() {
    let (mut graph, mut store, log) = logged(&[&[], &[]]);
    assert_eq!(graph.start(&mut store, START), Ok(()));

    let while_started = graph.start(&mut store, START).unwrap_err();
    assert!(while_started.message.contains("GRF-20"));
    let named = (while_started.node, while_started.label.as_str());
    assert_eq!(named, (NodeId(0), "test"), "the graph, not node a");
    assert_eq!(
        graph.lifecycle(),
        Lifecycle::Started,
        "and is left as it was"
    );

    assert_eq!(graph.stop(&mut store, START), Ok(()));
    let once_stopped = graph.start(&mut store, START).unwrap_err();
    assert!(once_stopped.message.contains("GRF-20"));
    assert_eq!(once_stopped.phase, Phase::Start);
    assert_eq!(graph.lifecycle(), Lifecycle::Stopped);

    let expected = ["start a", "start b", "stop b", "stop a"];
    assert_eq!(entries(&log), expected, "no hook ran again");
}

// NOD-1: a node is created, started and stopped, in that order, so a graph
// that never started has nothing to stop, and is left to be started.
#[test]
fn nod1_stopping_a_graph_that_never_started_does_nothing() {
    let (mut graph, mut store, log) = logged(&[&[], &[]]);

    assert_eq!(graph.stop(&mut store, START), Ok(()));
    assert_eq!(graph.lifecycle(), Lifecycle::Instantiated);

    assert_eq!(graph.start(&mut store, START), Ok(()));
    assert_eq!(entries(&log), ["start a", "start b"]);
}

// P1 has no captured errors (NOD-19 is P2): a failing eval ends the pass
// there, names the node, and leaves the graph for its owner to stop.
#[test]
fn a_failing_eval_ends_the_pass_and_the_graph_is_still_stopped_in_full() {
    let (mut graph, mut store, log) = logged(&[&[], &[Phase::Eval], &[]]);
    assert_eq!(graph.start(&mut store, START), Ok(()));

    let error = graph.evaluate(&mut store, START).unwrap_err();

    assert_eq!((error.node, error.phase), (NodeId(1), Phase::Eval));
    assert_eq!(
        (error.label.as_str(), error.message.as_str()),
        ("b", "b failed")
    );
    assert_eq!(graph.lifecycle(), Lifecycle::Evaluating);
    assert_eq!(entries(&log)[3..], ["eval a", "eval b"]);

    assert_eq!(graph.stop(&mut store, START), Ok(()));
    assert_eq!(entries(&log)[5..], ["stop c", "stop b", "stop a"]);
    assert_eq!(graph.lifecycle(), Lifecycle::Stopped);
}

/// Reads its own output in every hook, and writes it where it is told to.
struct OwnOutput {
    out: Out<i64>,
    writes_in: Phase,
    read: Vec<Option<i64>>,
}

impl OwnOutput {
    fn hook(&mut self, ctx: &mut Ctx<'_>, phase: Phase) {
        self.read.push(ctx.output_value(self.out));
        if phase == self.writes_in {
            ctx.set(self.out, 7);
        }
    }
}

impl Node for OwnOutput {
    fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.hook(ctx, Phase::Start);
        ctx.schedule_in(EngineDelta::from_micros(0))
    }

    fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.hook(ctx, Phase::Eval);
        Ok(())
    }

    fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.hook(ctx, Phase::Stop);
        Ok(())
    }
}

/// Start, one cycle, stop, for a node that writes its output in `writes_in`.
fn own_output_read(writes_in: Phase) -> Vec<Option<i64>> {
    let mut store = Store::new();
    let out = store.add_output(NodeId(0));
    let node = OwnOutput {
        out,
        writes_in,
        read: Vec::new(),
    };
    let mut graph = Graph::new("test".to_owned(), vec![slot("own", node)]);
    assert_eq!(graph.start(&mut store, START), Ok(()));
    assert_eq!(graph.evaluate(&mut store, START), Ok(()));
    assert_eq!(graph.stop(&mut store, START), Ok(()));
    let node = graph.node::<OwnOutput>(NodeId(0));
    node.map(|node| node.read.clone()).unwrap_or_default()
}

// NOD-22, INJ-8: a node may read its own output in start, eval and stop.
#[test]
fn nod22_inj8_a_node_reads_its_own_output_in_every_hook() {
    assert_eq!(own_output_read(Phase::Eval), [None, None, Some(7)]);
}

// NOD-22, INJ-8 (debug assertion): it writes it only in eval.
#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "NOD-22")]
fn nod22_inj8_writing_the_output_in_start_asserts_in_debug() {
    own_output_read(Phase::Start);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "NOD-22")]
fn nod22_inj8_writing_the_output_in_stop_asserts_in_debug() {
    own_output_read(Phase::Stop);
}

// Card, `Graph::node`: a node comes back as its own type, or not at all.
#[test]
fn node_gives_a_node_back_as_its_own_type() {
    let (mut graph, _store, _log) = logged(&[&[], &[]]);

    assert_eq!(graph.node::<Logged>(NodeId(1)).unwrap().name, "b");
    assert!(graph.node::<OwnOutput>(NodeId(1)).is_none(), "another type");
    assert!(graph.node::<Logged>(NodeId(2)).is_none(), "no such node");

    graph.node_mut::<Logged>(NodeId(0)).unwrap().name = "changed";
    assert_eq!(graph.node::<Logged>(NodeId(0)).unwrap().name, "changed");
    assert!(graph.node_mut::<OwnOutput>(NodeId(0)).is_none());
    assert!(graph.node_mut::<Logged>(NodeId(2)).is_none());
}
