//! A graph instance: nodes in rank order, their schedule, and the lifecycle
//! its owner drives (specification: Graph, Part 2).

use std::any::Any;
use std::fmt;

use hgl_store::{InputId, ScopeId, Store, Wake};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

use crate::schedule::Schedule;
use crate::{Ctx, Node, NodeError, NodeResult, Phase};

/// One node of a graph instance, ready to run. Built by hgl-describe.
pub struct NodeSlot {
    /// The node's behaviour and its handles.
    pub node: Box<dyn Node>,
    /// What the node type asks of the runtime: a scheduler, schedule on start.
    pub node_type: NodeType,
    /// The node's name in error reports.
    pub label: String,
    /// Inputs that must be valid before `eval` is called, already resolved
    /// from `node_type.valid_inputs`.
    pub required: Vec<InputId>,
}

impl fmt::Debug for NodeSlot {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NodeSlot")
            .field("node_type", &self.node_type)
            .field("label", &self.label)
            .field("required", &self.required)
            .finish_non_exhaustive()
    }
}

/// Where a graph is in its one life (specification: Graph, Part 2, "State").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    /// Complete; nothing has started and nothing has ticked.
    Instantiated,
    /// Its nodes are being started.
    Starting,
    /// Between cycles.
    Started,
    /// In a cycle, or left there by a cycle that failed.
    Evaluating,
    /// Its nodes are being stopped.
    Stopping,
    /// Never started again (GRF-20).
    Stopped,
}

/// Nodes in rank order, their schedule and their lifecycle. It does nothing
/// until its owner asks.
#[derive(Debug)]
pub struct Graph {
    pub(crate) scope: ScopeId,
    label: String,
    slots: Vec<NodeSlot>,
    schedule: Schedule,
    lifecycle: Lifecycle,
    /// The time of the cycle in progress or the last one completed. Once
    /// started and before any cycle it is a step before the start time, as
    /// hgraph's executor keeps it: the first cycle may be at the start time,
    /// and none before it (ENG-2, ENG-3).
    last_cycle: EngineTime,
    stop_requested: bool,
    /// Nodes `0..started` have started and have not been stopped.
    started: u32,
}

/// A node's rank from its position in the slots.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a rank is a u32 by design (NodeId); debug builds assert the count fits"
)]
fn rank_count(slots: usize) -> u32 {
    debug_assert!(u32::try_from(slots).is_ok(), "more than u32::MAX nodes");
    slots as u32
}

/// NOD-2: every input the node requires is valid. Checked afresh each time.
#[inline]
fn admitted(store: &Store, required: &[InputId]) -> bool {
    required.iter().all(|&input| store.input_valid(input))
}

impl Graph {
    /// A graph of `slots`, whose order is the rank order (GRF-3): slot `i`
    /// holds the node the store knows as `NodeId(i)`.
    pub fn new(label: String, slots: Vec<NodeSlot>) -> Self {
        Self {
            scope: ScopeId::default(),
            label,
            schedule: Schedule::new(slots.len()),
            slots,
            lifecycle: Lifecycle::Instantiated,
            last_cycle: EngineTime::NEVER,
            stop_requested: false,
            started: 0,
        }
    }

    /// This graph instance's scope, independent of its local ranks.
    pub fn scope(&self) -> ScopeId {
        self.scope
    }

