//! Typed publication from existing prepared ordinary storage.
use crate::OutputId;
use crate::columns::Scalar;
use crate::global::{PreparedValue, ValueColumns, ValueSlot};
use crate::shapes::{Atomic, Output};
use crate::{PreparedTick, Wake};
use hgl_types::NodeResult;
impl<W: Wake> PreparedTick<'_, W> {
    /// Validate octets before replacing an independently reserved rolling arrival.
    pub fn rolling_bytes_from_list<
        S: crate::rolling::WindowShape<Payload = Vec<u8>>,
        const N: i64,
    >(
        &mut self,
        input: crate::shapes::Input<Atomic<crate::list::List<i64, N>>>,
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

    /// Publish a typed scalar from an independently prepared ordinary destination.
    pub fn scalar_from_global<T: Scalar>(
        &mut self,
        output: OutputId,
        generation: u32,
        from: usize,
    ) -> NodeResult {
        self.authorize(output, generation);
        let slot = self.storage.bindings.output(output).slot as usize;
        self.storage
            .globals
            .copy_scalar(from, &mut T::column_mut(self.storage.columns)[slot])?;
        self.storage.bindings.publish(output, self.now, self.wake);
        Ok(())
    }
}
impl<W: Wake> PreparedTick<'_, W> {
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
        self.storage
            .bindings
            .publish(output.id(), self.now, self.wake);
        Ok(())
    }
}
