//! Construction-only passive projections of endpoints owned by a composite node.
use crate::{BindError, InputId, OutputId, Reference, Store, Wake};
use hgl_types::{EngineTime, NodeId};
struct Quiet;
impl Wake for Quiet {
    fn wake(&mut self, _: NodeId) {}
}
impl Store {
    /// Bind a passive own-node/descendant view before its graph starts.
    /// Consumer designation order checks remain unchanged.
    pub fn prepare_owned_view(
        &mut self,
        input: InputId,
        output: OutputId,
    ) -> Result<(), BindError> {
        self.bindings.prepare_owned_view(input, output)?;
        if self.bindings.input(input).kind == self.bindings.output(output).kind {
            self.bind(input, output)
        } else {
            self.follow(input, output, EngineTime::NEVER, &mut Quiet)
        }
    }
    /// Intern the currently prepared fixed child designations in a passive owner view.
    /// Ordinary fields snapshot targets; REF fields retain the live carrier itself.
    pub fn assemble_owned_view(&mut self, input: InputId) -> Result<Reference, BindError> {
        self.bindings.mark_owned_view(input)?;
        self.assemble_owned(input)
    }
    fn assemble_owned(&mut self, input: InputId) -> Result<Reference, BindError> {
        let view = self.bindings.input(input);
        if let Some(source) = view.source {
            return Ok(self.reference(source));
        }
        if !view.kind.fixed() {
            return Ok(self.bindings.input_reference(input));
        }
        let kind = view.kind.clone();
        let mut children = Vec::with_capacity(kind.len());
        for position in 0..kind.len() {
            children.push(self.assemble_owned(self.bindings.fixed_input(input, position))?);
        }
        let reference = self.items_reference(kind, children)?;
        self.sample(input, reference, EngineTime::NEVER, &mut Quiet)?;
        Ok(reference)
    }
}
