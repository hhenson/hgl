//! Endpoint storage without binding or notification policy.
use hgl_types::{EngineTime, NodeId};
use std::collections::BTreeMap;
/// One output slot in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OutputId(pub u32);
/// One input slot in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputId(pub u32);
/// Recursive shape shared with graph descriptions.
pub use hgl_types::TsType as Kind;
/// A designation; retaining it does not retain its endpoint.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Reference {
    /// Peer designation, absent for empty and items.
    pub output: Option<OutputId>,
    /// Peer or assembly lifetime.
    pub generation: u32,
    /// Interned child designation index.
    pub items: Option<usize>,
}
impl Reference {
    /// Peers use source identity; assembly slots also require their lifetime.
    pub fn same_items(self, other: Self) -> bool {
        self.items == other.items && (self.items.is_none() || self.generation == other.generation)
    }
}
/// A graph lifetime; local node ranks are meaningful only within this scope.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScopeId {
    /// Allocator slot.
    pub index: usize,
    /// Lifetime generation.
    pub generation: u64,
}
/// Keyed membership and current-cycle changes.
#[derive(Debug)]
pub struct Members<I> {
    /// Binding-owned storage bookkeeping.
    pub live: BTreeMap<i64, I>,
    /// Binding-owned storage bookkeeping.
    pub removed: BTreeMap<i64, I>,
    /// Binding-owned storage bookkeeping.
    pub initial: BTreeMap<i64, bool>,
    /// Binding-owned storage bookkeeping.
    pub changed: Vec<i64>,
    /// Binding-owned storage bookkeeping.
    pub epoch: EngineTime,
}
impl<I> Default for Members<I> {
    fn default() -> Self {
        Self {
            live: BTreeMap::new(),
            removed: BTreeMap::new(),
            initial: BTreeMap::new(),
            changed: Vec::new(),
            epoch: EngineTime::NEVER,
        }
    }
}
/// Read-only output metadata, owned by Bindings.
#[expect(
    clippy::struct_excessive_bools,
    reason = "lifetime, retirement and two independent notification observations are orthogonal flags"
)]
#[derive(Debug)]
pub struct Output {
    /// Typed column slot, unused for aggregate and reference outputs.
    pub slot: u32,
    /// Shape fixed when allocated.
    pub kind: Kind,
    /// Local writing node.
    pub owner: NodeId,
    /// Owning graph scope.
    pub scope: ScopeId,
    /// Binding-owned storage bookkeeping.
    pub scope_position: usize,
    /// NEVER while invalid.
    pub modified_at: EngineTime,
    /// Lifetime generation; exhausted generations are never reused.
    pub generation: u32,
    /// Whether this slot still designates an endpoint.
    pub alive: bool,
    /// Binding-owned storage bookkeeping.
    pub notified_at: EngineTime,
    /// Binding-owned storage bookkeeping.
    pub parent: Option<(OutputId, i64)>,
    /// Binding-owned storage bookkeeping.
    pub parent_at: EngineTime,
    /// Binding-owned storage bookkeeping.
    pub watchers: Vec<InputId>,
    /// Binding-owned storage bookkeeping.
    pub followers: Vec<InputId>,
    /// Binding-owned storage bookkeeping.
    pub reference: Reference,
    /// Binding-owned storage bookkeeping.
    pub retired_at: EngineTime,
    /// Binding-owned storage bookkeeping.
    pub retirement_queued: bool,
    /// Binding-owned storage bookkeeping.
    pub members: Members<OutputId>,
    /// Fixed children, allocated once.
    pub fixed: Vec<OutputId>,
    /// Valid immediate fixed children.
    pub valid_children: usize,
    /// Validity last propagated to the owning output.
    pub parent_valid: bool,
    /// Validity at the last notification.
    pub notified_valid: bool,
}
/// Read-only input metadata, owned by Bindings.
#[derive(Debug)]
pub struct Input {
    /// Currently followed endpoint.
    pub source: Option<OutputId>,
    /// Cached scalar slot.
    pub slot: u32,
    /// Declared shape.
    pub kind: Kind,
    /// Binding-owned storage bookkeeping.
    pub owner: NodeId,
    /// Binding-owned storage bookkeeping.
    pub scope: ScopeId,
    /// Binding-owned storage bookkeeping.
    pub scope_position: usize,
    /// Binding-owned storage bookkeeping.
    pub active: bool,
    /// Binding-owned storage bookkeeping.
    pub alive: bool,
    /// Binding-owned storage bookkeeping.
    pub sampled_at: EngineTime,
    /// Binding-owned storage bookkeeping.
    pub reference_source: Option<OutputId>,
    /// Binding-owned storage bookkeeping.
    pub source_position: usize,
    /// Binding-owned storage bookkeeping.
    pub reference_position: usize,
    /// Binding-owned storage bookkeeping.
    pub parent: Option<(InputId, i64)>,
    /// Binding-owned storage bookkeeping.
    pub parent_at: EngineTime,
    /// Binding-owned storage bookkeeping.
    pub members: Members<InputId>,
    /// Binding-owned storage bookkeeping.
    pub withdrawal: EngineTime,
    /// Fixed children, retained through rebinds.
    pub fixed: Vec<InputId>,
    /// Cached immediate child validity count.
    pub valid_children: usize,
    /// Previous validity used to update ancestors incrementally.
    pub valid: bool,
    /// Local event time, including an invalidation in a still-valid parent.
    pub observed_at: EngineTime,
    /// Current binding designation, excluding expiry.
    pub designation: Reference,
    /// Last scheduled cycle.
    pub notified_at: EngineTime,
}
/// Reusable endpoint slots. The runtime keeps this arena private.
#[derive(Debug, Default)]
pub struct Endpoints {
    /// Output slots.
    pub outputs: Vec<Output>,
    /// Input slots.
    pub inputs: Vec<Input>,
    /// Reusable output slots.
    pub free_outputs: Vec<OutputId>,
    /// Reusable input slots.
    pub free_inputs: Vec<InputId>,
}
fn index(n: usize) -> u32 {
    u32::try_from(n).unwrap_or_else(|_| unreachable!("endpoint capacity exceeded"))
}
impl Endpoints {
    /// A current peer, excluding retained removed-member projections (TS-11).
    pub fn has_peer(&self, mut input: InputId) -> bool {
        if self.input(input).source.is_none() {
            return false;
        }
        while let Some((parent, key)) = self.input(input).parent {
            if !self.input(parent).kind.fixed()
                && self.input(parent).members.live.get(&key) != Some(&input)
            {
                return false;
            }
            input = parent;
        }
        true
    }

