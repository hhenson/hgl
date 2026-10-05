//! What a node can reach while one of its hooks runs.

use hgl_store::{
    DictOut, GlobalState, In, InputId, Out, OutputId, Reference, Scalar, ScopeId, Store,
};
use hgl_types::{EngineDelta, EngineTime, NodeId, NodeType};

use crate::schedule::Schedule;
use crate::{Graph, NodeError, NodeResult, Phase};

/// Everything a node may touch while one of its hooks runs. Borrowed for the
/// call; a node cannot keep it (INJ-3).
///
/// It is made for one node and one hook. The node it was made for is who
/// writes, and the graph's schedule is who is woken; a node author sees
/// neither. Nothing here can move the clock (INJ-6).
///
/// INJ-3 is proved by the signature, not by a run: a hook is lent `ctx` for
/// the call, so a node that tries to keep it does not compile.
///
/// ```compile_fail
/// use hgl_kernel::{Ctx, Node, NodeResult};
///
/// struct Keeps {
///     kept: Option<&'static mut Ctx<'static>>,
/// }
///
/// impl Node for Keeps {
///     fn eval(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
///         // Refused: the borrow ends with the call.
///         self.kept = Some(ctx);
///         Ok(())
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Ctx<'a> {
    pub(crate) store: &'a mut Store,
    pub(crate) schedule: &'a mut Schedule,
    pub(crate) stop_requested: &'a mut bool,
    pub(crate) node_type: &'a NodeType,
    pub(crate) node: NodeId,
    pub(crate) now: EngineTime,
    pub(crate) phase: Phase,
    pub(crate) scheduled_now: bool,
}

