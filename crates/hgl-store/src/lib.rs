//! Where time-series live and how a tick travels.
//!
//! The store owns every value, every last-modified time and every binding of
//! a run. It knows nothing about nodes beyond an id to wake.
//!
//! The layout is struct of arrays. Everything is addressed by a dense `u32`
//! index and nothing by pointer; an index survives its vector growing, so
//! plain `Vec`s are enough and the crate is safe Rust.
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

/// Names one output of a [`Store`]: its position in the output table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputId(pub u32);

/// Names one input of a [`Store`]: its position in the input table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputId(pub u32);

/// A node's handle to its own `TS<T>` output. Eight bytes.
#[derive(Debug, Clone, Copy)]
pub struct Out<T: Scalar> {
    id: OutputId,
    /// Settled when the output is added, so that a write looks nothing up.
    slot: u32,
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

/// Who is told that a node must be evaluated in this cycle. The kernel's
/// schedule implements it. Generic, not `dyn`: a wake is inlined. Must be
/// idempotent: a node with two active inputs that both tick is woken twice.
/// (An output notifies once per cycle, TS-6; that a node so woken is
/// evaluated once, GRF-16, is the schedule's to keep.)
pub trait Wake {
    /// `node` has an active input whose source has just ticked.
    fn wake(&mut self, node: NodeId);
}

/// Why [`Store::bind`] refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindError {
    /// The store has no such input.
    UnknownInput(InputId),
    /// The store has no such output.
    UnknownOutput(OutputId),
    /// An input reads only an output of its own scalar type.
    TypeMismatch {
        /// What the input reads.
        input: ScalarType,
        /// What the output holds.
        output: ScalarType,
    },
    /// The input must be unbound before it is bound again.
    AlreadyBound(InputId),
}

/// One entry per output, indexed by [`OutputId`].
#[derive(Debug, Default)]
struct Outputs {
    /// `NEVER` until the first tick. Valid and modified are both read from
    /// this and from nothing else (TS-1), and so is whether the output has
    /// already notified in this cycle (TS-6).
    modified_at: Vec<EngineTime>,
    slot: Vec<u32>,
    scalar_type: Vec<ScalarType>,
    owner: Vec<NodeId>,
    /// Every input bound to the output, active or passive.
    watchers: Vec<Vec<InputId>>,
}

/// One entry per input, indexed by [`InputId`].
#[derive(Debug, Default)]
struct Inputs {
    /// `None` while unbound. Eight bytes an entry, where a reserved id would
    /// take four; the plainer form stays until a benchmark says otherwise.
    source: Vec<Option<OutputId>>,
    /// Repeats `outputs.slot[source]`, so that reading a value is two loads,
    /// the slot and then the value, rather than three.
    source_slot: Vec<u32>,
    scalar_type: Vec<ScalarType>,
    owner: Vec<NodeId>,
    active: Vec<bool>,
}

/// Every time-series of a run: the values, the last-modified times and the
/// bindings.
#[derive(Debug, Default)]
pub struct Store {
    columns: Columns,
    outputs: Outputs,
    inputs: Inputs,
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
    /// A store holding nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// A new output of `owner`, not valid until its first [`Self::set`].
    pub fn add_output<T: Scalar>(&mut self, owner: NodeId) -> Out<T> {
        let id = OutputId(next_index(self.outputs.owner.len()));
        let column = T::column_mut(&mut self.columns);
        let slot = next_index(column.len());
        // Never read before the first `set`: an output that has not ticked
        // has no value (TS-2).
        column.push(T::default());
        self.outputs.modified_at.push(EngineTime::NEVER);
        self.outputs.slot.push(slot);
        self.outputs.scalar_type.push(T::TYPE);
        self.outputs.owner.push(owner);
        self.outputs.watchers.push(Vec::new());
        Out {
            id,
            slot,
            value_type: PhantomData,
        }
    }

    /// A new, unbound input of `owner`. An active input wakes its owner when
    /// it is notified; a passive one reads the same and wakes nobody.
    pub fn add_input<T: Scalar>(&mut self, owner: NodeId, active: bool) -> In<T> {
        let id = InputId(next_index(self.inputs.owner.len()));
        self.inputs.source.push(None);
        self.inputs.source_slot.push(0);
        self.inputs.scalar_type.push(T::TYPE);
        self.inputs.owner.push(owner);
        self.inputs.active.push(active);
        In {
            id,
            value_type: PhantomData,
        }
    }

    /// Make `input` a view of `output`. By id, for the builder, which knows
    /// types only at run time.
    ///
    /// Nothing is notified and nobody is woken (TS-14): the input shows the
    /// output as it is, its last modified time included.
    ///
    /// Errors: either id unknown; the types differ; the input already bound.
    pub fn bind(&mut self, input: InputId, output: OutputId) -> Result<(), BindError> {
        let (input_index, output_index) = (input.0 as usize, output.0 as usize);
        let Some(&input_type) = self.inputs.scalar_type.get(input_index) else {
            return Err(BindError::UnknownInput(input));
        };
        let Some(&output_type) = self.outputs.scalar_type.get(output_index) else {
            return Err(BindError::UnknownOutput(output));
        };
        if input_type != output_type {
            return Err(BindError::TypeMismatch {
                input: input_type,
                output: output_type,
            });
        }
        if self.inputs.source[input_index].is_some() {
            return Err(BindError::AlreadyBound(input));
        }
        self.inputs.source[input_index] = Some(output);
        self.inputs.source_slot[input_index] = self.outputs.slot[output_index];
        self.outputs.watchers[output_index].push(input);
        Ok(())
    }

