//! Where time-series live and how a tick travels.
//!
//! The store owns every value, every last-modified time and every binding of
//! a run. It knows nothing about nodes beyond an id to wake.
//!
//! Values live in typed columns; endpoint metadata and graph scopes live in
//! the `bindings` module. Indices survive vector growth. References and scalar write
//! handles carry generations so reused child slots cannot revive old endpoints.
//!
//! Fixed scalar propagation borrows typed columns without allocation. Creating
//! text payloads or dynamic collection members may allocate; reading text need
//! not copy it (`Store::get_ref`).
//!
//! An id or a handle from anywhere but this store is a bug in the caller, and
//! is not looked for: one out of range panics, any other names the wrong
//! entry. Only [`Store::bind`] reports an unknown id, because the builder
//! calls it with ids read from a description.

pub use rolling::{Rolling, WindowShape};
use std::marker::PhantomData;

pub use global::{
    Capacity, Global, GlobalState, GlobalValue, Layouts, List, ListBounds, Optional, PreparedValue,
    Recursive, RecursiveTarget, ValueColumns, ValueSlot, append_slot, commit_append, list_index,
    list_index_mut, list_len, list_push,
};
use hgl_types::{EngineTime, NodeId, NodeResult, ScalarType, ScalarValue};

use bindings::Bindings;
pub use bindings::{BindError, InputId, Kind, OutputId, Reference, ScopeId, Wake};
pub use columns::{Columns, Scalar};
pub use keys::{Key, Keys};
pub use prepared_store::{Observation, PreparedStorage, PreparedTick};
pub mod atomic;
mod atomic_publication;
pub mod binding_build;
pub mod bindings;
pub mod columns;
pub mod endpoints;
mod fixed;
pub mod global;
pub mod global_arena;
pub mod global_value;
pub mod keys;
pub mod list;
pub mod list_storage;
pub mod member_table;
pub mod observation;
pub mod optional;
pub mod prepared_store;
pub mod prepared_value;
pub mod recursive_value;
pub mod rolling;
pub mod scalar_copy;
pub mod shapes;
pub mod store_build;
pub mod value_lists;

/// A node's handle to its own `TS<T>` output. Eight bytes.
#[derive(Debug, Clone)]
pub struct Out<T: Scalar> {
    id: OutputId,
    /// Keeps a retained writing handle from addressing a reused child slot.
    generation: u32,
    /// Zero-sized: it carries `T` at compile time only, as a tag template
    /// parameter does in C++.
    value_type: PhantomData<T>,
}

/// A node's handle to one of its `TS<T>` inputs. Four bytes.
#[derive(Debug, Clone)]
pub struct In<T: Scalar> {
    id: InputId,
    /// Zero-sized, as in [`Out`].
    value_type: PhantomData<T>,
}

impl<T: Scalar> Out<T> {
    /// The original endpoint generation for prepared publication validation.
    pub fn generation(self) -> u32 {
        self.generation
    }

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
    /// Exact cold key domains retained for this run.
    pub keys: Keys,
    columns: Columns,
    bindings: Bindings,
    globals: GlobalState,
    atomic: atomic::Arena,
    /// Independently prepared ordinary arrival windows.
    pub rolling: rolling::Arena,
}

/// A dictionary with i64 keys and scalar children.
#[derive(Debug, Clone)]
pub struct DictOut<T: Scalar> {
    id: OutputId,
    value_type: PhantomData<T>,
}
/// An input view of a dictionary, including its membership delta.
#[derive(Debug, Clone)]
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

