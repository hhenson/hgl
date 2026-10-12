//! Whole boxes retain independent buffers in the existing ordinary arena.
use crate::{Capacity, GlobalValue, Layouts, PreparedValue, ValueColumns, ValueSlot};
use hgl_types::{AnyValue, NodeResult, OrdinaryType};

/// The canonical owning any marker; its contained shape never becomes endpoints.
#[derive(Debug)]
pub struct Any;
impl ValueColumns {
    /// Observe the complete held box through a typed ordinary slot.
    pub fn any(&self, slot: usize) -> &AnyValue {
        &self.anys.get(slot).0
    }
    /// Mutate only an authorized owning destination.
    pub fn any_mut(&mut self, slot: usize) -> &mut AnyValue {
        &mut self.anys.get_mut(slot).0
    }
    pub(crate) fn insert_any(&mut self, value: AnyValue) -> usize {
        self.anys.insert((value, AnyValue::default()))
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
        columns.anys.release(slot);
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
        *columns.any_mut(slot) = value;
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
        let slot = columns.insert_any(value);
        columns.anys.get_mut(slot).1.reserve(*bounds)?;
        Ok(ValueSlot::from_fields(slot))
    }
    fn check_native(columns: &ValueColumns, to: ValueSlot<Self>, value: &AnyValue) -> NodeResult {
        columns
            .any(to.fields())
            .check_capacity(value.data().len())?;
        value.preflight()
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
        columns.anys.copy(from, to, |source, destination| {
            destination.0.copy_prepared(&source.0);
        });
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
        let storage = &mut self.storage;
        let (plan, width) = measure(storage.observations().0)?;
        let destination = storage.atomic.destination(storage.bindings, output)?;
        let mut value = storage.atomic.values_mut().begin_any(destination, width)?;
        value.encode_prepared(plan, |data| compose(data, storage.observations().0));
        storage.atomic.values_mut().finish_any(destination, value);
        storage.bindings.publish(output.id(), self.now, self.wake);
        Ok(())
    }
}

mod keys;
pub(crate) use keys::AnyKeys;

impl ValueColumns {
    fn begin_any(&mut self, destination: ValueSlot<Any>, width: usize) -> NodeResult<AnyValue> {
        let spare = &mut self.anys.get_mut(destination.fields()).1;
        spare.check_capacity(width)?;
        Ok(std::mem::take(spare))
    }
    fn finish_any(&mut self, destination: ValueSlot<Any>, mut value: AnyValue) {
        let (current, spare) = self.anys.get_mut(destination.fields());
        std::mem::swap(current, &mut value);
        *spare = value;
    }
    /// Encode an independent global owner after complete typed measurement succeeds.
    pub fn compose_any(
        &mut self,
        destination: ValueSlot<Any>,
        measure: impl FnOnce(&Self) -> NodeResult<(Option<&'static hgl_types::AnyPlan>, usize)>,
        compose: impl FnOnce(&Self, &mut AnyValue),
    ) -> NodeResult {
        let (plan, width) = measure(self)?;
        let mut value = self.begin_any(destination, width)?;
        value.encode_prepared(plan, |data| compose(self, data));
        self.finish_any(destination, value);
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
        let storage = &mut self.storage;
        let destination = storage.atomic.destination(storage.bindings, output)?;
        Any::check_slots(
            storage.globals.values(),
            source,
            storage.atomic.values(),
            destination,
        )?;
        Any::copy_between(
            storage.globals.values(),
            source,
            storage.atomic.values_mut(),
            destination,
        );
        storage.bindings.publish(output.id(), self.now, self.wake);
        Ok(())
    }
}

/// Compiled descriptor execution over independently retained encoded owners.
pub mod operations;
