//! Typed publication from existing prepared ordinary storage.
use crate::OutputId;
use crate::columns::Scalar;
use crate::{PreparedTick, Wake};
use hgl_types::NodeResult;
impl<W: Wake> PreparedTick<'_, W> {
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
