//! Independent ordinary storage prepared for a finite evaluation horizon.
use hgl_global_value::{GlobalValue, ValueColumns, ValueSlot};
use hgl_types::NodeResult;
mod scalars;
/// Whole-value preflight separates failure from infallible capacity-preserving copying.
pub trait PreparedValue: GlobalValue + Sized {
    /// Cold maxima needed by every independently owned destination.
    type Bounds: Default;
    /// Merge a native value into the cold capacity bounds.
    fn include(bounds: &mut Self::Bounds, value: &Self::Value);
    /// Allocate all descendants, leaving publication to the owning root.
    fn allocate(columns: &mut ValueColumns, bounds: &Self::Bounds) -> NodeResult<ValueSlot<Self>>;
    /// Check every descendant before changing any live field.
    fn check_native(
        columns: &ValueColumns,
        destination: ValueSlot<Self>,
        value: &Self::Value,
    ) -> NodeResult;
    /// Check the complete source against an independent destination.
    fn check_slots(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &ValueColumns,
        to: ValueSlot<Self>,
    ) -> NodeResult;
    /// Copy a native value after successful whole-value preflight.
    fn copy_native(columns: &mut ValueColumns, destination: ValueSlot<Self>, value: &Self::Value);
    /// Copy between independent arenas after successful whole-value preflight.
    fn copy_between(
        source: &ValueColumns,
        from: ValueSlot<Self>,
        destination: &mut ValueColumns,
        to: ValueSlot<Self>,
    );
    /// Copy within one arena after successful whole-value preflight; identical roots are allowed.
    fn copy_within(columns: &mut ValueColumns, from: ValueSlot<Self>, to: ValueSlot<Self>);
}
