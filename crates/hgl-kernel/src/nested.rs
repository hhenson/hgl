//! Keyed ownership of real Graph instances sharing the run's Store.

use crate::deadlines::Deadlines;
use crate::{Ctx, Graph, NodeError, NodeResult};
use hgl_store::{ScopeId, Store};
use hgl_types::EngineTime;
use std::collections::BTreeMap;

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
            ctx.evaluate_child(&mut child.graph)?;
            self.deadlines.set(slot, child.graph.next_scheduled_time());
        }
        self.rearm(ctx);
        Ok(())
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
