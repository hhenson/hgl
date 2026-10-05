use crate::List;
use hgl_global_value::{Capacity, ValueColumns, ValueSlot};
use hgl_prepared_value::PreparedValue;
use hgl_types::{NodeError, NodeResult};
/// Maximum finite length and merged descendant bounds for independently owned lists.
#[derive(Debug)]
pub struct ListBounds<T: PreparedValue> {
    /// Largest supported logical length.
    pub len: usize,
    /// Capacity maxima shared by each independently allocated element.
    pub element: T::Bounds,
}
impl<T: PreparedValue> Default for ListBounds<T> {
    fn default() -> Self {
        Self {
            len: 0,
            element: T::Bounds::default(),
        }
    }
}
impl<T: PreparedValue, const N: i64> PreparedValue for List<T, N> {
    type Bounds = ListBounds<T>;
    fn include(bounds: &mut Self::Bounds, value: &Self::Value) {
        bounds.len = bounds.len.max(value.len());
        for item in value {
            T::include(&mut bounds.element, item);
        }
    }
    fn allocate(columns: &mut ValueColumns, bounds: &Self::Bounds) -> NodeResult<ValueSlot<Self>> {
        let length = usize::try_from(N).unwrap_or(bounds.len);
        let mut items = Vec::new();
        items
            .try_reserve(length)
            .map_err(|e| NodeError::new(e.to_string()))?;
        for _ in 0..length {
            let item = T::allocate(columns, &bounds.element)?;
            let mut layout = Vec::new();
            layout
                .try_reserve(T::WIDTH)
                .map_err(|e| NodeError::new(e.to_string()))?;
            layout.resize(T::WIDTH, 0);
            item.flatten(&mut layout);
            items.push(layout);
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
        length::<N>(value.len(), columns.prepared_list(to.fields()).len())?;
        for (index, item) in value.iter().enumerate() {
            T::check_native(columns, element(columns, to.fields(), index), item)?;
        }
        Ok(())
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        let count = source.list(from.fields()).len();
        length::<N>(count, destination.prepared_list(to.fields()).len())?;
        for index in 0..count {
            T::check_slots(
                source,
                element(source, from.fields(), index),
                destination,
                element(destination, to.fields(), index),
            )?;
        }
        Ok(())
    }
    fn copy_native(columns: &mut ValueColumns, to: ValueSlot<Self>, value: &Self::Value) {
        for (index, item) in value.iter().enumerate() {
            T::copy_native(columns, element(columns, to.fields(), index), item);
        }
        columns.set_list_len(to.fields(), value.len());
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        let count = source.list(from.fields()).len();
        for index in 0..count {
            T::copy_between(
                source,
                element(source, from.fields(), index),
                destination,
                element(destination, to.fields(), index),
            );
        }
        destination.set_list_len(to.fields(), count);
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        if from.fields() == to.fields() {
            return;
        }
        let count = columns.list(from.fields()).len();
        for index in 0..count {
            T::copy_within(
                columns,
                element(columns, from.fields(), index),
                element(columns, to.fields(), index),
            );
        }
        columns.set_list_len(to.fields(), count);
    }
}
fn element<T: PreparedValue>(columns: &ValueColumns, slot: usize, index: usize) -> ValueSlot<T> {
    ValueSlot::bind(&mut columns.prepared_list(slot)[index].as_slice())
}
fn length<const N: i64>(length: usize, capacity: usize) -> NodeResult {
    if N < -1 || (N >= 0 && usize::try_from(N).ok() != Some(length)) {
        return Err(NodeError::new("fixed list length mismatch"));
    }
    if length > capacity || i64::try_from(length).is_err() {
        return Err(NodeError::new("prepared list capacity exceeded"));
    }
    Ok(())
}
/// Find the next independently prepared record element without publishing its length.
pub fn append_slot<T: PreparedValue>(
    columns: &ValueColumns,
    list: ValueSlot<List<T>>,
) -> NodeResult<ValueSlot<T>> {
    let count = columns.list(list.fields()).len();
    length::<-1>(
        count.saturating_add(1),
        columns.prepared_list(list.fields()).len(),
    )?;
    Ok(element(columns, list.fields(), count))
}
/// Publish one fully written record element after successful preflight and copying.
pub fn commit_append<T: PreparedValue>(columns: &mut ValueColumns, list: ValueSlot<List<T>>) {
    columns.set_list_len(list.fields(), columns.list(list.fields()).len() + 1);
}
