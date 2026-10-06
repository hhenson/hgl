use crate::columns::Scalar;
use crate::global_value::{Capacity, ValueColumns, ValueSlot};
use crate::prepared_value::PreparedValue;
use hgl_types::{NodeError, NodeResult};
impl<T: Scalar> PreparedValue for T {
    type Bounds = usize;
    fn include(bounds: &mut usize, value: &T) {
        *bounds = (*bounds).max(value.size());
    }
    fn allocate(columns: &mut ValueColumns, bounds: &usize) -> NodeResult<ValueSlot<Self>> {
        let mut value = T::default();
        value.reserve(*bounds)?;
        let mut capacity = Capacity::default();
        capacity.scalar::<T>();
        columns.reserve(&capacity)?;
        Ok(ValueSlot::from_fields(columns.insert(value)))
    }
    fn check_native(columns: &ValueColumns, to: ValueSlot<Self>, value: &T) -> NodeResult {
        if columns.scalar::<T>(to.fields()).capacity() < value.size() {
            return Err(NodeError::new("prepared scalar capacity exceeded"));
        }
        Ok(())
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        Self::check_native(destination, to, source.scalar::<T>(from.fields()))
    }
    fn copy_native(columns: &mut ValueColumns, to: ValueSlot<Self>, value: &T) {
        columns.scalar_mut::<T>(to.fields()).copy_from(value);
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        Self::copy_native(destination, to, source.scalar::<T>(from.fields()));
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        columns.copy_scalar::<T>(from.fields(), to.fields());
    }
}
