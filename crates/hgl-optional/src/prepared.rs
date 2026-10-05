use crate::Optional;
use hgl_global_value::{Capacity, ValueColumns, ValueSlot};
use hgl_list_storage::List;
use hgl_prepared_value::PreparedValue;
use hgl_types::{NodeError, NodeResult};
impl<T: PreparedValue> PreparedValue for Optional<T> {
    type Bounds = Option<T::Bounds>;
    fn include(bounds: &mut Self::Bounds, value: &Self::Value) {
        if let Some(value) = value {
            T::include(bounds.get_or_insert_with(Default::default), value);
        }
    }
    fn allocate(columns: &mut ValueColumns, bounds: &Self::Bounds) -> NodeResult<ValueSlot<Self>> {
        let mut items = Vec::new();
        if let Some(bounds) = bounds {
            let child = T::allocate(columns, bounds)?;
            let mut positions = Vec::new();
            positions
                .try_reserve(T::WIDTH)
                .map_err(|e| NodeError::new(e.to_string()))?;
            positions.resize(T::WIDTH, 0);
            child.flatten(&mut positions);
            items
                .try_reserve(1)
                .map_err(|e| NodeError::new(e.to_string()))?;
            items.push(positions);
        }
        let mut capacity = Capacity::default();
        capacity.list();
        columns.reserve(&capacity)?;
        let slot = columns.insert_list(items);
        columns.set_list_len(slot, 0);
        Ok(ValueSlot::from_fields(slot))
    }
    fn check_native(
        columns: &ValueColumns,
        to: ValueSlot<Self>,
        value: &Self::Value,
    ) -> NodeResult {
        if let Some(value) = value {
            T::check_native(columns, child(columns, to.fields())?, value)?;
        }
        Ok(())
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        List::<T>::check_slots(source, list(from), destination, list(to))
    }
    fn copy_native(columns: &mut ValueColumns, to: ValueSlot<Self>, value: &Self::Value) {
        if let Some(value) = value {
            T::copy_native(columns, prepared(columns, to.fields()), value);
        }
        columns.set_list_len(to.fields(), usize::from(value.is_some()));
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        List::<T>::copy_between(source, list(from), destination, list(to));
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        List::<T>::copy_within(columns, list(from), list(to));
    }
}
fn child<T: PreparedValue>(columns: &ValueColumns, slot: usize) -> NodeResult<ValueSlot<T>> {
    let positions = columns
        .prepared_list(slot)
        .first()
        .ok_or_else(|| NodeError::new("optional field was not prepared for presence"))?;
    Ok(ValueSlot::bind(&mut positions.as_slice()))
}
fn prepared<T: PreparedValue>(columns: &ValueColumns, slot: usize) -> ValueSlot<T> {
    child(columns, slot).unwrap_or_else(|_| unreachable!("complete optional value preflight"))
}

fn list<T: PreparedValue>(slot: ValueSlot<Optional<T>>) -> ValueSlot<List<T>> {
    ValueSlot::from_fields(slot.fields())
}
