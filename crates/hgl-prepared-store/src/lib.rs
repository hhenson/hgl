//! Disjoint borrowed runtime access for independently prepared finite evaluation.
use hgl_bindings::{Bindings, InputId, OutputId, Wake};
use hgl_columns::{Columns, Scalar};
use hgl_global::{GlobalState, PreparedValue, ValueColumns, ValueSlot};
use hgl_shapes::{Atomic, Input, Output};
use hgl_types::{EngineTime, NodeError, NodeId, NodeResult};
/// Read-only temporal sources can coexist with independently writable global records.
#[derive(Debug, Clone, Copy)]
pub struct Observation<'a> {
    /// Scalar source columns selected by generated types.
    pub columns: &'a Columns,
    /// Temporal validity, deltas and prepared projections.
    pub bindings: &'a Bindings,
    /// Whole-value atomic source columns.
    pub atomic: &'a hgl_atomic::Arena,
    /// Exact retained scalar key identities.
    pub keys: &'a hgl_keys::Keys,
}
impl Observation<'_> {
    /// Borrow a valid scalar input without constructing an owning value.
    pub fn scalar<T: Scalar>(&self, input: InputId) -> NodeResult<&T> {
        if !self.bindings.valid(input) {
            return Err(NodeError::new("prepared input is invalid"));
        }
        Ok(&T::column(self.columns)[self.bindings.input(input).slot as usize])
    }
}
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
    pub atomic: &'a mut hgl_atomic::Arena,
    /// Cold exact owning keys.
    pub keys: &'a hgl_keys::Keys,
}
impl<'a> PreparedStorage<'a> {
    /// Borrow temporal sources and global destinations simultaneously.
    pub fn observations(&mut self) -> (Observation<'_>, &mut GlobalState) {
        (
            Observation {
                columns: self.columns,
                bindings: self.bindings,
                atomic: self.atomic,
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
    now: EngineTime,
    writer: NodeId,
    wake: &'a mut W,
}
impl<W: Wake> PreparedTick<'_, W> {
    /// Copy a checked native scalar into reserved owning capacity, then publish.
    pub fn scalar<T: Scalar>(
        &mut self,
        output: OutputId,
        generation: u32,
        value: &T,
    ) -> NodeResult {
        validate_write(
            self.storage.bindings,
            output,
            generation,
            self.now,
            self.writer,
        );
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
        validate_write(
            self.storage.bindings,
            output,
            generation,
            self.now,
            self.writer,
        );
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
        validate_write(
            self.storage.bindings,
            output,
            generation,
            self.now,
            self.writer,
        );
        let observation = Observation {
            columns: self.storage.columns,
            bindings: self.storage.bindings,
            atomic: self.storage.atomic,
            keys: self.storage.keys,
        };
        let bytes = measure(observation)?;
        let slot = self.storage.bindings.output(output).slot as usize;
        if scalar_values::<String>(self.storage.columns)[slot].capacity() < bytes {
            return Err(NodeError::new("prepared text capacity exceeded"));
        }
        let mut destination =
            std::mem::take(&mut scalar_values::<String>(self.storage.columns)[slot]);
        destination.clear();
        let observation = Observation {
            columns: self.storage.columns,
            bindings: self.storage.bindings,
            atomic: self.storage.atomic,
            keys: self.storage.keys,
        };
        compose(&mut destination, observation);
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
        validate_write(
            self.storage.bindings,
            output.id(),
            output.generation(),
            self.now,
            self.writer,
        );
        let slot = self
            .storage
            .atomic
            .destination(self.storage.bindings, output)?;
        T::check_native(self.storage.atomic.values(), slot, value)?;
        T::copy_native(self.storage.atomic.values_mut(), slot, value);
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
        Ok(())
    }
    /// Pass an atomic publication directly between independently prepared slots.
    pub fn pass_atomic<T: PreparedValue>(
        &mut self,
        input: Input<Atomic<T>>,
        output: Output<Atomic<T>>,
    ) -> NodeResult {
        validate_write(
            self.storage.bindings,
            output.id(),
            output.generation(),
            self.now,
            self.writer,
        );
        let from = self.storage.atomic.borrow(self.storage.bindings, input)?;
        let to = self
            .storage
            .atomic
            .destination(self.storage.bindings, output)?;
        T::check_slots(
            self.storage.atomic.values(),
            from,
            self.storage.atomic.values(),
            to,
        )?;
        T::copy_within(self.storage.atomic.values_mut(), from, to);
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
        Ok(())
    }
    /// Publish from an independently owned prepared source arena.
    pub fn atomic_from<T: PreparedValue>(
        &mut self,
        source: &ValueColumns,
        from: ValueSlot<T>,
        output: Output<Atomic<T>>,
    ) -> NodeResult {
        validate_write(
            self.storage.bindings,
            output.id(),
            output.generation(),
            self.now,
            self.writer,
        );
        let to = self
            .storage
            .atomic
            .destination(self.storage.bindings, output)?;
        T::check_slots(source, from, self.storage.atomic.values(), to)?;
        T::copy_between(source, from, self.storage.atomic.values_mut(), to);
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
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
