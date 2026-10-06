//! Internal field-presence storage; no nullable source type or field operation.
use crate::global_value::{Capacity, GlobalValue, Layouts, ValueColumns};
use hgl_types::{NodeResult, OrdinaryType};
use std::marker::PhantomData;
mod prepared;
/// Independently retained optional ordinary field payload.
#[derive(Debug)]
pub struct Optional<T: GlobalValue>(PhantomData<T>);
impl<T: GlobalValue> GlobalValue for Optional<T> {
    type Value = Option<T::Value>;
    type Slots = usize;
    const WIDTH: usize = 1;
    fn schema() -> OrdinaryType {
        OrdinaryType::OptionalField(Box::new(T::schema()))
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
        value.as_ref().map(T::retain).transpose()
    }
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        capacity.list();
        let positions = value
            .as_ref()
            .map(|_| vec![0; T::WIDTH])
            .into_iter()
            .collect();
        layouts.push(positions)?;
        if let Some(value) = value {
            T::prepare(value, capacity, layouts)?;
        }
        Ok(())
    }
    fn install(columns: &mut ValueColumns, value: Self::Value, layouts: &mut Layouts) -> usize {
        let mut positions = layouts.take();
        if let Some(value) = value {
            T::flatten(T::install(columns, value, layouts), &mut positions[0]);
        }
        columns.insert_list(positions)
    }
    fn read(columns: &ValueColumns, slot: usize) -> NodeResult<Self::Value> {
        columns
            .list(slot)
            .first()
            .map(|positions| T::read(columns, T::slots(&mut positions.as_slice())))
            .transpose()
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: Self::Value, layouts: &mut Layouts) {
        let mut positions = layouts.take();
        if let Some(value) = value {
            T::flatten(T::install(columns, value, layouts), &mut positions[0]);
        }
        let old = columns.replace_list(slot, positions);
        if let Some(positions) = old.first() {
            T::release(columns, T::slots(&mut positions.as_slice()));
        }
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        let old = columns.replace_list(slot, Vec::new());
        if let Some(positions) = old.first() {
            T::release(columns, T::slots(&mut positions.as_slice()));
        }
        columns.release_list(slot);
    }
}
