//! Typed leaf layouts for independently owned ordinary nominal values.
use hgl_columns::Scalar;
pub use hgl_global_arena::{Capacity, Columns as ValueColumns, Layouts, ListData};
use hgl_types::{Date, EngineDelta, EngineTime, NodeError, OrdinaryType, ScalarType, Time};
use std::{fmt::Debug, marker::PhantomData};

/// A compiler-selected value representation; implementations preserve their schema.
pub trait GlobalValue {
    /// Nominal scalar storage already exists before the first publication.
    const PREPARED_SCALAR: bool = false;
    /// Independently owned representation, without borrow capabilities.
    type Value;
    /// Prepared typed field positions, copied without copying payloads.
    type Slots: Copy;
    /// Number of positions in this value's immediate flattened layout.
    const WIDTH: usize;
    /// Stage all layout buffers and count typed slots before changing live storage.
    fn prepare(
        value: &Self::Value,
        capacity: &mut Capacity,
        layouts: &mut Layouts,
    ) -> hgl_types::NodeResult;
    /// Install a complete value into reserved reusable slots.
    fn install(
        columns: &mut ValueColumns,
        value: Self::Value,
        layouts: &mut Layouts,
    ) -> Self::Slots;
    /// Release every owned descendant without allocation.
    fn release(columns: &mut ValueColumns, slots: Self::Slots);
    /// Serialize prepared positions in declaration order without payload inspection.
    fn flatten(slots: Self::Slots, layout: &mut [usize]);
    /// Exact canonical type, inspected only during construction.
    fn schema() -> OrdinaryType;
    /// Consume scalar positions in schema declaration order during construction.
    fn slots(layout: &mut &[usize]) -> Self::Slots;
    /// Retain every child independently before any destination is modified.
    fn retain(value: &Self::Value) -> Result<Self::Value, Box<NodeError>>;
    /// Extract an independently owned value at an explicit retention boundary.
    fn read(columns: &ValueColumns, slots: Self::Slots) -> Result<Self::Value, Box<NodeError>>;
    /// Move a completely retained value into its prepared storage, without failure.
    fn commit(
        columns: &mut ValueColumns,
        slots: Self::Slots,
        value: Self::Value,
        layouts: &mut Layouts,
    );
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
    /// Reconstruct a typed projection from compile-time selected field positions.
    pub fn from_fields(fields: T::Slots) -> Self {
        Self {
            fields,
            value: PhantomData,
        }
    }

    /// Bind the generated type's layout before any hooks execute.
    pub fn bind(layout: &mut &[usize]) -> Self {
        Self {
            fields: T::slots(layout),
            value: PhantomData,
        }
    }
    /// Install a complete value into previously reserved slots.
    pub fn install(columns: &mut ValueColumns, value: T::Value, layouts: &mut Layouts) -> Self {
        Self {
            fields: T::install(columns, value, layouts),
            value: PhantomData,
        }
    }
    /// Release typed owned descendants for subsequent reuse.
    pub fn release(self, columns: &mut ValueColumns) {
        T::release(columns, self.fields);
    }
    /// Fill a preallocated declaration-ordered position layout.
    pub fn flatten(self, layout: &mut [usize]) {
        T::flatten(self.fields, layout);
    }
    /// Project prepared fields without inspecting or copying payloads.
    pub fn fields(self) -> T::Slots {
        self.fields
    }
    /// Copy explicitly retained values from this typed position.
    pub fn read(self, columns: &ValueColumns) -> Result<T::Value, Box<NodeError>> {
        T::read(columns, self.fields)
    }
    /// Move fully retained values into this typed position.
    pub fn commit(self, columns: &mut ValueColumns, value: T::Value, layouts: &mut Layouts) {
        T::commit(columns, self.fields, value, layouts);
    }
}
impl<T: Scalar> GlobalValue for T {
    type Value = T;
    type Slots = usize;
    const WIDTH: usize = 1;
    fn prepare(_: &T, capacity: &mut Capacity, _: &mut Layouts) -> hgl_types::NodeResult {
        capacity.scalar::<T>();
        Ok(())
    }
    fn install(columns: &mut ValueColumns, value: T, _: &mut Layouts) -> usize {
        columns.insert(value)
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        columns.release::<T>(slot);
    }
    fn flatten(slot: usize, layout: &mut [usize]) {
        layout[0] = slot;
    }
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
    fn read(columns: &ValueColumns, slot: usize) -> Result<T, Box<NodeError>> {
        columns.scalar::<T>(slot).try_clone()
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: T, _: &mut Layouts) {
        columns.replace(slot, value);
    }
}

/// Allocate a schema's root positions during preparation, never inside hooks.
pub fn allocate(
    ty: &OrdinaryType,
    columns: &mut ValueColumns,
    slots: &mut Vec<usize>,
) -> hgl_types::NodeResult {
    match ty {
        OrdinaryType::Tuple(fields) => {
            for field in fields {
                allocate(field, columns, slots)?;
            }
        }
        OrdinaryType::Struct(_, fields) => {
            for (_, field) in fields {
                allocate(field, columns, slots)?;
            }
        }
        OrdinaryType::List(_, _) => {
            let mut capacity = Capacity::default();
            capacity.list();
            columns.reserve(&capacity)?;
            slots.push(columns.insert_list(Vec::new()));
        }
        OrdinaryType::Enum(_) => slots.push(leaf::<i64>(columns)?),
        OrdinaryType::Scalar(scalar) => slots.push(match scalar {
            ScalarType::Bool => leaf::<bool>(columns)?,
            ScalarType::I64 => leaf::<i64>(columns)?,
            ScalarType::F64 => leaf::<f64>(columns)?,
            ScalarType::Text => leaf::<String>(columns)?,
            ScalarType::Date => leaf::<Date>(columns)?,
            ScalarType::Time => leaf::<Time>(columns)?,
            ScalarType::DateTime => leaf::<EngineTime>(columns)?,
            ScalarType::Duration => leaf::<EngineDelta>(columns)?,
            ScalarType::CivilDateTime => leaf::<hgl_types::CivilDateTime>(columns)?,
            ScalarType::TimeZone => leaf::<hgl_types::ZoneId>(columns)?,
            ScalarType::ZonedTime => leaf::<hgl_types::ZonedTime>(columns)?,
            ScalarType::ZonedDateTime => leaf::<hgl_types::ZonedDateTime>(columns)?,
        }),
    }
    Ok(())
}
fn leaf<T: Scalar>(columns: &mut ValueColumns) -> Result<usize, Box<NodeError>> {
    let mut capacity = Capacity::default();
    capacity.scalar::<T>();
    columns.reserve(&capacity)?;
    Ok(columns.insert(T::default()))
}
