//! The simulation engine: the owner of a root graph, and of time
//! (specification: Execution engine).

use hgl_store::Store;
use hgl_types::EngineTime;

use crate::{Graph, NodeError};

/// Set before the run, never changed during it.
#[derive(Debug)]
pub struct RunConfig {
    /// Inclusive: the first cycle may run at the start time.
    pub start_time: EngineTime,
    /// Exclusive: no cycle runs at the end time.
    pub end_time: EngineTime,
}

/// Why a run did not complete.
#[derive(Debug, Clone, PartialEq)]
pub enum EngineError {
    /// The start time is before the earliest start, the end time is after the
    /// latest end, or the end time is not after the start time.
    BadTimes,
    /// A failure left the root graph. The graph was stopped before this was
    /// returned.
    Node(Box<NodeError>),
}

/// Simulation: start, cycles at each next scheduled time before the end time,
/// stop. Returns the number of cycles run.
///
/// The engine never waits (ENG-5) and only it chooses a cycle's time
/// (ENG-11). Whatever ends the run, every node that started is stopped
/// (ENG-10); of several failures the first is returned.
pub fn run_simulation(
    graph: &mut Graph,
    store: &mut Store,
    config: &RunConfig,
) -> Result<u64, EngineError> {
    let (start, end) = (config.start_time, config.end_time);
    if start < EngineTime::MIN_START || end > EngineTime::MAX_END || start >= end {
        return Err(EngineError::BadTimes);
    }
    graph.start(store, start).map_err(EngineError::Node)?;

    let mut now = start;
    let mut cycles = 0;
    let mut outcome = Ok(());
    while outcome.is_ok() && !graph.stop_requested() {
        let next = graph.next_scheduled_time();
        if next >= end {
            break;
        }
        now = next;
        outcome = graph.evaluate(store, now);
        cycles += 1;
    }
    let stopped = graph.stop(store, now);
    if outcome.is_ok() {
        outcome = stopped;
    }
    match outcome {
        Ok(()) => Ok(cycles),
        Err(error) => Err(EngineError::Node(error)),
    }
}
