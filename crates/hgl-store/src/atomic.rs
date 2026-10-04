//! Typed ordinary publication uses the existing endpoint lifetime and notification path.
use crate::{GlobalValue, NodeResult, OutputId, Store, ValueColumns, ValueSlot, Wake};
use hgl_shapes::{Atomic, Input, Output};
use hgl_types::{EngineTime, NodeId};

impl Store {
    /// Prepare an exact ordinary endpoint without inventing an initial value.
    pub fn add_atomic_output<T: GlobalValue>(&mut self, owner: NodeId) -> OutputId {
        self.atomic
            .add_output(&mut self.bindings, owner, T::schema())
    }
    /// Borrow prepared ordinary fields only while the input is valid.
    pub fn atomic_borrow<T: GlobalValue>(
        &self,
        input: Input<Atomic<T>>,
    ) -> NodeResult<ValueSlot<T>> {
        self.atomic.borrow(&self.bindings, input)
    }
    /// Retain an independently owned ordinary snapshot.
    pub fn atomic_get<T: GlobalValue>(&self, input: Input<Atomic<T>>) -> NodeResult<T::Value> {
        self.atomic_borrow(input)?.read(self.atomic.values())
    }
    /// Read-only ordinary storage for statically prepared borrowed projections.
    pub fn atomic_values(&self) -> &ValueColumns {
        self.atomic.values()
    }
    /// Replace the whole payload before notifying temporal bindings.
    /// # Panics
    /// A writing handle kept after its endpoint expires is a caller error.
    pub fn set_atomic<T: GlobalValue, W: Wake>(
        &mut self,
        output: Output<Atomic<T>>,
        value: T::Value,
        now: EngineTime,
        wake: &mut W,
    ) -> NodeResult {
        self.atomic
            .write(&mut self.bindings, output, value, now, wake)
    }
}
