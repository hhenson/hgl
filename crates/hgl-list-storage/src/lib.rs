//! Owning and finite-prepared homogeneous ordinary list storage.
use hgl_global_value::{Capacity, GlobalValue, Layouts, ListData, ValueColumns};
use hgl_types::{NodeError, NodeResult, OrdinaryType};
use std::marker::PhantomData;
mod prepared;
pub use prepared::{ListBounds, append_slot, commit_append};
/// A homogeneous ordinary list; -1 is unbounded and nonnegative N is exact length.
#[derive(Debug)]
pub struct List<T: GlobalValue, const N: i64 = -1>(PhantomData<T>);
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
    let length = i64::try_from(values.len()).map_err(|e| NodeError::new(e.to_string()))?;
    if N < -1 || (N >= 0 && length != N) {
        return Err(NodeError::new("fixed list length mismatch"));
    }
    Ok(())
}
