//! Typed leaf layouts for independently owned ordinary nominal values.
pub use hgl_columns::Columns;
use hgl_columns::Scalar;
use hgl_types::{Date, EngineDelta, EngineTime, NodeError, OrdinaryType, ScalarType, Time};
use std::{fmt::Debug, marker::PhantomData};

/// A compiler-selected value representation; implementations preserve their schema.
pub trait GlobalValue {
    /// Independently owned representation, without borrow capabilities.
    type Value;
    /// Prepared typed field positions, copied without copying payloads.
    type Slots: Copy;
    /// Exact canonical type, inspected only during construction.
    fn schema() -> OrdinaryType;
    /// Consume scalar positions in schema declaration order during construction.
    fn slots(layout: &mut &[usize]) -> Self::Slots;
    /// Retain every child independently before any destination is modified.
    fn retain(value: &Self::Value) -> Result<Self::Value, Box<NodeError>>;
    /// Extract an independently owned value at an explicit retention boundary.
    fn read(columns: &Columns, slots: Self::Slots) -> Result<Self::Value, Box<NodeError>>;
    /// Move a completely retained value into its prepared storage, without failure.
    fn commit(columns: &mut Columns, slots: Self::Slots, value: Self::Value);
}

/// Prepared projection into run-owned storage; lexical permissions are checked upstream.
pub struct ValueSlot<T: GlobalValue> {
    fields: T::Slots,
    value: PhantomData<T>,
}
impl<T: GlobalValue> Copy for ValueSlot<T> {}
impl<T: GlobalValue> Clone for ValueSlot<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T: GlobalValue> Debug for ValueSlot<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValueSlot").finish_non_exhaustive()
    }
}
impl<T: GlobalValue> ValueSlot<T> {
    /// Bind the generated type's layout before any hooks execute.
    pub fn bind(layout: &mut &[usize]) -> Self {
        Self {
            fields: T::slots(layout),
            value: PhantomData,
        }
    }
    /// Project prepared fields without inspecting or copying payloads.
    pub fn fields(self) -> T::Slots {
        self.fields
    }
    /// Copy explicitly retained values from this typed position.
    pub fn read(self, columns: &Columns) -> Result<T::Value, Box<NodeError>> {
        T::read(columns, self.fields)
    }
    /// Move fully retained values into this typed position.
    pub fn commit(self, columns: &mut Columns, value: T::Value) {
        T::commit(columns, self.fields, value);
    }
}
impl<T: Scalar> GlobalValue for T {
    type Value = T;
    type Slots = usize;
    fn schema() -> OrdinaryType {
        OrdinaryType::Scalar(T::TYPE)
    }
    fn slots(layout: &mut &[usize]) -> usize {
        let slot = layout[0];
        *layout = &layout[1..];
        slot
    }
    fn retain(value: &T) -> Result<T, Box<NodeError>> {
        value.try_clone()
    }
    fn read(columns: &Columns, slot: usize) -> Result<T, Box<NodeError>> {
        T::column(columns)[slot].try_clone()
    }
    fn commit(columns: &mut Columns, slot: usize, value: T) {
        T::column_mut(columns)[slot] = value;
    }
}

/// Allocate a finite schema's typed leaves during preparation, never inside hooks.
pub fn allocate(ty: &OrdinaryType, columns: &mut Columns, slots: &mut Vec<usize>) {
    match ty {
        OrdinaryType::Struct(_, fields) => {
            for (_, field) in fields {
                allocate(field, columns, slots);
            }
        }
        OrdinaryType::Scalar(scalar) => slots.push(match scalar {
            ScalarType::Bool => leaf::<bool>(columns),
            ScalarType::I64 => leaf::<i64>(columns),
            ScalarType::F64 => leaf::<f64>(columns),
            ScalarType::Text => leaf::<String>(columns),
            ScalarType::Date => leaf::<Date>(columns),
            ScalarType::Time => leaf::<Time>(columns),
            ScalarType::DateTime => leaf::<EngineTime>(columns),
            ScalarType::Duration => leaf::<EngineDelta>(columns),
        }),
    }
}
fn leaf<T: Scalar>(columns: &mut Columns) -> usize {
    let column = T::column_mut(columns);
    let slot = column.len();
    column.push(T::default());
    slot
}
