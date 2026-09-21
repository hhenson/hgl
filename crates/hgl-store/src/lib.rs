//! Where time-series live and how a tick travels.
//!
//! The store owns every value, every last-modified time and every binding of
//! a run. It knows nothing about nodes beyond an id to wake.
//!
//! Values live in typed columns; endpoint metadata and graph scopes live in
//! `hgl-bindings`. Indices survive vector growth. References and scalar write
//! handles carry generations so reused child slots cannot revive old endpoints.
//!
//! Instantiation and binding may allocate. A tick never does: everything it
//! needs was settled by then (`docs/explorations/0009-designing-for-speed.md`).
//!
//! An id or a handle from anywhere but this store is a bug in the caller, and
//! is not looked for: one out of range panics, any other names the wrong
//! entry. Only [`Store::bind`] reports an unknown id, because the builder
//! calls it with ids read from a description.

mod columns;

use std::marker::PhantomData;

use hgl_types::{EngineTime, NodeId, ScalarType, ScalarValue};

use columns::Columns;
pub use columns::Scalar;
pub use hgl_bindings::{BindError, InputId, OutputId, Reference, ScopeId, Wake};
use hgl_bindings::{Bindings, Kind};

/// A node's handle to its own `TS<T>` output. Eight bytes.
#[derive(Debug, Clone, Copy)]
pub struct Out<T: Scalar> {
    id: OutputId,
    /// Keeps a retained writing handle from addressing a reused child slot.
    generation: u32,
    /// Zero-sized: it carries `T` at compile time only, as a tag template
    /// parameter does in C++.
    value_type: PhantomData<T>,
}

/// A node's handle to one of its `TS<T>` inputs. Four bytes.
#[derive(Debug, Clone, Copy)]
pub struct In<T: Scalar> {
    id: InputId,
    /// Zero-sized, as in [`Out`].
    value_type: PhantomData<T>,
}

impl<T: Scalar> Out<T> {
    /// The id to bind inputs to.
    #[inline]
    pub fn id(self) -> OutputId {
        self.id
    }
}

impl<T: Scalar> In<T> {
    /// The id to bind to an output.
    #[inline]
    pub fn id(self) -> InputId {
        self.id
    }
}

/// Every value and binding of a run, shared by its graph scopes.
#[derive(Debug, Default)]
pub struct Store {
    columns: Columns,
    bindings: Bindings,
}

/// A dictionary with i64 keys and scalar children.
#[derive(Debug, Clone, Copy)]
pub struct DictOut<T: Scalar> {
    id: OutputId,
    value_type: PhantomData<T>,
}
/// An input view of a dictionary, including its membership delta.
#[derive(Debug, Clone, Copy)]
pub struct DictIn<T: Scalar> {
    id: InputId,
    value_type: PhantomData<T>,
}
impl<T: Scalar> DictOut<T> {
    /// The endpoint's identity.
    pub fn id(self) -> OutputId {
        self.id
    }
}
impl<T: Scalar> DictIn<T> {
    /// The input's identity.
    pub fn id(self) -> InputId {
        self.id
    }
}

/// The index the next entry gets in a table that holds `len` entries.
#[expect(
    clippy::cast_possible_truncation,
    reason = "indices are u32 by design and adding an entry cannot fail; debug builds assert the count fits"
)]
fn next_index(len: usize) -> u32 {
    debug_assert!(u32::try_from(len).is_ok(), "more than u32::MAX entries");
    len as u32
}

