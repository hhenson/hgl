//! Ordinary typed lists with owning retention and prepared borrowed projections.
use crate::global_value::{Capacity, GlobalValue, Layouts, ValueColumns, ValueSlot};
pub use crate::list_storage::{List, ListBounds, append_slot, commit_append};
use hgl_types::{NodeError, NodeResult};

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
    let mut layout = Vec::new();
    layout
        .try_reserve(T::WIDTH)
        .map_err(|e| NodeError::new(e.to_string()))?;
    layout.resize(T::WIDTH, 0);
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
