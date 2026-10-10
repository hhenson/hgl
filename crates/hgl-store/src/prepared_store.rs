//! Disjoint borrowed runtime access for independently prepared finite evaluation.
use crate::bindings::{Bindings, InputId, OutputId, Wake};
use crate::columns::{Columns, Scalar};
use crate::global::{GlobalState, PreparedValue, ValueColumns, ValueSlot};
pub use crate::observation::Observation;
use crate::shapes::{Atomic, Input, Output};
use hgl_types::{EngineTime, NodeError, NodeId, NodeResult};
/// Disjoint storage borrowed from one Store, without a secondary owner or alias registry.
#[derive(Debug)]
pub struct PreparedStorage<'a> {
    /// Scalar destinations and sources.
    pub columns: &'a mut Columns,
    /// Existing temporal publication and lifetime policy.
    pub bindings: &'a mut Bindings,
    /// Ordinary run-owned destinations.
    pub globals: &'a mut GlobalState,
    /// Ordinary whole-value temporal storage.
    pub atomic: &'a mut crate::atomic::Arena,
    /// Independent rolling arrival rings.
    pub rolling: &'a mut crate::rolling::Arena,
    /// Cold exact owning keys.
    pub keys: &'a crate::keys::Keys,
}
impl<'a> PreparedStorage<'a> {
    /// Borrow temporal sources and global destinations simultaneously.
    pub fn observations(&mut self) -> (Observation<'_>, &mut GlobalState) {
        (
            Observation {
                columns: self.columns,
                bindings: self.bindings,
                atomic: self.atomic,
                rolling: self.rolling,
                keys: self.keys,
            },
            self.globals,
        )
    }
    /// Reserve scalar payload capacity before the first publication.
    pub fn prepare_scalar<T: Scalar>(&mut self, output: OutputId, bytes: usize) -> NodeResult {
        T::column_mut(self.columns)[self.bindings.output(output).slot as usize].reserve(bytes)
    }
    /// Bind publication authority for the current evaluation hook.
    pub fn tick<W: Wake>(
        self,
        now: EngineTime,
        writer: NodeId,
        wake: &'a mut W,
    ) -> PreparedTick<'a, W> {
        PreparedTick {
            storage: self,
            now,
            writer,
            wake,
        }
    }
}
/// Prepared hook publication; all complete-value checks precede destination mutation.
#[derive(Debug)]
pub struct PreparedTick<'a, W: Wake> {
    /// Disjoint source and destination access for generated fieldwise recording.
    pub storage: PreparedStorage<'a>,
    pub(crate) now: EngineTime,
    writer: NodeId,
    pub(crate) wake: &'a mut W,
}
impl<W: Wake> PreparedTick<'_, W> {
    pub(crate) fn authorize(&self, output: OutputId, generation: u32) {
        validate_write(
            self.storage.bindings,
            output,
            generation,
            self.now,
            self.writer,
        );
    }
    /// Establish sparse parent validity without initializing any child.
    pub fn initialize_sparse(&mut self, output: OutputId, generation: u32) {
        self.authorize(output, generation);
        if self.storage.bindings.output(output).modified_at == EngineTime::NEVER {
            self.storage.bindings.publish(output, self.now, self.wake);
        }
    }
    /// Publish one independently retained arrival from a prepared source slot.
    pub fn rolling_from<S: crate::rolling::WindowShape>(
        &mut self,
        source: &ValueColumns,
        from: ValueSlot<S::Payload>,
        output: Output<S>,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        self.authorize(output.id(), output.generation());
        self.storage.rolling.from(
            self.storage.bindings,
            output,
            source,
            from,
            (self.now, self.wake),
        )
    }
    /// Compose a complete text arrival into its prepared ring destination.
    pub fn rolling_text<S: crate::rolling::WindowShape<Payload = String>>(
        &mut self,
        output: Output<S>,
        measure: impl FnOnce(Observation<'_>) -> NodeResult<usize>,
        compose: impl FnOnce(&mut String, Observation<'_>),
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let bytes = measure(self.storage.observations().0)?;
        let storage = &mut self.storage;
        storage.rolling.text(
            storage.bindings,
            output,
            bytes,
            (self.now, self.wake),
            |destination, bindings, rolling| {
                compose(
                    destination,
                    Observation {
                        columns: storage.columns,
                        bindings,
                        atomic: storage.atomic,
                        rolling,
                        keys: storage.keys,
                    },
                );
            },
        )
    }
    /// Publish one native arrival while reusing its output's reserved ring storage.
    pub fn rolling<S: crate::rolling::WindowShape>(
        &mut self,
        output: Output<S>,
        value: &<S::Payload as crate::global::GlobalValue>::Value,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        self.authorize(output.id(), output.generation());
        self.storage
            .rolling
            .write(self.storage.bindings, output, value, self.now, self.wake)
    }
    /// Forward the current arrival into an independently timed output window.
    pub fn pass_rolling<S: crate::rolling::WindowShape>(
        &mut self,
        input: Input<S>,
        output: Output<S>,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        self.authorize(output.id(), output.generation());
        self.storage
            .rolling
            .pass(self.storage.bindings, input, output, self.now, self.wake)
    }

    /// Copy a checked native scalar into reserved owning capacity, then publish.
    pub fn scalar<T: Scalar>(
        &mut self,
        output: OutputId,
        generation: u32,
        value: &T,
    ) -> NodeResult {
        self.authorize(output, generation);
        let slot = self.storage.bindings.output(output).slot as usize;
        let destination = &mut T::column_mut(self.storage.columns)[slot];
        if destination.capacity() < value.size() {
            return Err(NodeError::new("prepared scalar capacity exceeded"));
        }
        destination.copy_from(value);
        self.storage.bindings.publish(output, self.now, self.wake);
        Ok(())
    }
    /// Copy a scalar source directly into an independent reserved destination.
    pub fn pass_scalar<T: Scalar>(
        &mut self,
        input: InputId,
        output: OutputId,
        generation: u32,
    ) -> NodeResult {
        self.authorize(output, generation);
        if !self.storage.bindings.valid(input) {
            return Err(NodeError::new("prepared input is invalid"));
        }
        let from = self.storage.bindings.input(input).slot as usize;
        let to = self.storage.bindings.output(output).slot as usize;
        let values = T::column_mut(self.storage.columns);
        if values[to].capacity() < values[from].size() {
            return Err(NodeError::new("prepared scalar capacity exceeded"));
        }
        if from < to {
            let (left, right) = values.split_at_mut(to);
            right[0].copy_from(&left[from]);
        } else if from > to {
            let (left, right) = values.split_at_mut(from);
            left[to].copy_from(&right[0]);
        }
        self.storage.bindings.publish(output, self.now, self.wake);
        Ok(())
    }
    /// Compose text directly into its reserved destination after a complete size check.
    /// The generated closures must only read inputs and append the measured bytes.
    pub fn text(
        &mut self,
        output: OutputId,
        generation: u32,
        measure: impl FnOnce(Observation<'_>) -> NodeResult<usize>,
        compose: impl FnOnce(&mut String, Observation<'_>),
    ) -> NodeResult {
        self.authorize(output, generation);
        let bytes = measure(self.storage.observations().0)?;
        let slot = self.storage.bindings.output(output).slot as usize;
        if scalar_values::<String>(self.storage.columns)[slot].capacity() < bytes {
            return Err(NodeError::new("prepared text capacity exceeded"));
        }
        let mut destination =
            std::mem::take(&mut scalar_values::<String>(self.storage.columns)[slot]);
        destination.clear();
        compose(&mut destination, self.storage.observations().0);
        scalar_values::<String>(self.storage.columns)[slot] = destination;
        self.storage.bindings.publish(output, self.now, self.wake);
        Ok(())
    }
    /// Publish one native complete ordinary payload without retaining an intermediate.
    pub fn atomic<T: PreparedValue>(
        &mut self,
        output: Output<Atomic<T>>,
        value: &T::Value,
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let storage = &mut self.storage;
        let slot = storage.atomic.destination(storage.bindings, output)?;
        T::check_native(storage.atomic.values(), slot, value)?;
        T::copy_native(storage.atomic.values_mut(), slot, value);
        storage.bindings.publish(output.id(), self.now, self.wake);
        Ok(())
    }
    /// Pass an atomic publication directly between independently prepared slots.
    pub fn pass_atomic<T: PreparedValue>(
        &mut self,
        input: Input<Atomic<T>>,
        output: Output<Atomic<T>>,
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let storage = &mut self.storage;
        let from = storage.atomic.borrow(storage.bindings, input)?;
        let to = storage.atomic.destination(storage.bindings, output)?;
        T::check_slots(storage.atomic.values(), from, storage.atomic.values(), to)?;
        T::copy_within(storage.atomic.values_mut(), from, to);
        storage.bindings.publish(output.id(), self.now, self.wake);
        Ok(())
    }
    /// Publish from an independently owned prepared source arena.
    pub fn atomic_from<T: PreparedValue>(
        &mut self,
        source: &ValueColumns,
        from: ValueSlot<T>,
        output: Output<Atomic<T>>,
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let storage = &mut self.storage;
        let to = storage.atomic.destination(storage.bindings, output)?;
        T::check_slots(source, from, storage.atomic.values(), to)?;
        T::copy_between(source, from, storage.atomic.values_mut(), to);
        storage.bindings.publish(output.id(), self.now, self.wake);
        Ok(())
    }
}
/// Shared endpoint lifetime, owner, scope and monotone publication validation.
/// # Panics
/// Expired tokens are caller errors; debug builds also enforce hook authority.
pub fn validate_write(
    bindings: &Bindings,
    output: OutputId,
    generation: u32,
    now: EngineTime,
    writer: NodeId,
) {
    let endpoint = bindings.output(output);
    debug_assert_eq!(endpoint.owner, writer, "TS-21: not the owner");
    debug_assert_eq!(endpoint.scope, bindings.scope(), "TS-21: foreign graph");
    debug_assert!(now != EngineTime::NEVER, "NEVER is not an evaluation time");
    debug_assert!(
        now >= endpoint.modified_at,
        "TS-3: last modified time never decreases"
    );
    assert!(
        endpoint.alive && endpoint.generation == generation,
        "TS-23: expired output handle"
    );
}

fn scalar_values<T: Scalar>(columns: &mut Columns) -> &mut Vec<T> {
    T::column_mut(columns)
}