impl Store {
    /// An empty run.
    pub fn new() -> Self {
        Self::default()
    }
    /// Allocate a scalar output in the current graph scope.
    pub fn add_output<T: Scalar>(&mut self, owner: NodeId) -> Out<T> {
        let (id, fresh) = self.bindings.add_output(
            owner,
            Kind::Scalar(T::TYPE),
            next_index(T::column(&self.columns).len()),
        );
        if fresh {
            T::column_mut(&mut self.columns).push(T::default());
        }
        Out {
            id,
            generation: self.bindings.output(id).generation,
            value_type: PhantomData,
        }
    }
    /// Allocate an unbound scalar input.
    pub fn add_input<T: Scalar>(&mut self, owner: NodeId, active: bool) -> In<T> {
        In {
            id: self
                .bindings
                .add_input(owner, Kind::Scalar(T::TYPE), active),
            value_type: PhantomData,
        }
    }
    /// Plain, silent wiring-time binding.
    pub fn bind(&mut self, input: InputId, output: OutputId) -> Result<(), BindError> {
        self.bindings.bind(input, output)
    }
    /// Silent teardown; also detaches any designation subscription.
    pub fn unbind(&mut self, input: InputId) {
        self.bindings.unbind(input);
    }
    /// The scalar type a port reads.
    pub fn input_type(&self, input: InputId) -> ScalarType {
        self.bindings.input(input).kind.scalar()
    }
    /// The scalar type a port writes.
    pub fn output_type(&self, output: OutputId) -> ScalarType {
        self.bindings.output(output).kind.scalar()
    }
    /// Read a valid scalar input.
    #[inline]
    pub fn get<T: Scalar>(&self, input: In<T>) -> T {
        debug_assert_eq!(self.input_type(input.id), T::TYPE, "foreign handle");
        debug_assert!(self.valid(input), "TS-2: not valid, so no value");
        T::column(&self.columns)[self.bindings.input(input.id).slot as usize]
    }
    /// Whether the input has a value.
    #[inline]
    pub fn valid<T: Scalar>(&self, input: In<T>) -> bool {
        self.input_valid(input.id)
    }
    /// Admission by id.
    #[inline]
    pub fn input_valid(&self, input: InputId) -> bool {
        self.bindings.last_modified(input) != EngineTime::NEVER
    }
    /// Whether a scalar input ticked in this cycle.
    #[inline]
    pub fn modified<T: Scalar>(&self, input: In<T>, now: EngineTime) -> bool {
        debug_assert!(now != EngineTime::NEVER, "NEVER is not an evaluation time");
        self.bindings.modified(input.id, now)
    }
    /// Last output publication or input sample; NEVER while invalid.
    #[inline]
    pub fn last_modified<T: Scalar>(&self, input: In<T>) -> EngineTime {
        self.bindings.last_modified(input.id)
    }
    /// Passivity changes notification only.
    pub fn set_active<T: Scalar>(&mut self, input: In<T>, active: bool) {
        self.bindings.set_active(input.id, active);
    }
    /// Publish a scalar from its writing node.
    /// # Panics
    /// A writing handle kept after its endpoint expires is a caller error.
    #[inline]
    pub fn set<T: Scalar, W: Wake>(
        &mut self,
        output: Out<T>,
        value: T,
        now: EngineTime,
        writer: NodeId,
        wake: &mut W,
    ) {
        let o = self.bindings.output(output.id);
        debug_assert_eq!(o.kind, Kind::Scalar(T::TYPE), "foreign handle");
        debug_assert_eq!(o.owner, writer, "TS-21: not the owner");
        debug_assert_eq!(o.scope, self.bindings.scope(), "TS-21: foreign graph");
        debug_assert!(now != EngineTime::NEVER, "NEVER is not an evaluation time");
        debug_assert!(
            now >= o.modified_at,
            "TS-3: last modified time never decreases"
        );
        assert!(
            o.alive && o.generation == output.generation,
            "TS-23: expired output handle"
        );
        T::column_mut(&mut self.columns)[o.slot as usize] = value;
        self.bindings.publish(output.id, now, wake);
    }
    /// Read an output without mistaking its uninitialized slot for a value.
    #[inline]
    pub fn output_value<T: Scalar>(&self, output: Out<T>) -> Option<T> {
        let o = self.bindings.output(output.id);
        debug_assert_eq!(o.kind, Kind::Scalar(T::TYPE), "foreign handle");
        (o.alive && o.generation == output.generation && o.modified_at != EngineTime::NEVER)
            .then(|| T::column(&self.columns)[o.slot as usize])
    }
    /// Erased scalar observation; aggregate and REF endpoints have no scalar value.
    pub fn output_value_erased(&self, output: OutputId) -> Option<ScalarValue> {
        let o = self.bindings.output(output);
        (matches!(o.kind, Kind::Scalar(_)) && o.modified_at != EngineTime::NEVER)
            .then(|| self.columns.value(o.kind.scalar(), o.slot as usize))
    }
    /// Whether an output published in this cycle.
    pub fn output_modified(&self, output: OutputId, now: EngineTime) -> bool {
        debug_assert!(now != EngineTime::NEVER, "NEVER is not an evaluation time");
        self.bindings.output(output).modified_at == now
    }
    /// Metadata observations and scoped scheduling, without mutable access.
    pub fn bindings(&self) -> &Bindings {
        &self.bindings
    }
    /// Begin an independent root run, preserving existing output values.
    pub fn start_run(&mut self) {
        self.bindings.start_run();
    }
    /// Advance the engine boundary, expiring removed endpoints.
    pub fn begin_cycle(&mut self, now: EngineTime) {
        self.bindings.begin_cycle(now);
    }
    /// Invalidate an endpoint, notifying its owning collection.
    pub fn invalidate<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W) {
        self.bindings.invalidate(output, now, wake);
    }
    /// A saved, lifetime-checked designation.
    pub fn reference(&self, output: OutputId) -> Reference {
        self.bindings.reference(output)
    }
    /// Sample a designation without changing its producer's timestamp.
    pub fn sample<W: Wake>(
        &mut self,
        input: InputId,
        r: Reference,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        self.bindings.sample(input, r, now, wake)
    }
    /// Allocate a reference output to a scalar or dictionary.
    pub fn add_reference(
        &mut self,
        owner: NodeId,
        scalar: ScalarType,
        dictionary: bool,
    ) -> OutputId {
        self.bindings
            .add_output(owner, Kind::Reference { scalar, dictionary }, 0)
            .0
    }
    /// Follow designation changes and target publications independently.
    pub fn follow<W: Wake>(
        &mut self,
        input: InputId,
        reference: OutputId,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        self.bindings.follow(input, reference, now, wake)
    }
    /// Publish a reference value.
    pub fn set_reference<W: Wake>(
        &mut self,
        output: OutputId,
        r: Reference,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        self.bindings.set_reference(output, r, now, wake)
    }
    /// Create a dictionary output.
    pub fn add_dictionary<T: Scalar>(&mut self, owner: NodeId) -> DictOut<T> {
        DictOut {
            id: self
                .bindings
                .add_output(owner, Kind::Dictionary(T::TYPE), 0)
                .0,
            value_type: PhantomData,
        }
    }
    /// Create a dictionary input.
    pub fn add_dictionary_input<T: Scalar>(&mut self, owner: NodeId, active: bool) -> DictIn<T> {
        DictIn {
            id: self
                .bindings
                .add_input(owner, Kind::Dictionary(T::TYPE), active),
            value_type: PhantomData,
        }
    }
    /// Find a live child input.
    pub fn child<T: Scalar>(&self, input: DictIn<T>, key: i64) -> Option<In<T>> {
        self.bindings.child_input(input.id, key).map(|id| In {
            id,
            value_type: PhantomData,
        })
    }
    /// Find a retained removed child input.
    pub fn removed_child<T: Scalar>(&self, input: DictIn<T>, key: i64) -> Option<In<T>> {
        self.bindings.removed_input(input.id, key).map(|id| In {
            id,
            value_type: PhantomData,
        })
    }
    /// Create or restore an invalid child, without inventing a value tick.
    pub fn get_or_create<T: Scalar, W: Wake>(
        &mut self,
        dict: DictOut<T>,
        key: i64,
        now: EngineTime,
        wake: &mut W,
    ) -> Out<T> {
        self.begin_cycle(now);
        let old = self.bindings.child_output(dict.id, key);
        let id = old
            .or_else(|| self.bindings.removed_output(dict.id, key))
            .unwrap_or_else(|| self.add_output::<T>(self.bindings.output(dict.id).owner).id);
        let out = Out {
            id,
            generation: self.bindings.output(id).generation,
            value_type: PhantomData,
        };
        if old.is_none() {
            let result = self.bindings.insert(dict.id, key, id, now, wake);
            debug_assert!(result.is_ok(), "typed dictionary child");
        }
        out
    }
    /// Attach a child graph's output without copying its value.
    pub fn attach<T: Scalar, W: Wake>(
        &mut self,
        dict: DictOut<T>,
        key: i64,
        child: Out<T>,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        let endpoint = self.bindings.output(child.id);
        if !endpoint.alive || endpoint.generation != child.generation {
            return Err(BindError::UnknownOutput(child.id));
        }
        self.bindings.insert(dict.id, key, child.id, now, wake)
    }
    /// Remove a member, retaining its value through this engine cycle.
    pub fn remove<T: Scalar, W: Wake>(
        &mut self,
        dict: DictOut<T>,
        key: i64,
        now: EngineTime,
        wake: &mut W,
    ) {
        self.bindings.remove(dict.id, key, now, wake);
    }
    /// Current graph scope.
    pub fn scope(&self) -> ScopeId {
        self.bindings.scope()
    }
    /// Enter a scope and return the previous one.
    pub fn enter_scope(&mut self, scope: ScopeId) -> ScopeId {
        self.bindings.enter_scope(scope)
    }
    /// Create a scope owned by a local node.
    pub fn child_scope(&mut self, owner: NodeId) -> ScopeId {
        self.bindings.child_scope(owner)
    }
    /// Allocate mailbox capacity during graph construction.
    pub fn reserve_scope(&mut self, scope: ScopeId, nodes: usize) {
        self.bindings.reserve_scope(scope, nodes);
    }
    /// Retire a stopped child's storage.
    pub fn release_scope<W: Wake>(&mut self, scope: ScopeId, now: EngineTime, wake: &mut W) {
        self.bindings.release_scope(scope, now, wake);
    }
    /// A pending local node from an enclosing or sibling graph.
    pub fn take_wake(&mut self, scope: ScopeId) -> Option<NodeId> {
        self.bindings.take_wake(scope)
    }
    /// A direct child that needs its owner to run it.
    pub fn take_child(&mut self, owner: NodeId) -> Option<ScopeId> {
        self.bindings.take_child(owner)
    }
}
