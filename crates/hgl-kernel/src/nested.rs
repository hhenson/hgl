//! Keyed ownership of real Graph instances sharing the run's Store.

use crate::deadlines::Deadlines;
use crate::{Ctx, Graph, NodeError, NodeResult};
use hgl_store::{ScopeId, Store};
use hgl_types::EngineTime;
use std::collections::BTreeMap;

/// One fresh child instance constructed before its owner's first hook.
/// Its boundary remains independent of the graph's activation and lifetime.
#[derive(Debug)]
pub struct PreparedChild<T> {
    graph: Graph,
    boundary: T,
}
impl<T> PreparedChild<T> {
    /// Build in a new child scope without starting hooks or running nodes.
    /// Allocate every finite instance before startup, then prepare Store queues.
    pub fn new(
        store: &mut Store,
        owner: hgl_types::NodeId,
        build: impl FnOnce(&mut Store) -> Result<(Graph, T), Box<NodeError>>,
    ) -> Result<Self, Box<NodeError>> {
        let scope = store.child_scope(owner);
        let previous = store.enter_scope(scope);
        let result = build(store);
        store.enter_scope(previous);
        match result {
            Ok((mut graph, boundary)) => {
                graph.prepare_scope(scope, store);
                Ok(Self { graph, boundary })
            }
            Err(error) => {
                store.discard_scope(scope);
                Err(error)
            }
        }
    }
    /// The already prepared boundary; obtaining it does not start the child.
    pub fn boundary(&self) -> &T {
        &self.boundary
    }
    /// The stable scope used to match this child's notification mailbox.
    pub fn scope(&self) -> ScopeId {
        self.graph.scope()
    }
    /// Start this fresh instance once under the exact owner that prepared it.
    pub fn start(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.start_with(ctx, |_| Ok(()))
    }
    /// Bind prepared captures in the child scope immediately before first start.
    /// The callback may sample inputs with a quiet wake; pending child nodes are
    /// admitted by start after the boundary is complete.
    pub fn start_with(
        &mut self,
        ctx: &mut Ctx<'_>,
        bind: impl FnOnce(&mut Store) -> NodeResult,
    ) -> NodeResult {
        self.start_bound(ctx, |store, _| bind(store))
    }
    /// Bind using the retained boundary without cloning it during activation.
    pub fn start_bound(
        &mut self,
        ctx: &mut Ctx<'_>,
        bind: impl FnOnce(&mut Store, &T) -> NodeResult,
    ) -> NodeResult {
        self.check_owner(ctx)?;
        if self.graph.lifecycle() != crate::Lifecycle::Instantiated {
            return Err(NodeError::new("GRF-20: a graph is started once"));
        }
        let previous = ctx.store.enter_scope(self.graph.scope());
        let result = bind(ctx.store, &self.boundary);
        ctx.store.enter_scope(previous);
        let result = result.and_then(|()| self.graph.start(ctx.store, ctx.now));
        if self.graph.stop_requested() {
            ctx.request_stop();
        }
        if result.is_err() {
            ctx.store
                .release_scope(self.graph.scope(), ctx.now, ctx.schedule);
        }
        result
    }
    /// Refresh a live boundary without restarting its child or duplicating unchanged bindings.
    pub fn refresh_bound(
        &mut self,
        ctx: &mut Ctx<'_>,
        bind: impl FnOnce(&mut Store, &T) -> NodeResult,
    ) -> NodeResult {
        self.check_owner(ctx)?;
        if self.graph.lifecycle() != crate::Lifecycle::Started {
            return Err(NodeError::new("prepared child is not started"));
        }
        bind(ctx.store, &self.boundary)
    }
    /// Drive this started child at its owner's current time.
    pub fn evaluate(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        self.check_owner(ctx)?;
        if self.graph.lifecycle() != crate::Lifecycle::Started {
            return Err(NodeError::new("prepared child is not started"));
        }
        ctx.evaluate_child(&mut self.graph)
    }
    /// Consume one evaluation failure as canonical data; lifecycle failures still escape.
    pub fn evaluate_captured(
        &mut self,
        ctx: &mut Ctx<'_>,
    ) -> NodeResult<Option<hgl_types::ErrorValue>> {
        self.check_owner(ctx)?;
        if self.graph.lifecycle() != crate::Lifecycle::Started {
            return Err(NodeError::new("prepared child is not started"));
        }
        match ctx.evaluate_child_resumable(&mut self.graph) {
            Ok(()) => Ok(None),
            Err(error) if error.phase == crate::Phase::Eval => Ok(Some(error.error_value())),
            Err(error) => Err(error),
        }
    }
    /// Discard this active child's abandoned work without stopping or retiring its lifetime.
    /// # Panics
    /// The supplied context must belong to the exact owner that prepared this child.
    pub fn discard_captured_pass(&mut self, ctx: &mut Ctx<'_>) {
        assert_eq!(
            ctx.store.bindings().child_owner(self.graph.scope()),
            Some((ctx.store.scope(), ctx.node)),
            "prepared child belongs to current owner"
        );
        self.graph.discard_captured_pass(ctx.store, ctx.now);
    }
    /// Its next deadline, independent of the owner's selector input.
    pub fn next_scheduled_time(&self) -> EngineTime {
        self.graph.next_scheduled_time()
    }
    /// Stop and retire once; retained designations expire next cycle.
    pub fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        if ctx
            .store
            .bindings()
            .child_owner(self.graph.scope())
            .is_none()
        {
            return Ok(());
        }
        self.check_owner(ctx)?;
        ctx.stop_child(&mut self.graph)
    }
    fn check_owner(&self, ctx: &Ctx<'_>) -> NodeResult {
        if ctx.store.bindings().child_owner(self.graph.scope())
            != Some((ctx.store.scope(), ctx.node))
        {
            return Err(NodeError::new("prepared child belongs to another owner"));
        }
        Ok(())
    }
}

