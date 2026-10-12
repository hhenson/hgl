//! Single provider execution followed by checked owning prepared publication.
use crate::bindings::Wake;
use crate::columns::Column;
use crate::native_values::{Native, NativeType, NativeView};
use crate::prepared_store::{Observation, PreparedTick};
use crate::shapes::{Atomic, Output};
use hgl_types::{NodeError, NodeResult};
impl crate::GlobalState {
    /// Retain one borrowed text result in an independent reserved ordinary slot.
    pub fn text_result(
        &mut self,
        observation: Observation<'_>,
        to: crate::ValueSlot<String>,
        execute: impl for<'v> FnOnce(Observation<'v>, &'v Self) -> NodeResult<&'v str>,
    ) -> NodeResult {
        let mut destination = std::mem::take(self.values_mut().scalar_mut::<String>(to.fields()));
        let result = (|| {
            let value = execute(observation, self)?;
            if value.len() > destination.capacity() {
                return Err(NodeError::new("prepared native text capacity exceeded"));
            }
            destination.clear();
            destination.push_str(value);
            Ok(())
        })();
        *self.values_mut().scalar_mut::<String>(to.fields()) = destination;
        result
    }
}
impl<W: Wake> PreparedTick<'_, W> {
    /// Invoke a borrowed-result provider once, retaining only after complete preflight.
    pub fn native_result<T: NativeType, const DOMAIN: usize>(
        &mut self,
        output: Output<Atomic<Native<T, DOMAIN>>>,
        execute: impl for<'v> FnOnce(
            Observation<'v>,
            &'v crate::GlobalState,
        ) -> NodeResult<NativeView<'v, T>>,
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let slot = self
            .storage
            .atomic
            .destination(self.storage.bindings, output)?;
        let mut destination = std::mem::take(
            self.storage
                .atomic
                .values_mut()
                .natives
                .get_mut(slot.fields()),
        );
        let result: NodeResult = (|| {
            let (observation, globals) = self.storage.observations();
            let value = execute(observation, globals)?;
            T::preflight(value.data(), destination.capacity())?;
            destination.clear();
            T::copy(value.data(), &mut destination);
            Ok(())
        })();
        *self
            .storage
            .atomic
            .values_mut()
            .natives
            .get_mut(slot.fields()) = destination;
        result?;
        self.publish(output.id());
        Ok(())
    }
    /// Own a borrowed text result with one helper call and no speculative execution.
    pub fn text_result(
        &mut self,
        output: crate::Out<String>,
        execute: impl for<'v> FnOnce(Observation<'v>, &'v crate::GlobalState) -> NodeResult<&'v str>,
    ) -> NodeResult {
        self.authorize(output.id(), output.generation());
        let slot = self.storage.bindings.output(output.id()).slot as usize;
        let mut destination = std::mem::take(&mut String::column_mut(self.storage.columns)[slot]);
        let result: NodeResult = (|| {
            let (observation, globals) = self.storage.observations();
            let value = execute(observation, globals)?;
            if value.len() > destination.capacity() {
                return Err(NodeError::new("prepared native text capacity exceeded"));
            }
            destination.clear();
            destination.push_str(value);
            Ok(())
        })();
        String::column_mut(self.storage.columns)[slot] = destination;
        result?;
        self.publish(output.id());
        Ok(())
    }
}
