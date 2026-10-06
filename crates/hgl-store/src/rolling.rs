//! Independent prepared ordinary arrivals retained in statically selected windows.
use crate::bindings::{Bindings, OutputId, Wake};
use crate::global_value::{GlobalValue, ValueColumns, ValueSlot};
use crate::prepared_value::PreparedValue;
use crate::shapes::{Input, Output, Shape};
use hgl_types::{
    EngineTime, NodeError, NodeId, NodeResult, OrdinaryType, TsType, Window, WindowKind,
};
use std::marker::PhantomData;
/// Static window policy; no runtime schema tests select arrival behavior.
pub trait WindowShape: Shape {
    /// Complete ordinary arrival payload.
    type Payload: GlobalValue;
    /// True for an inclusive duration window, false for a tick count.
    const DURATION: bool;
    /// Maximum count or age in microseconds.
    const MAX: i64;
    /// Required count or retained span in microseconds.
    const MIN: i64;
}
/// Exact prepared rolling endpoint marker.
#[derive(Debug)]
pub struct Rolling<T, const DURATION: bool, const MAX: i64, const MIN: i64>(PhantomData<T>);
impl<T: GlobalValue, const D: bool, const MAX: i64, const MIN: i64> Shape
    for Rolling<T, D, MAX, MIN>
{
    fn shape() -> TsType {
        let kind = if D {
            WindowKind::Duration
        } else {
            WindowKind::Ticks
        };
        TsType::Rolling(
            T::schema(),
            Window::new(kind, MAX, MIN)
                .unwrap_or_else(|_| unreachable!("invalid rolling marker bounds")),
        )
    }
}
impl<T: GlobalValue, const D: bool, const MAX: i64, const MIN: i64> WindowShape
    for Rolling<T, D, MAX, MIN>
{
    type Payload = T;
    const DURATION: bool = D;
    const MAX: i64 = MAX;
    const MIN: i64 = MIN;
}
#[derive(Debug, Default)]
struct Root {
    slots: Vec<Vec<usize>>,
    times: Vec<EngineTime>,
    head: usize,
    count: usize,
}
/// Run-owned rings, allocated only from finite cold bounds before publication.
#[derive(Debug, Default)]
pub struct Arena {
    values: ValueColumns,
    roots: Vec<Root>,
}
impl Arena {
    /// Create an invalid endpoint without constructing any arrival.
    /// # Panics
    /// Exhausting the u32 endpoint address space is a cold construction error.
    pub fn add_output(
        &mut self,
        bindings: &mut Bindings,
        owner: NodeId,
        ty: OrdinaryType,
        window: Window,
    ) -> OutputId {
        let slot = u32::try_from(self.roots.len())
            .unwrap_or_else(|_| unreachable!("rolling root capacity"));
        let (output, fresh) = bindings.add_output(owner, TsType::Rolling(ty, window), slot);
        if fresh {
            self.roots.push(Root::default());
        }
        output
    }
    /// Reserve independent value capacity for each possible retained arrival.
    pub fn prepare_output<S: WindowShape>(
        &mut self,
        bindings: &Bindings,
        output: OutputId,
        bounds: &<S::Payload as PreparedValue>::Bounds,
        horizon: usize,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        let capacity = if S::DURATION {
            horizon
        } else {
            horizon.min(usize::try_from(S::MAX).map_err(|e| NodeError::new(e.to_string()))?)
        };
        let root = &mut self.roots[bindings.output(output).slot as usize];
        for layout in &root.slots {
            ValueSlot::<S::Payload>::bind(&mut layout.as_slice()).release(&mut self.values);
        }
        root.slots.clear();
        root.times.clear();
        root.head = 0;
        root.count = 0;
        root.slots
            .try_reserve(capacity)
            .map_err(|e| NodeError::new(e.to_string()))?;
        root.times
            .try_reserve(capacity)
            .map_err(|e| NodeError::new(e.to_string()))?;
        for _ in 0..capacity {
            let slot = S::Payload::allocate(&mut self.values, bounds)?;
            let mut layout = vec![0; S::Payload::WIDTH];
            slot.flatten(&mut layout);
            root.slots.push(layout);
            root.times.push(EngineTime::NEVER);
        }
        Ok(())
    }
    /// Borrow the latest arrival, never the retained window as a synthetic delta.
    pub fn borrow<S: WindowShape>(
        &self,
        bindings: &Bindings,
        input: Input<S>,
    ) -> NodeResult<ValueSlot<S::Payload>> {
        if !bindings.valid(input.id()) {
            return Err(NodeError::new("rolling input is invalid"));
        }
        let root = &self.roots[bindings.input(input.id()).slot as usize];
        if root.count == 0 {
            return Err(NodeError::new("rolling input has no arrival"));
        }
        let index = (root.head + root.count - 1) % root.slots.len();
        Ok(ValueSlot::bind(&mut root.slots[index].as_slice()))
    }
    /// Readiness is derived from the currently retained count or span, without eviction.
    pub fn ready<S: WindowShape>(&self, bindings: &Bindings, input: Input<S>) -> bool {
        if !bindings.valid(input.id()) {
            return false;
        }
        let root = &self.roots[bindings.input(input.id()).slot as usize];
        if root.count == 0 {
            return false;
        }
        if S::DURATION {
            let newest = (root.head + root.count - 1) % root.times.len();
            i128::from(root.times[newest].micros()) - i128::from(root.times[root.head].micros())
                >= i128::from(S::MIN)
        } else {
            i64::try_from(root.count).is_ok_and(|count| count >= S::MIN)
        }
    }
    /// Borrow ordinary source columns for capture into an independent destination.
    pub fn values(&self) -> &ValueColumns {
        &self.values
    }
    fn destination<S: WindowShape>(
        &self,
        bindings: &Bindings,
        output: Output<S>,
        now: EngineTime,
    ) -> NodeResult<(ValueSlot<S::Payload>, usize, usize)> {
        let endpoint = bindings.output(output.id());
        assert!(
            endpoint.alive && endpoint.generation == output.generation(),
            "expired rolling output"
        );
        let root = &self.roots[endpoint.slot as usize];
        let (mut head, mut count) = if endpoint.modified_at == EngineTime::NEVER {
            (0, 0)
        } else {
            (root.head, root.count)
        };
        if root.slots.is_empty() {
            return Err(NodeError::new(
                "rolling output lacks finite prepared capacity",
            ));
        }
        if S::DURATION {
            while count > 0
                && i128::from(now.micros()) - i128::from(root.times[head].micros())
                    > i128::from(S::MAX)
            {
                head = (head + 1) % root.slots.len();
                count -= 1;
            }
        } else if i64::try_from(count) == Ok(S::MAX) {
            head = (head + 1) % root.slots.len();
            count -= 1;
        }
        if count == root.slots.len() {
            return Err(NodeError::new("rolling finite arrival capacity exceeded"));
        }
        let index = (head + count) % root.slots.len();
        Ok((
            ValueSlot::bind(&mut root.slots[index].as_slice()),
            head,
            count,
        ))
    }
    fn commit<S: WindowShape, W: Wake>(
        &mut self,
        bindings: &mut Bindings,
        output: Output<S>,
        now: EngineTime,
        wake: &mut W,
        (head, count): (usize, usize),
    ) {
        let root = &mut self.roots[bindings.output(output.id()).slot as usize];
        root.times[(head + count) % root.slots.len()] = now;
        root.head = head;
        root.count = count + 1;
        bindings.publish(output.id(), now, wake);
    }
    /// Compose premeasured text, then commit the independently retained arrival.
    /// The callback reads source state and appends exactly the measured bytes.
    pub fn text<S: WindowShape<Payload = String>, W: Wake>(
        &mut self,
        bindings: &mut Bindings,
        output: Output<S>,
        bytes: usize,
        (now, wake): (EngineTime, &mut W),
        compose: impl FnOnce(&mut String, &Bindings, &Self),
    ) -> NodeResult {
        let (to, head, count) = self.destination(bindings, output, now)?;
        if self.values.scalar::<String>(to.fields()).capacity() < bytes {
            return Err(NodeError::new("prepared rolling text capacity exceeded"));
        }
        let mut destination = std::mem::take(self.values.scalar_mut::<String>(to.fields()));
        destination.clear();
        compose(&mut destination, bindings, self);
        *self.values.scalar_mut::<String>(to.fields()) = destination;
        self.commit(bindings, output, now, wake, (head, count));
        Ok(())
    }
    /// Copy an independently owned source slot, then publish and apply arrival eviction.
    pub fn from<S: WindowShape, W: Wake>(
        &mut self,
        bindings: &mut Bindings,
        output: Output<S>,
        source: &ValueColumns,
        from: ValueSlot<S::Payload>,
        (now, wake): (EngineTime, &mut W),
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        let (to, head, count) = self.destination(bindings, output, now)?;
        S::Payload::check_slots(source, from, &self.values, to)?;
        S::Payload::copy_between(source, from, &mut self.values, to);
        self.commit(bindings, output, now, wake, (head, count));
        Ok(())
    }
    /// Retain a native arrival into reserved storage before changing the retained window.
    pub fn write<S: WindowShape, W: Wake>(
        &mut self,
        bindings: &mut Bindings,
        output: Output<S>,
        value: &<S::Payload as GlobalValue>::Value,
        now: EngineTime,
        wake: &mut W,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        let (to, head, count) = self.destination(bindings, output, now)?;
        S::Payload::check_native(&self.values, to, value)?;
        S::Payload::copy_native(&mut self.values, to, value);
        self.commit(bindings, output, now, wake, (head, count));
        Ok(())
    }
    /// Forward the current arrival into an independent output window at its new time.
    pub fn pass<S: WindowShape, W: Wake>(
        &mut self,
        bindings: &mut Bindings,
        input: Input<S>,
        output: Output<S>,
        now: EngineTime,
        wake: &mut W,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        let from = self.borrow(bindings, input)?;
        let (to, head, count) = self.destination(bindings, output, now)?;
        S::Payload::check_slots(&self.values, from, &self.values, to)?;
        S::Payload::copy_within(&mut self.values, from, to);
        self.commit(bindings, output, now, wake, (head, count));
        Ok(())
    }
}