    /// Where the graph is in its life.
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }

    /// Start every node, in rank order, at the start time `now`. If one
    /// fails, those before it are stopped in reverse, it is not, its failure
    /// is returned (GRF-18, NOD-11), and nothing is left scheduled.
    ///
    /// Errors: a node's start failed; the graph is not newly instantiated
    /// (GRF-20).
    pub fn start(&mut self, store: &mut Store, now: EngineTime) -> NodeResult {
        let previous = store.enter_scope(self.scope);
        store.reserve_scope(self.scope, self.slots.len());

        let result = self.start_scoped(store, now);
        store.enter_scope(previous);
        result
    }
    fn start_scoped(&mut self, store: &mut Store, now: EngineTime) -> NodeResult {
        if self.lifecycle != Lifecycle::Instantiated {
            let mut error = NodeError::new("GRF-20: a graph is started once");
            error.label.clone_from(&self.label);
            error.phase = Phase::Start;
            return Err(error);
        }
        if self.scope == ScopeId::default() {
            store.start_run();
        }
        self.lifecycle = Lifecycle::Starting;
        self.last_cycle = EngineTime::from_micros(now.micros() - EngineDelta::STEP.micros());
        for rank in 0..rank_count(self.slots.len()) {
            let node = NodeId(rank);
            if let Err(error) = self.call(store, node, Phase::Start, now, false) {
                // The start failure is the one reported, whatever the
                // rollback's stops return.
                let _rollback = self.stop(store, now);
                self.schedule.clear();
                return Err(error);
            }
            self.started = rank + 1;
            self.schedule.rearm(node);
            // After the re-arm, so that a request made in start cannot move
            // the node off the start time.
            if self.slots[rank as usize].node_type.schedule_on_start {
                self.schedule.schedule_now(node, now);
            }
        }
        while let Some(node) = store.take_wake(self.scope) {
            self.schedule.schedule_now(node, now);
        }
        while let Some(node) = self.schedule.take_ready() {
            self.schedule.schedule_now(node, now);
        }
        self.lifecycle = Lifecycle::Started;
        Ok(())
    }

    /// One cycle. `now` must be later than the last cycle's time, and not
    /// before the start time (ENG-2), and not later than the next scheduled
    /// time (ENG-4).
    ///
    /// Errors: a node's eval failed. The pass ends there, the nodes it had
    /// still to reach lose the cycle, and the graph is left for its owner to
    /// stop.
    pub fn evaluate(&mut self, store: &mut Store, now: EngineTime) -> NodeResult {
        let previous = store.enter_scope(self.scope);
        store.reserve_scope(self.scope, self.slots.len());
        store.begin_cycle(now);
        let result = self.evaluate_scoped(store, now);
        store.enter_scope(previous);
        result
    }
    fn evaluate_scoped(&mut self, store: &mut Store, now: EngineTime) -> NodeResult {
        debug_assert!(
            self.lifecycle == Lifecycle::Started,
            "NOD-1: eval is called only while started"
        );
        debug_assert!(
            now > self.last_cycle,
            "ENG-2: each cycle is later than the one before, and none before the start"
        );
        self.lifecycle = Lifecycle::Evaluating;
        self.last_cycle = now;
        self.schedule.begin_pass(now);
        // `None` is less than every `Some`, so the first node taken passes.
        let mut passed = None;
        loop {
            while let Some(node) = store.take_wake(self.scope) {
                self.schedule.wake(node);
            }
            let Some(node) = self.schedule.take_ready() else {
                break;
            };
            debug_assert!(
                passed < Some(node),
                "GRF-13: a node was woken after the scan had passed it"
            );
            passed = Some(node);
            let scheduled_now = self.schedule.take_due_request(node, now);
            let evaluated = if admitted(store, &self.slots[node.0 as usize].required) {
                self.call(store, node, Phase::Eval, now, scheduled_now)
            } else {
                Ok(())
            };
            self.schedule.rearm(node);
            if evaluated.is_err() {
                self.schedule.abandon_pass(now);
                return evaluated;
            }
        }
        self.lifecycle = Lifecycle::Started;
        Ok(())
    }

    /// `EngineTime::FOREVER` when nothing is scheduled.
    pub fn next_scheduled_time(&self) -> EngineTime {
        self.schedule.next_time()
    }

    /// Stops every started node, in reverse; returns the first failure.
    /// Each is stopped once however often this is called. A graph that never
    /// started has nothing to stop, and is left as it was, still to start.
    pub fn stop(&mut self, store: &mut Store, now: EngineTime) -> NodeResult {
        let previous = store.enter_scope(self.scope);
        store.reserve_scope(self.scope, self.slots.len());

        let result = self.stop_scoped(store, now);
        store.enter_scope(previous);
        result
    }
    fn stop_scoped(&mut self, store: &mut Store, now: EngineTime) -> NodeResult {
        if self.lifecycle == Lifecycle::Instantiated {
            return Ok(());
        }
        self.lifecycle = Lifecycle::Stopping;
        let mut first_failure = Ok(());
        while self.started > 0 {
            self.started -= 1;
            let stopped = self.call(store, NodeId(self.started), Phase::Stop, now, false);
            if first_failure.is_ok() {
                first_failure = stopped;
            }
        }
        self.schedule.clear();
        self.lifecycle = Lifecycle::Stopped;
        first_failure
    }

    /// Whether a node has asked for the run to end. Never cleared.
    pub fn stop_requested(&self) -> bool {
        self.stop_requested
    }

    /// The node at `id`, if it is an `N`. C++: `dynamic_cast<N*>`. For tests
    /// and tools, outside a cycle; nothing on the tick path calls it.
    pub fn node<N: Node>(&self, id: NodeId) -> Option<&N> {
        let node: &dyn Any = &*self.slots.get(id.0 as usize)?.node;
        node.downcast_ref()
    }

    /// [`Self::node`], to change the node: a harness loads a fixture into it
    /// before `start`.
    pub fn node_mut<N: Node>(&mut self, id: NodeId) -> Option<&mut N> {
        let node: &mut dyn Any = &mut *self.slots.get_mut(id.0 as usize)?.node;
        node.downcast_mut()
    }

    /// Run one hook of one node, and name the node in any failure.
    #[inline]
    fn call(
        &mut self,
        store: &mut Store,
        node: NodeId,
        phase: Phase,
        now: EngineTime,
        scheduled_now: bool,
    ) -> NodeResult {
        let slot = &mut self.slots[node.0 as usize];
        let mut ctx = Ctx {
            store,
            schedule: &mut self.schedule,
            stop_requested: &mut self.stop_requested,
            node_type: &slot.node_type,
            node,
            now,
            phase,
            scheduled_now,
        };
        let outcome = match phase {
            Phase::Start => slot.node.start(&mut ctx),
            Phase::Eval => slot.node.eval(&mut ctx),
            Phase::Stop => slot.node.stop(&mut ctx),
        };
        outcome.map_err(|mut error| {
            if error.label.is_empty() {
                error.node = node;
                error.label.clone_from(&slot.label);
                error.phase = phase;
            } else {
                error.label = format!("{}/{}", slot.label, error.label);
            }
            error
        })
    }
}