impl Store {
    /// Borrow disjoint prepared runtime storage for cold setup or typed hook copying.
    pub fn prepared(&mut self) -> PreparedStorage<'_> {
        PreparedStorage {
            columns: &mut self.columns,
            bindings: &mut self.bindings,
            globals: &mut self.globals,
            atomic: &mut self.atomic,
            rolling: &mut self.rolling,
            keys: &self.keys,
        }
    }
    /// Access the run-owned ordinary capability, independently of temporal storage.
    pub fn global_state(&mut self) -> &mut GlobalState {
        &mut self.globals
    }
    /// An empty run.
    pub fn new() -> Self {
        Self::default()
    }
    /// Allocate a scalar output in the current graph scope.
    pub fn add_output<T: Scalar>(&mut self, owner: NodeId) -> Out<T> {
        let id = store_build::scalar::<T>(&mut self.bindings, &mut self.columns, owner);
        self.scalar_output(id)
            .unwrap_or_else(|_| unreachable!("new scalar output"))
    }
    /// Allocate an unbound scalar input.
    pub fn add_input<T: Scalar>(&mut self, owner: NodeId, active: bool) -> In<T> {
        let id = self.bindings.add_input(owner, Kind::Ts(T::TYPE), active);
        self.scalar_input(id)
            .unwrap_or_else(|_| unreachable!("new scalar input"))
    }
    /// Plain, silent wiring-time binding.
    pub fn bind(&mut self, input: InputId, output: OutputId) -> Result<(), BindError> {
        self.bindings.bind(input, output)
    }
    /// Capture a source's identity for a reference parameter, independent of value validity.
    pub fn bind_designation(&mut self, input: InputId, output: OutputId) -> Result<(), BindError> {
        self.bindings.bind_designation(input, output)
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
        self.get_ref(input).clone()
    }
    /// Borrow the current scalar payload without copying it.
    pub fn get_ref<T: Scalar>(&self, input: In<T>) -> &T {
        debug_assert!(self.valid(input), "TS-2: invalid input");
        &T::column(&self.columns)[self.bindings.input(input.id).slot as usize]
    }
    /// Whether the input has a value.
    #[inline]
    pub fn valid<T: Scalar>(&self, input: In<T>) -> bool {
        self.input_valid(input.id)
    }
    /// Admission by id.
    #[inline]
    pub fn input_valid(&self, input: InputId) -> bool {
        self.bindings.valid(input)
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
        debug_assert_eq!(o.kind, Kind::Ts(T::TYPE), "foreign handle");
        prepared_store::validate_write(&self.bindings, output.id, output.generation, now, writer);
        T::column_mut(&mut self.columns)[o.slot as usize] = value;
        self.bindings.publish(output.id, now, wake);
    }
    /// Read an output without mistaking its uninitialized slot for a value.
    #[inline]
    pub fn output_value<T: Scalar>(&self, output: Out<T>) -> Option<T> {
        self.output_ref(output).cloned()
    }
    /// Borrow a live, valid output payload, checking its handle generation.
    pub fn output_ref<T: Scalar>(&self, output: Out<T>) -> Option<&T> {
        let o = self.bindings.output(output.id);
        debug_assert_eq!(o.kind, Kind::Ts(T::TYPE), "foreign handle");
        (o.alive && o.generation == output.generation && o.modified_at != EngineTime::NEVER)
            .then(|| &T::column(&self.columns)[o.slot as usize])
    }
    /// Erased scalar observation; aggregate and REF endpoints have no scalar value.
    pub fn output_value_erased(&self, output: OutputId) -> Option<ScalarValue> {
        let o = self.bindings.output(output);
        (matches!(o.kind, Kind::Ts(_)) && o.modified_at != EngineTime::NEVER)
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
        self.add_shaped_output(
            owner,
            Kind::Reference(Box::new(if dictionary {
                Kind::Dictionary(Box::new(Kind::Ts(scalar)))
            } else {
                Kind::Ts(scalar)
            })),
        )
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
            id: self.add_shaped_output(owner, Kind::Dictionary(Box::new(Kind::Ts(T::TYPE)))),
            value_type: PhantomData,
        }
    }
    /// Create a dictionary input.
    pub fn add_dictionary_input<T: Scalar>(&mut self, owner: NodeId, active: bool) -> DictIn<T> {
        DictIn {
            id: self.add_shaped_input(owner, Kind::Dictionary(Box::new(Kind::Ts(T::TYPE))), active),
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
        let id = self.get_or_create_shaped(dict.id, key, now, wake);
        self.scalar_output(id)
            .unwrap_or_else(|_| unreachable!("typed dictionary"))
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

impl<T: Scalar> Copy for Out<T> {}

impl<T: Scalar> Copy for In<T> {}

impl<T: Scalar> Copy for DictOut<T> {}

impl<T: Scalar> Copy for DictIn<T> {}
