//! Statically typed provider values own opaque buffers in independently reserved ordinary slots.
use crate::{Capacity, GlobalValue, Layouts, PreparedValue, ValueColumns, ValueSlot};
use hgl_types::{NodeError, NodeResult, OrdinaryType};
use std::{cmp::Ordering, fmt::Debug, marker::PhantomData};
/// Inert ABI identity checked once before construction; statically linked images remain loaded.
#[derive(Debug)]
pub struct Contract {
    /// Selected provider identity.
    pub provider: &'static str,
    /// Semantic descriptor fingerprint.
    pub fingerprint: &'static str,
    /// Existing canonical scalar identity.
    pub canonical: &'static str,
    /// Owning copy, text, equality, hash, order and serialization capabilities.
    pub capabilities: [bool; 6],
}
impl Contract {
    /// Validate the same checked descriptor before any helper or graph executes.
    pub fn validate(
        &self,
        provider: &str,
        fingerprint: &str,
        canonical: &str,
        caps: [bool; 6],
    ) -> NodeResult {
        if self.provider != provider
            || self.fingerprint != fingerprint
            || self.canonical != canonical
            || !caps
                .iter()
                .zip(self.capabilities)
                .all(|(required, supplied)| !required || supplied)
            || !self.capabilities[0]
            || !self.capabilities[1]
        {
            return Err(NodeError::new("incompatible native provider contract"));
        }
        Ok(())
    }
}
/// A provider-defined opaque buffer ABI; HGL never receives its encoding or writable access.
/// Copy preflight is fallible, while copying after successful preflight is allocation-free.
/// Implementations and their metadata are statically retained until process exit.
pub trait NativeType: Debug + 'static {
    /// Canonical shared capabilities and executable image identity.
    const CONTRACT: Contract;
    /// Check complete owning retention before changing an existing destination.
    fn preflight(data: &[u8], capacity: usize) -> NodeResult;
    /// Independently copy after preflight, preserving destination capacity.
    fn copy(data: &[u8], destination: &mut Vec<u8>);
    /// Construct independent ordinary text through the declared provider operation.
    fn text(data: &[u8]) -> NodeResult<String>;
    /// Provider equality; an undeclared operation remains unavailable to checked HGL.
    fn equal(lhs: &[u8], rhs: &[u8]) -> NodeResult<bool>;
    /// Provider hash consistent with equality.
    fn hash(data: &[u8]) -> NodeResult<u64>;
    /// Provider partial ordering.
    fn compare(lhs: &[u8], rhs: &[u8]) -> NodeResult<Option<Ordering>>;
}
/// An independently owning exact native scalar, without an erased runtime type tag.
#[derive(Debug)]
pub struct NativeValue<T: NativeType> {
    data: Vec<u8>,
    marker: PhantomData<T>,
}
impl<T: NativeType> NativeValue<T> {
    /// Accept an independently owned provider encoding at its typed ABI boundary.
    pub fn from_owned(data: Vec<u8>) -> Self {
        Self {
            data,
            marker: PhantomData,
        }
    }
    /// Borrow a readonly provider view only for its consumer's lifetime.
    pub fn view(&self) -> NativeView<'_, T> {
        NativeView {
            data: &self.data,
            marker: PhantomData,
        }
    }
    /// Retain through the provider's owning-copy contract.
    pub fn retain(&self) -> NodeResult<Self> {
        retain::<T>(&self.data)
    }
}
/// Borrowed exact native argument, never retained by HGL or a generated helper boundary.
#[derive(Debug)]
pub struct NativeView<'a, T: NativeType> {
    data: &'a [u8],
    marker: PhantomData<T>,
}
impl<T: NativeType> Copy for NativeView<'_, T> {}
impl<T: NativeType> Clone for NativeView<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<'a, T: NativeType> NativeView<'a, T> {
    /// Introduce a readonly provider encoding at a statically selected ABI boundary.
    pub fn from_provider(data: &'a [u8]) -> Self {
        Self {
            data,
            marker: PhantomData,
        }
    }
    /// Provider-private encoded data; no HGL field or representation access is introduced.
    pub fn data(&self) -> &'a [u8] {
        self.data
    }
    /// Materialize an owning result in an executed cold context.
    pub fn retain(self) -> NodeResult<NativeValue<T>> {
        retain::<T>(self.data)
    }
}
fn retain<T: NativeType>(data: &[u8]) -> NodeResult<NativeValue<T>> {
    let mut destination = Vec::new();
    destination
        .try_reserve(data.len())
        .map_err(|e| NodeError::new(e.to_string()))?;
    T::preflight(data, destination.capacity())?;
    T::copy(data, &mut destination);
    Ok(NativeValue::from_owned(destination))
}
/// An exact marker whose opaque buffer positions are selected before execution.
#[derive(Debug)]
pub struct Native<T: NativeType>(PhantomData<T>);
impl<T: NativeType> Native<T> {
    /// Preflight an owning result supplied through an explicit borrowed provider ABI.
    pub fn check_view(
        columns: &ValueColumns,
        to: ValueSlot<Self>,
        view: NativeView<'_, T>,
    ) -> NodeResult {
        T::preflight(view.data, columns.natives.get(to.fields()).capacity())
    }
    /// Copy a preflighted provider result into its independently owning destination.
    pub fn copy_view(columns: &mut ValueColumns, to: ValueSlot<Self>, view: NativeView<'_, T>) {
        T::copy(view.data, columns.natives.get_mut(to.fields()));
    }
}
impl ValueColumns {
    /// Read a statically selected native slot without checking a per-value runtime tag.
    pub fn native<T: NativeType>(&self, slot: ValueSlot<Native<T>>) -> NativeView<'_, T> {
        NativeView {
            data: self.natives.get(slot.fields()),
            marker: PhantomData,
        }
    }
}
impl<T: NativeType> GlobalValue for Native<T> {
    type Value = NativeValue<T>;
    type Slots = usize;
    const WIDTH: usize = 1;
    const PREPARED_SCALAR: bool = true;
    fn prepare(_: &Self::Value, capacity: &mut Capacity, _: &mut Layouts) -> NodeResult {
        capacity.native();
        Ok(())
    }
    fn install(columns: &mut ValueColumns, value: Self::Value, _: &mut Layouts) -> usize {
        columns.natives.insert(value.data)
    }
    fn release(columns: &mut ValueColumns, slot: usize) {
        columns.natives.release(slot);
    }
    fn flatten(slot: usize, layout: &mut [usize]) {
        layout[0] = slot;
    }
    fn schema() -> OrdinaryType {
        OrdinaryType::Native(T::CONTRACT.canonical)
    }
    fn slots(layout: &mut &[usize]) -> usize {
        let slot = layout[0];
        *layout = &layout[1..];
        slot
    }
    fn retain(value: &Self::Value) -> NodeResult<Self::Value> {
        value.retain()
    }
    fn read(columns: &ValueColumns, slot: usize) -> NodeResult<Self::Value> {
        retain::<T>(columns.natives.get(slot))
    }
    fn commit(columns: &mut ValueColumns, slot: usize, value: Self::Value, _: &mut Layouts) {
        *columns.natives.get_mut(slot) = value.data;
    }
}
impl<T: NativeType> PreparedValue for Native<T> {
    type Bounds = usize;
    fn include(bounds: &mut usize, value: &Self::Value) {
        *bounds = (*bounds).max(value.data.len());
    }
    fn allocate(columns: &mut ValueColumns, bounds: &usize) -> NodeResult<ValueSlot<Self>> {
        let mut data = Vec::new();
        data.try_reserve(*bounds)
            .map_err(|e| NodeError::new(e.to_string()))?;
        let mut capacity = Capacity::default();
        capacity.native();
        columns.reserve(&capacity)?;
        Ok(ValueSlot::from_fields(columns.natives.insert(data)))
    }
    fn check_native(
        columns: &ValueColumns,
        to: ValueSlot<Self>,
        value: &Self::Value,
    ) -> NodeResult {
        T::preflight(&value.data, columns.natives.get(to.fields()).capacity())
    }
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult {
        T::preflight(
            source.natives.get(from.fields()),
            destination.natives.get(to.fields()).capacity(),
        )
    }
    fn copy_native(columns: &mut ValueColumns, to: ValueSlot<Self>, value: &Self::Value) {
        T::copy(&value.data, columns.natives.get_mut(to.fields()));
    }
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    ) {
        T::copy(
            source.natives.get(from.fields()),
            destination.natives.get_mut(to.fields()),
        );
    }
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>) {
        columns
            .natives
            .copy(from.fields(), to.fields(), |source, destination| {
                T::copy(source, destination);
            });
    }
}