/// Finite fresh branch instances; exhaustion never resumes a stopped graph.
#[derive(Debug)]
pub struct PreparedSwitch<T> {
    children: [Vec<PreparedChild<T>>; 2],
    next: [usize; 2],
    selected: Option<(usize, usize)>,
}
impl<T> PreparedSwitch<T> {
    /// Cold pools contain independent instances owned by this exact node.
    pub fn new(children: [Vec<PreparedChild<T>>; 2]) -> Self {
        Self {
            children,
            next: [0, 0],
            selected: None,
        }
    }
    /// Switch before evaluating, sample fresh captures, and drive pending work.
    pub fn evaluate(
        &mut self,
        enabled: bool,
        ctx: &mut Ctx<'_>,
        bind: impl FnOnce(&mut Store, &T) -> NodeResult,
    ) -> NodeResult<Option<&T>> {
        let branch = usize::from(!enabled);
        let fresh = self.selected.is_none_or(|(selected, _)| selected != branch);
        if fresh {
            if let Some((previous, slot)) = self.selected.take() {
                self.children[previous][slot].stop(ctx)?;
            }
            let slot = self.next[branch];
            let child = self.children[branch]
                .get_mut(slot)
                .ok_or_else(|| NodeError::new("prepared conditional activation bound exhausted"))?;
            child.start_bound(ctx, bind)?;
            self.next[branch] += 1;
            self.selected = Some((branch, slot));
        }
        let (_, slot) = self
            .selected
            .unwrap_or_else(|| unreachable!("selected branch"));
        let child = &mut self.children[branch][slot];
        let mut notified = false;
        while let Some(scope) = ctx.take_child() {
            notified |= scope == child.scope();
        }
        if fresh || notified || child.next_scheduled_time() <= ctx.evaluation_time() {
            let result = child.evaluate(ctx);
            ctx.schedule_children(child.next_scheduled_time());
            result?;
        }
        ctx.schedule_children(child.next_scheduled_time());
        Ok(Some(child.boundary()))
    }
    /// Abandon only the failed pass of the selected live branch and retain its future wakes.
    pub fn discard_captured_pass(&mut self, ctx: &mut Ctx<'_>) {
        if let Some((branch, slot)) = self.selected {
            let child = &mut self.children[branch][slot];
            child.discard_captured_pass(ctx);
            ctx.schedule_children(child.next_scheduled_time());
        } else {
            ctx.schedule_children(EngineTime::FOREVER);
        }
    }
    /// Stop the active instance and discard all unused prepared descendants.
    pub fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let mut result = Ok(());
        for branch in &mut self.children {
            for child in branch {
                let stopped = child.stop(ctx);
                if result.is_ok() {
                    result = stopped;
                }
            }
        }
        ctx.schedule_children(EngineTime::FOREVER);
        result
    }
}

