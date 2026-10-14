//! Finite keyed templates retain fresh instances across membership lifetimes.
use crate::deadlines::Deadlines;
use crate::nested::PreparedChild;
use crate::{Ctx, NodeError, NodeResult};
use hgl_store::{ScopeId, Store};
use hgl_types::EngineTime;
use std::collections::BTreeMap;

#[derive(Debug)]
struct Slot<T> {
    children: Vec<PreparedChild<T>>,
    next: usize,
    active: Option<usize>,
    previous: Option<usize>,
    next_active: Option<usize>,
}

/// Cold finite pools indexed by exact collection membership tokens.
#[derive(Debug)]
pub struct PreparedKeyed<T> {
    slots: Vec<Slot<T>>,
    keys: BTreeMap<i64, usize>,
    scopes: BTreeMap<ScopeId, usize>,
    head: Option<usize>,
    tail: Option<usize>,
    deadlines: Deadlines,
}

impl<T> PreparedKeyed<T> {
    /// Prepare independently owned instances without running lifecycle hooks.
    /// # Panics
    /// The checked domain must not repeat a membership token.
    pub fn new(pools: Vec<(i64, Vec<PreparedChild<T>>)>) -> Self {
        let mut keys = BTreeMap::new();
        let mut scopes = BTreeMap::new();
        let mut slots = Vec::with_capacity(pools.len());
        for (key, children) in pools {
            let index = slots.len();
            assert!(
                keys.insert(key, index).is_none(),
                "unique prepared keyed domain"
            );
            for child in &children {
                scopes.insert(child.scope(), index);
            }
            slots.push(Slot {
                children,
                next: 0,
                active: None,
                previous: None,
                next_active: None,
            });
        }
        let mut deadlines = Deadlines::default();
        deadlines.reserve(slots.len());
        Self {
            head: None,
            tail: None,
            slots,
            keys,
            scopes,
            deadlines,
        }
    }

    /// Retain an existing child's state or start one fresh membership lifetime.
    pub fn insert(
        &mut self,
        key: i64,
        ctx: &mut Ctx<'_>,
        bind: impl FnOnce(&mut Store, &T) -> NodeResult,
    ) -> NodeResult {
        let index = *self
            .keys
            .get(&key)
            .ok_or_else(|| NodeError::new("keyed graph key was not prepared before start"))?;
        let slot = &mut self.slots[index];
        if slot.active.is_some() {
            return Ok(());
        }
        let next = slot.next;
        let child = slot
            .children
            .get_mut(next)
            .ok_or_else(|| NodeError::new("prepared keyed activation bound exhausted"))?;
        child.start_bound(ctx, bind)?;
        slot.next += 1;
        slot.active = Some(next);
        slot.previous = self.tail;
        slot.next_active = None;
        if let Some(previous) = self.tail {
            self.slots[previous].next_active = Some(index);
        } else {
            self.head = Some(index);
        }
        self.tail = Some(index);
        self.deadlines.set(index, ctx.evaluation_time());
        self.rearm(ctx);
        Ok(())
    }

    /// Stop the removed child; its endpoints retire through the normal scope protocol.
    pub fn remove(&mut self, key: i64, ctx: &mut Ctx<'_>) -> NodeResult {
        let Some(&index) = self.keys.get(&key) else {
            return Ok(());
        };
        self.remove_slot(index, ctx)
    }

    /// Update a live member's changing boundary without restarting its graph.
    pub fn refresh(
        &mut self,
        key: i64,
        ctx: &mut Ctx<'_>,
        bind: impl FnOnce(&mut Store, &T) -> NodeResult,
    ) -> NodeResult {
        let Some(&index) = self.keys.get(&key) else {
            return Ok(());
        };
        let slot = &self.slots[index];
        let Some(active) = slot.active else {
            return Ok(());
        };
        let child = &slot.children[active];
        if ctx.store().bindings().child_owner(child.scope())
            != Some((ctx.store().scope(), ctx.node))
        {
            return Err(NodeError::new(
                "prepared keyed child belongs to another owner",
            ));
        }
        bind(ctx.store, child.boundary())?;
        self.deadlines.set(index, ctx.evaluation_time());
        self.rearm(ctx);
        Ok(())
    }

    fn remove_slot(&mut self, index: usize, ctx: &mut Ctx<'_>) -> NodeResult {
        let slot = &mut self.slots[index];
        let Some(active) = slot.active.take() else {
            return Ok(());
        };
        let (previous, next) = (slot.previous.take(), slot.next_active.take());
        if let Some(previous) = previous {
            self.slots[previous].next_active = next;
        } else {
            self.head = next;
        }
        if let Some(next) = next {
            self.slots[next].previous = previous;
        } else {
            self.tail = previous;
        }
        self.deadlines.remove(index);
        let result = self.slots[index].children[active].stop(ctx);
        self.rearm(ctx);
        result
    }

    /// Drive notified and due children; an uncaught failure escapes normally.
    pub fn evaluate(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let now = ctx.evaluation_time();
        while let Some(scope) = ctx.take_child() {
            if let Some(&index) = self.scopes.get(&scope)
                && let Some(active) = self.slots[index].active
                && self.slots[index].children[active].scope() == scope
            {
                self.deadlines.set(index, now);
            }
        }
        while let Some((time, index)) = self.deadlines.first() {
            if time > now {
                break;
            }
            self.deadlines.remove(index);
            let slot = &mut self.slots[index];
            let active = slot
                .active
                .unwrap_or_else(|| unreachable!("scheduled live child"));
            let child = &mut slot.children[active];
            let result = child.evaluate(ctx);
            self.deadlines.set(index, child.next_scheduled_time());
            if result.is_err() {
                self.rearm(ctx);
                return result;
            }
        }
        self.rearm(ctx);
        Ok(())
    }

    /// An enclosing explicit capture abandons due work, preserving future wakes.
    /// This scan runs only after a captured failure, never on successful ticks.
    pub fn discard_captured_pass(&mut self, ctx: &mut Ctx<'_>) {
        let now = ctx.evaluation_time();
        self.deadlines.clear();
        let mut index = self.head;
        while let Some(current) = index {
            index = self.slots[current].next_active;
            let slot = &mut self.slots[current];
            let active = slot
                .active
                .unwrap_or_else(|| unreachable!("live keyed child"));
            let child = &mut slot.children[active];
            child.discard_captured_pass(ctx);
            let next = child.next_scheduled_time();
            if next > now {
                self.deadlines.set(current, next);
            }
        }
        self.rearm(ctx);
    }

    /// Stop live children in reverse creation order, then discard unused scopes.
    pub fn stop(&mut self, ctx: &mut Ctx<'_>) -> NodeResult {
        let mut result = Ok(());
        while let Some(index) = self.tail {
            let stopped = self.remove_slot(index, ctx);
            if result.is_ok() {
                result = stopped;
            }
        }
        for slot in &mut self.slots {
            for child in slot.children.iter_mut().skip(slot.next) {
                let stopped = child.stop(ctx);
                if result.is_ok() {
                    result = stopped;
                }
            }
        }
        ctx.schedule_children(EngineTime::FOREVER);
        result
    }

    fn rearm(&self, ctx: &mut Ctx<'_>) {
        ctx.schedule_children(
            self.deadlines
                .first()
                .map_or(EngineTime::FOREVER, |(time, _)| time),
        );
    }
}