    /// Output metadata. An unknown id is a caller error.
    pub fn output(&self, id: OutputId) -> &Output {
        &self.outputs[id.0 as usize]
    }
    /// Input metadata. An unknown id is a caller error.
    pub fn input(&self, id: InputId) -> &Input {
        &self.inputs[id.0 as usize]
    }
    /// Endpoint and scope slots, then live subscriptions; diagnostic only.
    pub fn storage_counts(&self, scopes: usize) -> [usize; 4] {
        [
            self.outputs.len(),
            self.inputs.len(),
            scopes,
            self.outputs
                .iter()
                .map(|o| o.watchers.len() + o.followers.len())
                .sum(),
        ]
    }
    /// Allocate or reuse a slot; the bool says whether a scalar column must grow.
    pub fn add_output(
        &mut self,
        owner: NodeId,
        kind: Kind,
        next_slot: u32,
        scope: ScopeId,
    ) -> (OutputId, bool) {
        let reused = self
            .free_outputs
            .iter()
            .position(|&id| self.output(id).kind == kind);
        let id = reused.map_or_else(
            || OutputId(index(self.outputs.len())),
            |n| self.free_outputs.swap_remove(n),
        );
        let (slot, generation) = if reused.is_some() {
            let o = self.output(id);
            (o.slot, o.generation)
        } else {
            (next_slot, 1)
        };
        let output = Output {
            slot,
            kind,
            owner,
            scope,
            scope_position: 0,
            modified_at: EngineTime::NEVER,
            generation,
            alive: true,
            notified_at: EngineTime::NEVER,
            parent: None,
            parent_at: EngineTime::NEVER,
            watchers: Vec::new(),
            followers: Vec::new(),
            reference: Reference::default(),
            retired_at: EngineTime::NEVER,
            retirement_queued: false,
            members: Members::default(),
            fixed: Vec::new(),
            valid_children: 0,
            parent_valid: false,
            notified_valid: false,
        };
        if reused.is_some() {
            self.outputs[id.0 as usize] = output;
        } else {
            self.outputs.push(output);
        }
        (id, reused.is_none())
    }
    /// Allocate an unbound view.
    pub fn add_input(
        &mut self,
        owner: NodeId,
        kind: Kind,
        active: bool,
        scope: ScopeId,
    ) -> InputId {
        let id = self
            .free_inputs
            .pop()
            .unwrap_or(InputId(index(self.inputs.len())));
        let input = Input {
            source: None,
            slot: 0,
            kind,
            owner,
            scope,
            scope_position: 0,
            active,
            alive: true,
            sampled_at: EngineTime::NEVER,
            reference_source: None,
            source_position: 0,
            reference_position: 0,
            parent: None,
            parent_at: EngineTime::NEVER,
            members: Members::default(),
            withdrawal: EngineTime::NEVER,
            fixed: Vec::new(),
            valid_children: 0,
            valid: false,
            observed_at: EngineTime::NEVER,
            designation: Reference::default(),
            notified_at: EngineTime::NEVER,
        };
        if id.0 as usize == self.inputs.len() {
            self.inputs.push(input);
        } else {
            self.inputs[id.0 as usize] = input;
        }
        id
    }
}

/// Who is woken by a publication in the current scope.
pub trait Wake {
    /// Make a local node ready, idempotently.
    fn wake(&mut self, node: NodeId);
}

mod scopes;
pub use scopes::{Phase, Scope, Scopes};

mod assemblies;
pub use assemblies::Assemblies;