    /// Leave `input` bound to nothing, and so not valid. Nothing is notified.
    /// Unbinding an unbound input does nothing.
    pub fn unbind(&mut self, input: InputId) {
        if let Some(output) = self.inputs.source[input.0 as usize].take() {
            self.outputs.watchers[output.0 as usize].retain(|&watcher| watcher != input);
        }
    }

    /// The scalar type `input` reads.
    pub fn input_type(&self, input: InputId) -> ScalarType {
        self.inputs.scalar_type[input.0 as usize]
    }

    /// The scalar type `output` holds.
    pub fn output_type(&self, output: OutputId) -> ScalarType {
        self.outputs.scalar_type[output.0 as usize]
    }

    /// The bound output's value. Debug builds assert the input is valid;
    /// release builds trust the caller to have checked [`Self::valid`] (TS-2).
    #[inline]
    pub fn get<T: Scalar>(&self, input: In<T>) -> T {
        let index = input.id.0 as usize;
        debug_assert_eq!(self.inputs.scalar_type[index], T::TYPE, "foreign handle");
        debug_assert!(self.valid(input), "TS-2: not valid, so no value");
        T::column(&self.columns)[self.inputs.source_slot[index] as usize]
    }

    /// Whether the input has a value: it is bound, and to an output that has
    /// ticked (TS-1).
    #[inline]
    pub fn valid<T: Scalar>(&self, input: In<T>) -> bool {
        self.input_valid(input.id)
    }

    /// `valid`, by id: what the kernel asks of a node's required inputs
    /// before it calls `eval` (NOD-2). The same two loads and a compare.
    #[inline]
    pub fn input_valid(&self, input: InputId) -> bool {
        self.source_modified_at(input) != EngineTime::NEVER
    }

    /// Whether the input ticked in the cycle at `now`: it is bound, and to an
    /// output last modified at `now` (TS-1).
    #[inline]
    pub fn modified<T: Scalar>(&self, input: In<T>, now: EngineTime) -> bool {
        // An unbound input reads `NEVER`, which is no cycle's time.
        debug_assert!(now != EngineTime::NEVER, "NEVER is not an evaluation time");
        self.last_modified(input) == now
    }

    /// When the bound output last ticked; `NEVER` if it has not, or if the
    /// input is unbound.
    #[inline]
    pub fn last_modified<T: Scalar>(&self, input: In<T>) -> EngineTime {
        self.source_modified_at(input.id)
    }

    /// The one place a binding is followed, so that valid, modified and last
    /// modified, typed or by id, cannot come to disagree.
    #[inline]
    fn source_modified_at(&self, input: InputId) -> EngineTime {
        match self.inputs.source[input.0 as usize] {
            Some(output) => self.outputs.modified_at[output.0 as usize],
            None => EngineTime::NEVER,
        }
    }

    /// Choose whether notifications wake the input's owner from now on. It
    /// changes nothing the input reads (TS-8, NOD-7).
    #[inline]
    pub fn set_active<T: Scalar>(&mut self, input: In<T>, active: bool) {
        self.inputs.active[input.id.0 as usize] = active;
    }

    /// Write the value. If this is the output's first write at `now`, stamp
    /// it and wake the owner of every active watcher; a later write at `now`
    /// only changes the value (TS-6). `writer` is the node being
    /// evaluated; debug builds assert it owns `output` (TS-21). Release builds
    /// do not read it.
    #[inline]
    pub fn set<T: Scalar, W: Wake>(
        &mut self,
        output: Out<T>,
        value: T,
        now: EngineTime,
        writer: NodeId,
        wake: &mut W,
    ) {
        let index = output.id.0 as usize;
        debug_assert_eq!(self.outputs.scalar_type[index], T::TYPE, "foreign handle");
        debug_assert_eq!(self.outputs.owner[index], writer, "TS-21: not the owner");
        let last = self.outputs.modified_at[index];
        debug_assert!(now != EngineTime::NEVER, "NEVER is not an evaluation time");
        debug_assert!(now >= last, "TS-3: last modified time never decreases");
        T::column_mut(&mut self.columns)[output.slot as usize] = value;
        // Once per cycle is kept on the output, as hgraph's C++ keeps it: a
        // watcher bound or made active since the first write is not woken.
        if last == now {
            return;
        }
        self.outputs.modified_at[index] = now;
        for &watcher in &self.outputs.watchers[index] {
            let watcher = watcher.0 as usize;
            if self.inputs.active[watcher] {
                wake.wake(self.inputs.owner[watcher]);
            }
        }
    }

    /// A node reading its own output (INJ-8): `None` until it has ticked.
    #[inline]
    pub fn output_value<T: Scalar>(&self, output: Out<T>) -> Option<T> {
        let index = output.id.0 as usize;
        debug_assert_eq!(self.outputs.scalar_type[index], T::TYPE, "foreign handle");
        let ticked = self.outputs.modified_at[index] != EngineTime::NEVER;
        if ticked {
            Some(T::column(&self.columns)[output.slot as usize])
        } else {
            None
        }
    }

    /// The erased path, for tests and tools, never a node's eval: the
    /// output's value, `None` until it has ticked.
    pub fn output_value_erased(&self, output: OutputId) -> Option<ScalarValue> {
        let index = output.0 as usize;
        let ticked = self.outputs.modified_at[index] != EngineTime::NEVER;
        let slot = self.outputs.slot[index] as usize;
        if ticked {
            Some(self.columns.value(self.outputs.scalar_type[index], slot))
        } else {
            None
        }
    }

    /// The erased path: whether the output ticked in the cycle at `now`.
    pub fn output_modified(&self, output: OutputId, now: EngineTime) -> bool {
        debug_assert!(now != EngineTime::NEVER, "NEVER is not an evaluation time");
        self.outputs.modified_at[output.0 as usize] == now
    }
}
