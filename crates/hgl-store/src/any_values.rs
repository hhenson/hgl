//! Whole boxes retain independent buffers in the existing ordinary arena.
use crate::{Capacity, GlobalValue, Layouts, PreparedValue, ValueColumns, ValueSlot};
use hgl_types::{AnyValue, NodeError, NodeResult, OrdinaryType};

/// The canonical owning any marker; its contained shape never becomes endpoints.
#[derive(Debug)]
pub struct Any;
impl ValueColumns {
    pub(crate) fn reserve_any(&mut self, count: usize) -> NodeResult {
        let extra = count.saturating_sub(self.free_anys.len());
        self.anys
            .try_reserve(extra)
            .map_err(|error| NodeError::new(error.to_string()))?;
        self.free_anys
            .try_reserve(
                self.anys
                    .len()
                    .saturating_add(extra)
                    .saturating_sub(self.free_anys.len()),
            )
            .map_err(|error| NodeError::new(error.to_string()))
    }
    /// Observe the complete held box through a typed ordinary slot.
    pub fn any(&self, slot: usize) -> &AnyValue {
        &self.anys[slot]
    }
    /// Mutate only an authorized owning destination.
    pub fn any_mut(&mut self, slot: usize) -> &mut AnyValue {
        &mut self.anys[slot]
    }
    pub(crate) fn insert_any(&mut self, value: AnyValue) -> usize {
        if let Some(slot) = self.free_anys.pop() {
            self.anys[slot] = value;
            slot
        } else {
            let slot = self.anys.len();
            self.anys.push(value);
            slot
        }
    }
}
impl GlobalValue for Any {
    type Value = AnyValue;
    type Slots = usize;
    const WIDTH: usize = 1;
    const PREPARED_SCALAR: bool = true;
    fn prepare(_: &AnyValue, capacity: &mut Capacity, _: &mut Layouts) -> NodeResult {
        capacity.any();
        Ok(())
    }
    fn install(columns: &mut ValueColumns, value: AnyValue, _: &mut Layouts) -> usize {
        columns.insert_any(value)
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        columns.anys[slot] = AnyValue::default();
        columns.free_anys.push(slot);
    }
    fn flatten(slot: usize, layout: &mut [usize]) {
        layout[0] = slot;
    }
    fn schema() -> OrdinaryType {
        OrdinaryType::Any
    }
    fn slots(layout: &mut &[usize]) -> usize {
        let slot = layout[0];
        *layout = &layout[1..];
        slot
    }
    fn retain(value: &AnyValue) -> NodeResult<AnyValue> {
        value.retain()
    }
    fn read(columns: &ValueColumns, slot: usize) -> NodeResult<AnyValue> {
        columns.any(slot).retain()
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: AnyValue, _: &mut Layouts) {
        columns.anys[slot] = value;
    }
}
impl PreparedValue for Any {
    type Bounds = usize;
    fn include(bounds: &mut usize, value: &AnyValue) {
        *bounds = (*bounds).max(value.data().len());
    }
    fn allocate(columns: &mut ValueColumns, bounds: &usize) -> NodeResult<ValueSlot<Self>> {
        let mut value = AnyValue::default();
        value.reserve(*bounds)?;
        let mut capacity = Capacity::default();
        capacity.any();
        columns.reserve(&capacity)?;
        Ok(ValueSlot::from_fields(columns.insert_any(value)))
    }
    fn check_native(columns: &ValueColumns, to: ValueSlot<Self>, value: &AnyValue) -> NodeResult {
        if columns.any(to.fields()).capacity() < value.data().len() {
            return Err(NodeError::new("prepared any capacity exceeded"));
        }
        Ok(())
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        Self::check_native(destination, to, source.any(from.fields()))
    }
    fn copy_native(columns: &mut ValueColumns, to: ValueSlot<Self>, value: &AnyValue) {
        columns.any_mut(to.fields()).copy_prepared(value);
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        Self::copy_native(destination, to, source.any(from.fields()));
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        let (from, to) = (from.fields(), to.fields());
        if from < to {
            let (left, right) = columns.anys.split_at_mut(to);
            right[0].copy_prepared(&left[from]);
        } else if from > to {
            let (left, right) = columns.anys.split_at_mut(from);
            left[to].copy_prepared(&right[0]);
        }
    }
}

impl<W: crate::Wake> crate::PreparedTick<'_, W> {
    /// Compose a complete box only after its typed source and owning width are validated.
    pub fn any(
        &mut self,
        output: crate::shapes::Output<crate::shapes::Atomic<Any>>,
        measure: impl FnOnce(
            crate::Observation<'_>,
        ) -> NodeResult<(Option<&'static hgl_types::AnyPlan>, usize)>,
        compose: impl FnOnce(&mut AnyValue, crate::Observation<'_>),
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let (plan, width) = measure(self.storage.observations().0)?;
        let destination = self
            .storage
            .atomic
            .destination(self.storage.bindings, output)?;
        if self
            .storage
            .atomic
            .values()
            .any(destination.fields())
            .capacity()
            < width
        {
            return Err(NodeError::new("prepared any capacity exceeded"));
        }
        let mut value = std::mem::take(
            self.storage
                .atomic
                .values_mut()
                .any_mut(destination.fields()),
        );
        value.encode_prepared(plan, |data| compose(data, self.storage.observations().0));
        *self
            .storage
            .atomic
            .values_mut()
            .any_mut(destination.fields()) = value;
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
        Ok(())
    }
}

mod keys;
pub(crate) use keys::AnyKeys;

impl ValueColumns {
    /// Encode an independent global owner after complete typed measurement succeeds.
    pub fn compose_any(
        &mut self,
        destination: ValueSlot<Any>,
        measure: impl FnOnce(&Self) -> NodeResult<(Option<&'static hgl_types::AnyPlan>, usize)>,
        compose: impl FnOnce(&Self, &mut AnyValue),
    ) -> NodeResult {
        let (plan, width) = measure(self)?;
        if self.any(destination.fields()).capacity() < width {
            return Err(NodeError::new("prepared any capacity exceeded"));
        }
        let mut value = std::mem::take(self.any_mut(destination.fields()));
        value.encode_prepared(plan, |data| compose(self, data));
        *self.any_mut(destination.fields()) = value;
        Ok(())
    }
}

impl<W: crate::Wake> crate::PreparedTick<'_, W> {
    /// Publish an independent box held in a prepared lexical owner.
    pub fn any_from_global(
        &mut self,
        output: crate::shapes::Output<crate::shapes::Atomic<Any>>,
        source: ValueSlot<Any>,
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let destination = self
            .storage
            .atomic
            .destination(self.storage.bindings, output)?;
        Any::check_slots(
            self.storage.globals.values(),
            source,
            self.storage.atomic.values(),
            destination,
        )?;
        Any::copy_between(
            self.storage.globals.values(),
            source,
            self.storage.atomic.values_mut(),
            destination,
        );
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
        Ok(())
    }
}

/// Compiled descriptor execution over independently retained encoded owners.
pub mod operations;
