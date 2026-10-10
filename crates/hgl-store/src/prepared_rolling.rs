//! Typed arrival publication into independent scalar, atomic and rolling destinations.
use crate::columns::{Column, Scalar};
use crate::global::{GlobalValue, PreparedValue, ValueColumns, ValueSlot};
use crate::observation::Observation;
use crate::scalar_copy::ScalarCopy;
use crate::shapes::{Atomic, Input, Output};
use crate::{PreparedTick, Wake};
use hgl_types::{NodeError, NodeResult};
impl<W: Wake> PreparedTick<'_, W> {
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
        value: &<S::Payload as GlobalValue>::Value,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        self.authorize(output.id(), output.generation());
        self.storage
            .rolling
            .write(self.storage.bindings, output, value, self.now, self.wake)
    }
    /// Copy a valid scalar input into an independently retained arrival.
    pub fn rolling_scalar<S: crate::rolling::WindowShape>(
        &mut self,
        input: crate::In<S::Payload>,
        output: Output<S>,
    ) -> NodeResult
    where
        S::Payload: Scalar + PreparedValue + GlobalValue<Value = S::Payload>,
    {
        self.authorize(output.id(), output.generation());
        let storage = &mut self.storage;
        if !storage.bindings.valid(input.id()) {
            return Err(NodeError::new("prepared input is invalid"));
        }
        let from = storage.bindings.input(input.id()).slot as usize;
        let value = &S::Payload::column(storage.columns)[from];
        storage
            .rolling
            .write(storage.bindings, output, value, self.now, self.wake)
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

    /// Validate octets before replacing an independently reserved rolling arrival.
    pub fn rolling_bytes_from_list<
        S: crate::rolling::WindowShape<Payload = Vec<u8>>,
        const N: i64,
    >(
        &mut self,
        input: Input<Atomic<crate::list::List<i64, N>>>,
        output: Output<S>,
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let storage = &mut self.storage;
        storage.rolling.bytes(
            storage.bindings,
            output,
            (self.now, self.wake),
            |bindings, destination| storage.atomic.copy_bytes(bindings, input, destination),
        )
    }
    /// Publish one independently retained arrival from a prepared source slot.
    pub fn rolling_from<S: crate::rolling::WindowShape>(
        &mut self,
        source: Option<&ValueColumns>,
        from: ValueSlot<S::Payload>,
        output: Output<S>,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        self.authorize(output.id(), output.generation());
        let source = source.unwrap_or_else(|| self.storage.globals.values());
        self.storage.rolling.from(
            self.storage.bindings,
            output,
            source,
            from,
            (self.now, self.wake),
        )
    }

    /// Copy a rolling arrival into independently reserved scalar storage.
    pub fn scalar_from_rolling<S: crate::rolling::WindowShape>(
        &mut self,
        input: Input<S>,
        output: crate::Out<S::Payload>,
    ) -> NodeResult
    where
        S::Payload: Scalar + PreparedValue + GlobalValue<Value = S::Payload, Slots = usize>,
    {
        self.authorize(output.id(), output.generation());
        let storage = &mut self.storage;
        let from = storage.rolling.borrow::<S>(storage.bindings, input)?;
        let value = storage.rolling.values().scalar::<S::Payload>(from.fields());
        let slot = storage.bindings.output(output.id()).slot as usize;
        let destination = &mut S::Payload::column_mut(storage.columns)[slot];
        if destination.capacity() < value.size() {
            return Err(NodeError::new("prepared scalar capacity exceeded"));
        }
        destination.copy_from(value);
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
        Ok(())
    }
    /// Copy an arrival into complete ordinary output storage.
    pub fn atomic_from_rolling<S: crate::rolling::WindowShape>(
        &mut self,
        input: Input<S>,
        output: Output<Atomic<S::Payload>>,
    ) -> NodeResult
    where
        S::Payload: PreparedValue,
    {
        self.authorize(output.id(), output.generation());
        let storage = &mut self.storage;
        let from = storage.rolling.borrow::<S>(storage.bindings, input)?;
        let to = storage.atomic.destination(storage.bindings, output)?;
        S::Payload::check_slots(storage.rolling.values(), from, storage.atomic.values(), to)?;
        S::Payload::copy_between(
            storage.rolling.values(),
            from,
            storage.atomic.values_mut(),
            to,
        );
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
        Ok(())
    }
    /// Forward an arrival between different checked window policies.
    pub fn pass_rolling_as<S, T>(&mut self, input: Input<S>, output: Output<T>) -> NodeResult
    where
        S: crate::rolling::WindowShape,
        T: crate::rolling::WindowShape<Payload = S::Payload>,
        S::Payload: PreparedValue,
    {
        self.authorize(output.id(), output.generation());
        self.storage
            .rolling
            .pass(self.storage.bindings, input, output, self.now, self.wake)
    }
}
