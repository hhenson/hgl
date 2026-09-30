//! Construction of recursive endpoints; scalar evaluation stays typed.
use crate::{BindError, In, InputId, Kind, Out, OutputId, Reference, Scalar, Store, Wake};
use hgl_types::{EngineTime, NodeId, ScalarType};
use std::marker::PhantomData;
impl Store {
    /// Construct an output and its fixed descendants in the current scope.
    pub fn add_shaped_output(&mut self, owner: NodeId, kind: Kind) -> OutputId {
        if let Kind::Ts(t) = kind {
            return match t {
                ScalarType::Bool => self.add_output::<bool>(owner).id(),
                ScalarType::I64 => self.add_output::<i64>(owner).id(),
                ScalarType::F64 => self.add_output::<f64>(owner).id(),
                ScalarType::Date => self.add_output::<hgl_types::Date>(owner).id(),
                ScalarType::Time => self.add_output::<hgl_types::Time>(owner).id(),
                ScalarType::DateTime => self.add_output::<EngineTime>(owner).id(),
                ScalarType::Duration => self.add_output::<hgl_types::EngineDelta>(owner).id(),
                ScalarType::Text => self.add_output::<String>(owner).id(),
            };
        }
        let id = self.bindings.add_output(owner, kind, 0).0;
        for n in 0..self.bindings.output(id).kind.len() {
            let child =
                self.add_shaped_output(owner, self.bindings.output(id).kind.child(n).clone());
            let result = self.bindings.append_fixed(id, child);
            debug_assert!(result.is_ok());
        }
        id
    }
    /// Construct an unbound view with stable fixed child slots.
    pub fn add_shaped_input(&mut self, owner: NodeId, kind: Kind, active: bool) -> InputId {
        self.bindings.add_input(owner, kind, active)
    }
    /// Project a scalar writing handle once during construction.
    pub fn scalar_output<T: Scalar>(&self, id: OutputId) -> Result<Out<T>, BindError> {
        let o = self.bindings.output(id);
        if o.kind != Kind::Ts(T::TYPE) {
            return Err(BindError::ShapeMismatch);
        }
        Ok(Out {
            id,
            generation: o.generation,
            value_type: PhantomData,
        })
    }
    /// Project a typed scalar read handle once during construction.
    pub fn scalar_input<T: Scalar>(&self, id: InputId) -> Result<In<T>, BindError> {
        if self.bindings.input(id).kind != Kind::Ts(T::TYPE) {
            return Err(BindError::ShapeMismatch);
        }
        Ok(In {
            id,
            value_type: PhantomData,
        })
    }
    /// Intern a reusable assembled designation during wiring.
    pub fn items_reference(
        &mut self,
        kind: Kind,
        children: Vec<Reference>,
    ) -> Result<Reference, BindError> {
        self.bindings.items_reference(kind, children)
    }
    /// Create or restore a dictionary's compound member.
    pub fn get_or_create_shaped<W: Wake>(
        &mut self,
        dict: OutputId,
        key: i64,
        now: EngineTime,
        wake: &mut W,
    ) -> OutputId {
        self.begin_cycle(now);
        if let Some(id) = self.bindings.child_output(dict, key) {
            return id;
        }
        let id = self
            .bindings
            .restorable_output(dict, key)
            .unwrap_or_else(|| {
                let o = self.bindings.output(dict);
                let child = o
                    .kind
                    .member()
                    .unwrap_or_else(|| unreachable!("membership output"))
                    .clone();
                self.add_shaped_output(o.owner, child)
            });
        let result = self.bindings.insert(dict, key, id, now, wake);
        debug_assert!(result.is_ok());
        id
    }
    /// Attach a child graph's compound output without copying values.
    pub fn attach_shaped<W: Wake>(
        &mut self,
        dict: OutputId,
        key: i64,
        child: Reference,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        let child = self
            .bindings
            .resolve(child)
            .ok_or(BindError::InvalidReference)?;
        self.bindings.insert(dict, key, child, now, wake)
    }
    /// Remove a compound dictionary member for next-cycle retirement.
    pub fn remove_shaped<W: Wake>(
        &mut self,
        dict: OutputId,
        key: i64,
        now: EngineTime,
        wake: &mut W,
    ) {
        self.bindings.remove(dict, key, now, wake);
    }
}