impl Ctx<'_> {
    /// Typed prepared publication and disjoint source/record destination access.
    pub fn prepared(&mut self) -> hgl_store::PreparedTick<'_, impl hgl_store::Wake> {
        debug_assert!(self.phase == Phase::Eval, "NOD-22: writes only in eval");
        self.store
            .prepared()
            .tick(self.now, self.node, self.schedule)
    }
    /// Prepared ordinary capability operations, without hook-time key or type binding.
    pub fn global_state(&mut self) -> &mut GlobalState {
        self.store.global_state()
    }
    /// The input's value. The input must be valid: one the node requires is;
    /// any other is asked first.
    #[inline]
    pub fn get<T: Scalar>(&self, input: In<T>) -> T {
        self.store.get(input)
    }

    /// Whether the input has a value.
    #[inline]
    pub fn valid<T: Scalar>(&self, input: In<T>) -> bool {
        self.store.valid(input)
    }

    /// Whether the input ticked in this cycle.
    #[inline]
    pub fn modified<T: Scalar>(&self, input: In<T>) -> bool {
        self.store.modified(input, self.now)
    }

    /// When the input last ticked; `NEVER` if it has not.
    #[inline]
    pub fn last_modified<T: Scalar>(&self, input: In<T>) -> EngineTime {
        self.store.last_modified(input)
    }

    /// Choose whether the input wakes this node from now on.
    #[inline]
    pub fn set_active<T: Scalar>(&mut self, input: In<T>, active: bool) {
        self.store.set_active(input, active);
    }

    /// Tick the node's own output. In eval only (NOD-22, INJ-8).
    #[inline]
    pub fn set<T: Scalar>(&mut self, output: Out<T>, value: T) {
        debug_assert!(
            self.phase == Phase::Eval,
            "NOD-22, INJ-8: an output is written in eval only"
        );
        self.store
            .set(output, value, self.now, self.node, self.schedule);
    }

    /// Publish a prepared complete ordinary value from an atomic endpoint.
    pub fn set_atomic<T: hgl_store::GlobalValue>(
        &mut self,
        output: hgl_store::shapes::Output<hgl_store::shapes::Atomic<T>>,
        value: T::Value,
    ) -> NodeResult {
        self.writes(output.id());
        self.store
            .set_atomic(output, value, self.now, self.schedule)
    }

    /// What the node's own output holds: `None` until it has ticked.
    #[inline]
    pub fn output_value<T: Scalar>(&self, output: Out<T>) -> Option<T> {
        self.store.output_value(output)
    }

    /// The time of this cycle; in start, the start time.
    #[inline]
    pub fn evaluation_time(&self) -> EngineTime {
        self.now
    }

    /// The earliest time a following cycle could have.
    #[inline]
    pub fn next_cycle_evaluation_time(&self) -> EngineTime {
        self.now
            .checked_add(EngineDelta::STEP)
            .unwrap_or(EngineTime::FOREVER)
    }

    /// Ask to be evaluated `delay` after the evaluation time. The node has
    /// one pending request, which this replaces.
    ///
    /// Errors: the node type does not use a scheduler (INJ-2); the time is
    /// not in the future, or in start is before the start time (GRF-12,
    /// NOD-14); the time is after `FOREVER`.
    #[inline]
    pub fn schedule_in(&mut self, delay: EngineDelta) -> NodeResult {
        let time = self.schedule_time(delay)?;
        self.schedule.set_request(self.node, time, false);
        Ok(())
    }

    /// Arm a source alarm, retaining the earliest pending request (ADR 0015).
    /// Requires scheduler capability and no temporal inputs. This carries no
    /// recoverable scheduler state; a new run rearms alarms in `start`.
    pub fn alarm_in(&mut self, delay: EngineDelta) -> NodeResult {
        if !self.node_type.inputs.is_empty() {
            return Err(NodeError::new("alarm is admitted only on sources"));
        }
        let time = self.schedule_time(delay)?;
        self.schedule.set_request(self.node, time, true);
        Ok(())
    }

    fn schedule_time(&self, delay: EngineDelta) -> Result<EngineTime, Box<NodeError>> {
        if !self.node_type.uses_scheduler {
            return Err(NodeError::new(
                "INJ-2: the node type does not use a scheduler",
            ));
        }
        let soonest = match self.phase {
            Phase::Start => EngineDelta::from_micros(0),
            Phase::Eval | Phase::Stop => EngineDelta::STEP,
        };
        if delay < soonest {
            return Err(NodeError::new(
                "GRF-12, NOD-14: the time asked for is not in the future",
            ));
        }
        self.now
            .checked_add(delay)
            .ok_or_else(|| NodeError::new("ENG-16: the time asked for is after forever"))
    }

    /// Whether the node's own request is why it is being evaluated now.
    #[inline]
    pub fn is_scheduled_now(&self) -> bool {
        self.scheduled_now
    }

    /// End the run once this cycle is complete (INJ-10).
    #[inline]
    pub fn request_stop(&mut self) {
        *self.stop_requested = true;
    }
    fn writes(&self, output: OutputId) {
        let endpoint = self.store.bindings().output(output);
        debug_assert!(self.phase == Phase::Eval, "NOD-22: writes only in eval");
        debug_assert_eq!(endpoint.owner, self.node, "TS-21: not the owner");
        debug_assert_eq!(endpoint.scope, self.store.scope(), "TS-21: foreign graph");
    }
    /// Logical data observations, without mutable store access.
    pub fn store(&self) -> &Store {
        self.store
    }
    /// Sample a designation at this cycle's time.
    pub fn sample(&mut self, input: InputId, r: Reference) -> NodeResult {
        self.store
            .sample(input, r, self.now, self.schedule)
            .map_err(|e| NodeError::new(format!("{e:?}")))
    }
    /// Publish an independently tracked reference designation.
    pub fn set_reference(&mut self, output: OutputId, r: Reference) -> NodeResult {
        self.writes(output);
        self.store
            .set_reference(output, r, self.now, self.schedule)
            .map_err(|e| NodeError::new(format!("{e:?}")))
    }
    /// Invalidate an owned output and notify its collection.
    pub fn invalidate(&mut self, output: OutputId) {
        self.writes(output);
        self.store.invalidate(output, self.now, self.schedule);
    }
    /// Create or restore a dictionary member.
    pub fn get_or_create<T: Scalar>(&mut self, dict: DictOut<T>, key: i64) -> Out<T> {
        self.writes(dict.id());
        self.store.get_or_create(dict, key, self.now, self.schedule)
    }
    /// Attach a child graph's output to this node's dictionary.
    pub fn attach<T: Scalar>(&mut self, dict: DictOut<T>, key: i64, child: Out<T>) -> NodeResult {
        self.writes(dict.id());
        self.store
            .attach(dict, key, child, self.now, self.schedule)
            .map_err(|e| NodeError::new(format!("{e:?}")))
    }
    /// Remove membership now, preserving the removed child this cycle.
    pub fn remove<T: Scalar>(&mut self, dict: DictOut<T>, key: i64) {
        self.remove_shaped(dict.id(), key);
    }
    /// Create or restore a compound dictionary member in this node's scope.
    pub fn get_or_create_shaped(&mut self, dict: OutputId, key: i64) -> OutputId {
        self.writes(dict);
        self.store
            .get_or_create_shaped(dict, key, self.now, self.schedule)
    }
    /// Create a missing member using its statically prepared allocation factory.
    pub fn get_or_create_with(
        &mut self,
        dict: OutputId,
        key: i64,
        create: impl FnOnce(&mut Store, NodeId) -> OutputId,
    ) -> OutputId {
        self.writes(dict);
        self.store
            .get_or_create_with(dict, key, self.now, self.schedule, create)
    }
    /// Attach a compound child graph output without copying it.
    pub fn attach_shaped(&mut self, dict: OutputId, key: i64, child: Reference) -> NodeResult {
        self.writes(dict);
        self.store
            .attach_shaped(dict, key, child, self.now, self.schedule)
            .map_err(|e| NodeError::new(format!("{e:?}")))
    }
    /// Remove a compound member, retaining its subtree for this cycle.
    pub fn remove_shaped(&mut self, dict: OutputId, key: i64) {
        self.writes(dict);
        self.store.remove_shaped(dict, key, self.now, self.schedule);
    }
    /// Build and start a fresh scoped graph; roll back failed construction.
    pub fn create_child<T>(
        &mut self,
        build: impl FnOnce(&mut Store) -> Result<(Graph, T), Box<NodeError>>,
    ) -> Result<(Graph, T), Box<NodeError>> {
        let scope = self.store.child_scope(self.node);
        let previous = self.store.enter_scope(scope);
        let construction = build(self.store);
        self.store.enter_scope(previous);
        let result = construction.and_then(|(mut graph, value)| {
            graph.scope = scope;
            graph.start(self.store, self.now)?;
            if graph.stop_requested() {
                self.request_stop();
            }
            Ok((graph, value))
        });
        if result.is_err() {
            self.store.release_scope(scope, self.now, self.schedule);
        }
        result
    }
    /// Evaluate a child at the parent's current time.
    pub fn evaluate_child(&mut self, graph: &mut Graph) -> NodeResult {
        let result = graph.evaluate(self.store, self.now);
        if graph.stop_requested() {
            self.request_stop();
        }
        result
    }
    /// Stop once, detach ports and cancel the child's remaining schedule.
    pub fn stop_child(&mut self, graph: &mut Graph) -> NodeResult {
        if graph.lifecycle() == crate::Lifecycle::Stopped {
            return Ok(());
        }
        let result = graph.stop(self.store, self.now);
        self.store
            .release_scope(graph.scope, self.now, self.schedule);
        result
    }
    /// A directly owned child with a pending input notification.
    pub fn take_child(&mut self) -> Option<ScopeId> {
        self.store.take_child(self.node)
    }
    /// Replace the internal wake-up for owned children, independently of timers.
    pub fn schedule_children(&mut self, time: EngineTime) {
        self.schedule.set_child_request(self.node, time);
    }
}
