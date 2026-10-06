//! Typed independent recursive edges; finite payloads determine storage depth.
use hgl_global_value::{Capacity, GlobalValue, Layouts, ValueColumns, ValueSlot};
use hgl_prepared_value::PreparedValue;
use hgl_types::{NodeError, NodeResult, OrdinaryType};
use std::marker::PhantomData;
/// Exact target identity supplied by a generated concrete nominal marker.
pub trait RecursiveTarget: GlobalValue {
    /// Canonical fully applied nominal source name.
    const IDENTITY: &'static str;
}
/// Internal owning indirection beneath an optional recursive field.
#[derive(Debug)]
pub struct Recursive<T: RecursiveTarget>(PhantomData<T>);
impl<T: RecursiveTarget> GlobalValue for Recursive<T> {
    type Value = Box<T::Value>;
    type Slots = usize;
    const WIDTH: usize = 1;
    fn schema() -> OrdinaryType {
        OrdinaryType::RecursiveReference(T::IDENTITY)
    }
    fn slots(layout: &mut &[usize]) -> usize {
        let slot = layout[0];
        *layout = &layout[1..];
        slot
    }
    fn flatten(slot: usize, layout: &mut [usize]) {
        layout[0] = slot;
    }
    fn retain(value: &Self::Value) -> NodeResult<Self::Value> {
        Ok(Box::new(T::retain(value)?))
    }
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        capacity.list();
        layouts.push(vec![vec![0; T::WIDTH]])?;
        T::prepare(value, capacity, layouts)
    }
    fn install(columns: &mut ValueColumns, value: Self::Value, layouts: &mut Layouts) -> usize {
        let mut positions = layouts.take();
        T::flatten(T::install(columns, *value, layouts), &mut positions[0]);
        columns.insert_list(positions)
    }
    fn read(columns: &ValueColumns, slot: usize) -> NodeResult<Self::Value> {
        Ok(Box::new(element::<T>(columns, slot).read(columns)?))
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: Self::Value, layouts: &mut Layouts) {
        let mut positions = layouts.take();
        T::flatten(T::install(columns, *value, layouts), &mut positions[0]);
        let old = columns.replace_list(slot, positions);
        T::release(columns, T::slots(&mut old[0].as_slice()));
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        let old = columns.replace_list(slot, Vec::new());
        T::release(columns, T::slots(&mut old[0].as_slice()));
        columns.release_list(slot);
    }
}
impl<T: RecursiveTarget + PreparedValue> PreparedValue for Recursive<T> {
    type Bounds = Box<T::Bounds>;
    fn include(bounds: &mut Self::Bounds, value: &Self::Value) {
        T::include(bounds, value);
    }
    fn allocate(columns: &mut ValueColumns, bounds: &Self::Bounds) -> NodeResult<ValueSlot<Self>> {
        let child = T::allocate(columns, bounds)?;
        let mut layout = Vec::new();
        layout
            .try_reserve(T::WIDTH)
            .map_err(|e| NodeError::new(e.to_string()))?;
        layout.resize(T::WIDTH, 0);
        child.flatten(&mut layout);
        let mut items = Vec::new();
        items
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        items.push(layout);
        let mut capacity = Capacity::default();
        capacity.list();
        columns.reserve(&capacity)?;
        let slot = columns.insert_list(items);
        columns.set_list_len(slot, 1);
        Ok(ValueSlot::from_fields(slot))
    }
    fn check_native(
        columns: &ValueColumns,
        to: ValueSlot<Self>,
        value: &Self::Value,
    ) -> NodeResult {
        T::check_native(columns, element(columns, to.fields()), value)
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        T::check_slots(
            source,
            element(source, from.fields()),
            destination,
            element(destination, to.fields()),
        )
    }
    fn copy_native(columns: &mut ValueColumns, to: ValueSlot<Self>, value: &Self::Value) {
        T::copy_native(columns, element(columns, to.fields()), value);
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        T::copy_between(
            source,
            element(source, from.fields()),
            destination,
            element(destination, to.fields()),
        );
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        T::copy_within(
            columns,
            element(columns, from.fields()),
            element(columns, to.fields()),
        );
    }
}
fn element<T: GlobalValue>(columns: &ValueColumns, slot: usize) -> ValueSlot<T> {
    ValueSlot::bind(&mut columns.list(slot)[0].as_slice())
}
