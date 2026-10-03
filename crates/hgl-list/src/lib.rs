//! Ordinary typed lists with owning retention and prepared borrowed projections.
use hgl_global_value::{Capacity, GlobalValue, Layouts, ListData, ValueColumns, ValueSlot};
use hgl_types::{NodeError, NodeResult, OrdinaryType};
use std::marker::PhantomData;

/// A homogeneous ordinary list; -1 is unbounded and nonnegative N is exact length.
#[derive(Debug)]
pub struct List<T: GlobalValue, const N: i64 = -1>(PhantomData<T>);

/// Ordinary lengths never expose a type-size sentinel or exceed i64.
pub fn list_len<T>(values: &[T]) -> NodeResult<i64> {
    i64::try_from(values.len())
        .map_err(|error| NodeError::new(format!("list length exceeds i64: {error}")))
}
/// A primitive read can retain the resulting value; an aggregate remains a projection.
pub fn list_index<T>(values: &[T], index: i64) -> NodeResult<&T> {
    usize::try_from(index)
        .ok()
        .and_then(|index| values.get(index))
        .ok_or_else(|| NodeError::new(format!("list index {index} out of bounds")))
}
/// Internal owner projection for admitted content mutation, not indexed replacement syntax.
pub fn list_index_mut<T>(values: &mut [T], index: i64) -> NodeResult<&mut T> {
    usize::try_from(index)
        .ok()
        .and_then(|index| values.get_mut(index))
        .ok_or_else(|| NodeError::new(format!("list index {index} out of bounds")))
}
/// Retain the item completely and obtain capacity before extending an owning list.
pub fn list_push<T: GlobalValue>(values: &mut Vec<T::Value>, item: &T::Value) -> NodeResult {
    growth(values.len())?;
    let owned = T::retain(item)?;
    values
        .try_reserve(1)
        .map_err(|error| NodeError::new(error.to_string()))?;
    values.push(owned);
    Ok(())
}
/// Read the length of a prepared list without copying any payload.
pub fn global_len<T: GlobalValue, const N: i64>(
    columns: &ValueColumns,
    slot: ValueSlot<List<T, N>>,
) -> NodeResult<i64> {
    list_len(columns.list(slot.fields()))
}
/// Bounds-check and reconstruct typed element positions without copying the element.
pub fn global_index<T: GlobalValue, const N: i64>(
    columns: &ValueColumns,
    slot: ValueSlot<List<T, N>>,
    index: i64,
) -> NodeResult<ValueSlot<T>> {
    Ok(ValueSlot::bind(
        &mut list_index(columns.list(slot.fields()), index)?.as_slice(),
    ))
}
/// Append one retained item after every fallible operation has completed.
pub fn global_push<T: GlobalValue>(
    columns: &mut ValueColumns,
    slot: ValueSlot<List<T>>,
    item: &T::Value,
) -> NodeResult {
    growth(columns.list(slot.fields()).len())?;
    let owned = T::retain(item)?;
    let mut capacity = Capacity::default();
    let mut layouts = Layouts::default();
    T::prepare(&owned, &mut capacity, &mut layouts)?;
    let mut layout = positions(T::WIDTH)?;
    columns.reserve(&capacity)?;
    columns
        .list_mut(slot.fields())
        .try_reserve(1)
        .map_err(|error| NodeError::new(error.to_string()))?;
    let element = T::install(columns, owned, &mut layouts);
    T::flatten(element, &mut layout);
    columns.list_mut(slot.fields()).push(layout);
    Ok(())
}
impl<T: GlobalValue, const N: i64> GlobalValue for List<T, N> {
    type Value = Vec<T::Value>;
    type Slots = usize;
    const WIDTH: usize = 1;
    fn schema() -> OrdinaryType {
        OrdinaryType::List(Box::new(T::schema()), usize::try_from(N).ok())
    }
    fn slots(layout: &mut &[usize]) -> usize {
        let slot = layout[0];
        *layout = &layout[1..];
        slot
    }
    fn retain(value: &Self::Value) -> NodeResult<Self::Value> {
        check::<_, N>(value)?;
        let mut owned = Vec::new();
        owned
            .try_reserve(value.len())
            .map_err(|error| NodeError::new(error.to_string()))?;
        for item in value {
            owned.push(T::retain(item)?);
        }
        Ok(owned)
    }
    fn prepare(value: &Self::Value, capacity: &mut Capacity, layouts: &mut Layouts) -> NodeResult {
        check::<_, N>(value)?;
        capacity.list();
        let mut items = Vec::new();
        items
            .try_reserve(value.len())
            .map_err(|error| NodeError::new(error.to_string()))?;
        for _ in value {
            items.push(positions(T::WIDTH)?);
        }
        layouts.push(items)?;
        for item in value {
            T::prepare(item, capacity, layouts)?;
        }
        Ok(())
    }
    fn read(columns: &ValueColumns, slot: usize) -> NodeResult<Self::Value> {
        let data = columns.list(slot);
        let mut value = Vec::new();
        value
            .try_reserve(data.len())
            .map_err(|error| NodeError::new(error.to_string()))?;
        for layout in data {
            value.push(T::read(columns, T::slots(&mut layout.as_slice()))?);
        }
        Ok(value)
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: Self::Value, layouts: &mut Layouts) {
        let data = install_elements::<T>(columns, value, layouts);
        let previous = columns.replace_list(slot, data);
        release_elements::<T>(columns, previous);
    }
    fn install(columns: &mut ValueColumns, value: Self::Value, layouts: &mut Layouts) -> usize {
        let data = install_elements::<T>(columns, value, layouts);
        columns.insert_list(data)
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        let previous = columns.replace_list(slot, Vec::new());
        release_elements::<T>(columns, previous);
        columns.release_list(slot);
    }
    fn flatten(slot: usize, layout: &mut [usize]) {
        layout[0] = slot;
    }
}
fn install_elements<T: GlobalValue>(
    columns: &mut ValueColumns,
    value: Vec<T::Value>,
    layouts: &mut Layouts,
) -> ListData {
    let mut data = layouts.take();
    for (item, layout) in value.into_iter().zip(&mut data) {
        T::flatten(T::install(columns, item, layouts), layout);
    }
    data
}
fn release_elements<T: GlobalValue>(columns: &mut ValueColumns, data: ListData) {
    for layout in data {
        T::release(columns, T::slots(&mut layout.as_slice()));
    }
}
fn positions(width: usize) -> NodeResult<Vec<usize>> {
    let mut layout = Vec::new();
    layout
        .try_reserve(width)
        .map_err(|error| NodeError::new(error.to_string()))?;
    layout.resize(width, 0);
    Ok(layout)
}
fn check<T, const N: i64>(values: &[T]) -> NodeResult {
    let length = list_len(values)?;
    if N < -1 || (N >= 0 && length != N) {
        return Err(NodeError::new("fixed list length mismatch"));
    }
    Ok(())
}
fn growth(length: usize) -> NodeResult {
    if i64::try_from(length)
        .ok()
        .and_then(|length| length.checked_add(1))
        .is_none()
    {
        return Err(NodeError::new("list length exceeds i64"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn i64_length_limit_rejects_growth_before_overflow() {
        if let Ok(maximum) = usize::try_from(i64::MAX) {
            assert!(super::growth(maximum).is_err());
            assert!(super::growth(maximum - 1).is_ok());
        }
    }
}
