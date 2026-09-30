//! Test support for the runtime.
//!
//! [`cases`] holds what nodes must do, as ticks in and ticks out. [`run`] is
//! the harness that drives a node through one case and compares what it
//! produced. It is the equivalent of hgraph's `eval_node`, and like it is built
//! from ordinary nodes — a [`Replay`] source and a [`Record`] sink — so that it
//! tests the runtime through its own front door.
//!
//! The caller supplies the nodes: the registry it hands to [`run`] must hold
//! the case's node and the harness's own two.

pub mod cases;
mod fixtures;

use std::fmt;

use hgl_describe::{BuildError, Builder, GraphDescription, NodeDescription, Registry, instantiate};
use hgl_kernel::{EngineError, RunConfig, run_simulation};
use hgl_store::Store;
use hgl_types::{EngineDelta, EngineTime, NodeId, ScalarValue};

use cases::Case;
use fixtures::{CYCLES, RECORD, REPLAY, SLOT};
pub use fixtures::{Record, Replay};

/// Why a case did not pass.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    /// No implementation is registered under this name, or what is registered
    /// under it is not the node the harness asked for.
    UnknownNode(&'static str),
    /// The case's node has no output, so there is nothing to compare.
    NoOutput(&'static str),
    /// The graph the case describes was refused.
    Build(BuildError),
    /// The run did not complete.
    Engine(EngineError),
    /// The output differed from what the case expects.
    Mismatch {
        /// The engine cycle, counted from zero.
        cycle: usize,
        /// What the case expected in that cycle; `None` is *no tick*.
        expected: Option<ScalarValue>,
        /// What the node produced.
        actual: Option<ScalarValue>,
    },
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownNode(name) => write!(out, "no node is registered as '{name}'"),
            Self::NoOutput(name) => write!(out, "'{name}' has no output to compare"),
            Self::Build(error) => write!(out, "the graph was refused: {error:?}"),
            Self::Engine(error) => write!(out, "the run failed: {error:?}"),
            Self::Mismatch {
                cycle,
                expected,
                actual,
            } => write!(out, "cycle {cycle}: expected {expected:?}, got {actual:?}"),
        }
    }
}

impl std::error::Error for Failure {}

/// Drive the case's node through its inputs and compare its output.
///
/// One graph per case: a [`Replay`] for each input, the node under test, a
/// [`Record`] on its output. It runs from `EngineTime::MIN_START`, ends after
/// `case.cycles` cycles when the case sets them and otherwise when nothing is
/// scheduled, and compares cycle by cycle.
///
/// # Errors
///
/// The first [`Failure`]: a node the registry does not hold, a node with no
/// output, a graph the builder refused, a run that failed, or the first cycle
/// whose output differs.
pub fn run(case: &Case, registry: &Registry) -> Result<(), Failure> {
    let Some(node_type) = registry.node_type(case.node) else {
        return Err(Failure::UnknownNode(case.node));
    };
    if node_type.output.is_none() {
        return Err(Failure::NoOutput(case.node));
    }
    let description = describe(case, registry).map_err(Failure::Build)?;
    let mut store = Store::new();
    let mut graph = instantiate(&description, registry, &mut store).map_err(Failure::Build)?;
    for (slot, &(_, ticks)) in case.inputs.iter().enumerate() {
        let replay = position(&description.nodes, REPLAY, Some(slot))
            .and_then(|id| graph.node_mut::<Replay>(id))
            .ok_or(Failure::UnknownNode(REPLAY))?;
        replay.load(ticks);
    }
    let config = RunConfig {
        start_time: EngineTime::MIN_START,
        end_time: end_time(case.cycles),
    };
    run_simulation(&mut graph, &mut store, &config).map_err(Failure::Engine)?;
    let record = position(&description.nodes, RECORD, None)
        .and_then(|id| graph.node::<Record>(id))
        .ok_or(Failure::UnknownNode(RECORD))?;
    compare(case.expected, record.seen())
}

/// The graph one case needs, in the order the fixtures wire it.
fn describe(case: &Case, registry: &Registry) -> Result<GraphDescription, BuildError> {
    let mut builder = Builder::new(case.name, registry);
    let mut replays = Vec::with_capacity(case.inputs.len());
    for slot in 0..case.inputs.len() {
        replays.push(builder.node(REPLAY, &[(SLOT, count(slot))])?);
    }
    let under_test = builder.node(case.node, case.scalars)?;
    for (&replay, &(input, _)) in replays.iter().zip(case.inputs) {
        builder.connect(replay, under_test, input)?;
    }
    // Every cycle the run can reach: what the case runs for, what it expects,
    // and the longest sequence a replay emits. One further, so that a tick the
    // case did not expect is a difference the comparison can name rather than
    // a tick the record refuses.
    let longest = case.inputs.iter().map(|(_, ticks)| ticks.len()).max();
    let reachable = case.cycles.unwrap_or(0).max(case.expected.len());
    let cycles = reachable.max(longest.unwrap_or(0)) + 1;
    let record = builder.node(RECORD, &[(CYCLES, count(cycles))])?;
    builder.connect(under_test, record, "in")?;
    builder.finish()
}

/// ENG-3: the end time is exclusive, so `cycles` cycles run at the start time
/// and in the steps after it. A case that sets none runs until nothing is
/// scheduled.
fn end_time(cycles: Option<usize>) -> EngineTime {
    let Some(cycles) = cycles else {
        return EngineTime::MAX_END;
    };
    let span = EngineDelta::from_micros(i64::try_from(cycles).unwrap_or(i64::MAX));
    EngineTime::MIN_START
        .checked_add(span)
        .unwrap_or(EngineTime::MAX_END)
}

/// A node's id: its position in the finished description (GRF-3), found by the
/// name it was described under and, for a replay, by the input it stands for.
fn position(
    nodes: &[NodeDescription],
    implementation: &str,
    slot: Option<usize>,
) -> Option<NodeId> {
    let wanted = slot.map(count);
    let found = nodes.iter().position(|node| {
        node.implementation == implementation
            && wanted.as_ref().is_none_or(|slot| {
                (node.scalars.iter()).any(|(name, value)| name == SLOT && value == slot)
            })
    })?;
    u32::try_from(found).ok().map(NodeId)
}

/// Cycle by cycle, to the end of the longer of the two: a tick the case does
/// not expect is as much a difference as one it expects and did not get.
fn compare(expected: &[Option<ScalarValue>], seen: &[Option<ScalarValue>]) -> Result<(), Failure> {
    for cycle in 0..expected.len().max(seen.len()) {
        let (expected, actual) = (at(expected, cycle), at(seen, cycle));
        if expected != actual {
            return Err(Failure::Mismatch {
                cycle,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

/// A cycle a sequence does not reach is a cycle with no tick.
fn at(ticks: &[Option<ScalarValue>], cycle: usize) -> Option<ScalarValue> {
    ticks.get(cycle).cloned().flatten()
}

/// A count as the scalar a fixture reads it as.
fn count(value: usize) -> ScalarValue {
    ScalarValue::I64(i64::try_from(value).unwrap_or(i64::MAX))
}

pub mod evaluation;