#[derive(Debug)]
struct Child {
    key: i64,
    graph: Graph,
}
/// A node's children, driven only by input notifications and live deadlines.
#[derive(Debug, Default)]
pub struct Children {
    slots: Vec<Option<Child>>,
    free: Vec<usize>,
    keys: BTreeMap<i64, usize>,
    scopes: BTreeMap<ScopeId, usize>,
    order: Vec<usize>,
    deadlines: Deadlines,
}
impl Children {
    /// No child graphs and no pending deadlines.
    pub fn new() -> Self {
        Self::default()
    }
    /// Build and start a fresh child, returning its factory's boundary handle.
    pub fn insert<T>(
        &mut self,
        key: i64,
        ctx: &mut Ctx<'_>,
        build: impl FnOnce(&mut Store) -> Result<(Graph, T), Box<NodeError>>,
    ) -> Result<T, Box<NodeError>> {
        if self.keys.contains_key(&key) {
            return Err(NodeError::new("child key already present"));
        }
        let (graph, value) = ctx.create_child(build)?;
        let slot = self.free.pop().unwrap_or(self.slots.len());
        self.keys.insert(key, slot);
        self.scopes.insert(graph.scope(), slot);
        self.order.push(slot);
        let time = graph.next_scheduled_time();
        if slot == self.slots.len() {
            self.slots.push(Some(Child { key, graph }));
            self.deadlines.reserve(self.slots.len());
        } else {
            self.slots[slot] = Some(Child { key, graph });
        }
        self.deadlines.set(slot, time);
        self.rearm(ctx);
        Ok(value)
    }
    /// Stop and detach a child; its values remain readable this engine cycle.
    pub fn remove(&mut self, key: i64, ctx: &mut Ctx<'_>) -> NodeResult {
        let Some(slot) = self.keys.remove(&key) else {
            return Ok(());
        };
        self.order.retain(|&s| s != slot);
        self.deadlines.remove(slot);
        let mut child = self.slots[slot]
            .take()
            .unwrap_or_else(|| unreachable!("live child slot"));
        self.scopes.remove(&child.graph.scope());
        self.free.push(slot);
        let result = ctx.stop_child(&mut child.graph);
        self.rearm(ctx);
        result
    }
    /// Evaluate each notified or due child once at the parent's current time.
    pub fn evaluate(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let now = ctx.evaluation_time();
        while let Some(scope) = ctx.take_child() {
            if let Some(&slot) = self.scopes.get(&scope) {
                self.deadlines.set(slot, now);
            }
        }
        while let Some((time, slot)) = self.deadlines.first() {
            if time > now {
                break;
            }
            self.deadlines.remove(slot);
            let child = self.slots[slot]
                .as_mut()
                .unwrap_or_else(|| unreachable!("scheduled child"));
            let result = ctx.evaluate_child(&mut child.graph);
            self.deadlines.set(slot, child.graph.next_scheduled_time());
            if result.is_err() {
                self.rearm(ctx);
            }
            result?;
        }
        self.rearm(ctx);
        Ok(())
    }
    /// Discard due-only nested work after an enclosing capture consumes a failure.
    pub fn discard_captured_pass(&mut self, ctx: &mut Ctx<'_>) {
        while ctx.take_child().is_some() {}
        let now = ctx.evaluation_time();
        for slot in 0..self.slots.len() {
            if let Some(child) = &mut self.slots[slot] {
                child.graph.discard_captured_pass(ctx.store, now);
                self.deadlines.set(slot, child.graph.next_scheduled_time());
            }
        }
        self.rearm(ctx);
    }
    /// Stop every successfully started child in reverse creation order.
    pub fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let mut first = Ok(());
        while let Some(&slot) = self.order.last() {
            let key = self.slots[slot]
                .as_ref()
                .unwrap_or_else(|| unreachable!("started child"))
                .key;
            let result = self.remove(key, ctx);
            if first.is_ok() {
                first = result;
            }
        }
        first
    }
    fn rearm(&self, ctx: &mut Ctx<'_>) {
        ctx.schedule_children(
            self.deadlines
                .first()
                .map_or(EngineTime::FOREVER, |(time, _)| time),
        );
    }
}
