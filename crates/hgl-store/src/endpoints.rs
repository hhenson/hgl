//! Endpoint storage without binding or notification policy.
use crate::member_table::Table;
use hgl_types::{EngineTime, NodeId};
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
    /// Permanently allocated children for a finite domain.
    pub prepared: Vec<(i64, I)>,
    /// Binding-owned storage bookkeeping.
    pub live: Table<I>,
    /// Binding-owned storage bookkeeping.
    pub removed: Table<I>,
    /// Binding-owned storage bookkeeping.
    pub initial: Table<bool>,
    /// Binding-owned storage bookkeeping.
    pub changed: Vec<i64>,
    /// Binding-owned storage bookkeeping.
    pub epoch: EngineTime,
}
impl<I> Default for Members<I> {
    fn default() -> Self {
        Self {
            prepared: Vec::new(),
            live: Table::default(),
            removed: Table::default(),
            initial: Table::default(),
            changed: Vec::new(),
            epoch: EngineTime::NEVER,
        }
    }
}
impl<I: Copy> Members<I> {
    /// Establish reusable absent slots for all prepared children.
    pub fn prepare(&mut self, mut children: Vec<(i64, I)>) {
        children.sort_unstable_by_key(|&(key, _)| key);
        let keys = children.iter().map(|&(key, _)| key).collect::<Vec<_>>();
        self.live.prepare(&keys);
        self.removed.prepare(&keys);
        self.initial.prepare(&keys);
        self.changed = Vec::with_capacity(keys.len());
        self.prepared = children;
    }
    /// One permanently allocated child, including an absent member.
    pub fn prepared_child(&self, key: i64) -> Option<I> {
        self.live
            .domain_index(key)
            .map(|index| self.prepared[index].1)
    }
    /// Retain children while allowing their primitive keys to arrive during execution.
    pub fn pool(&mut self) {
        self.live.pool();
        self.removed.pool();
        self.initial.pool();
    }
    /// Bind a new runtime key to the corresponding retained child and bookkeeping slots.
    pub fn claim(&mut self, key: i64) -> I {
        let index = self.live.register(key);
        self.removed.register(key);
        self.initial.register(key);
        self.prepared[index].0 = key;
        self.prepared[index].1
    }

    /// Retain first membership and move the child into the live set.
    pub fn insert(&mut self, key: i64, child: I) {
        self.initial
            .insert_initial(key, self.removed.contains_key(key));
        self.removed.remove(key);
        self.live.insert(key, child);
        if !self.live.prepared() {
            self.changed.reserve(self.live.len() + self.removed.len());
        }
    }
    /// Record a removal while retaining its current-cycle child.
    pub fn remove(&mut self, key: i64) -> Option<I> {
        let child = self.live.remove(key)?;
        self.initial.insert_initial(key, true);
        self.removed.insert(key, child);
        Some(child)
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
                && self.input(parent).members.live.get(key) != Some(&input)
            {
                return false;
            }
            input = parent;
        }
        true
    }

    /// Reset a retained finite member after its removal observation cycle.
    pub fn reset_prepared(&mut self, id: OutputId) {
        for n in 0..self.output(id).fixed.len() {
            self.reset_prepared(self.output(id).fixed[n]);
        }
        while let Some((_, child)) = self.outputs[id.0 as usize].members.live.pop_first() {
            self.reset_prepared(child);
        }
        while let Some((_, child)) = self.outputs[id.0 as usize].members.removed.pop_first() {
            self.reset_prepared(child);
        }
        for n in 0..self.output(id).watchers.len() {
            self.reset_input(self.output(id).watchers[n]);
        }
        let output = &mut self.outputs[id.0 as usize];
        output.generation = output
            .generation
            .checked_add(1)
            .unwrap_or_else(|| unreachable!("prepared output generation exhausted"));
        output.modified_at = EngineTime::NEVER;
        output.notified_at = EngineTime::NEVER;
        output.parent_at = EngineTime::NEVER;
        output.valid_children = 0;
        output.parent_valid = false;
        output.notified_valid = false;
        output.retired_at = EngineTime::NEVER;
        output.retirement_queued = false;
        output.members.initial.clear();
        output.members.changed.clear();
    }
    fn reset_input(&mut self, id: InputId) {
        for n in 0..self.input(id).fixed.len() {
            self.reset_input(self.input(id).fixed[n]);
        }
        while let Some((_, child)) = self.inputs[id.0 as usize].members.live.pop_first() {
            self.reset_input(child);
        }
        while let Some((_, child)) = self.inputs[id.0 as usize].members.removed.pop_first() {
            self.reset_input(child);
        }
        let input = &mut self.inputs[id.0 as usize];
        input.valid = false;
        input.valid_children = 0;
        input.sampled_at = EngineTime::NEVER;
        input.observed_at = EngineTime::NEVER;
        input.notified_at = EngineTime::NEVER;
        input.parent_at = EngineTime::NEVER;
        input.members.initial.clear();
        input.members.changed.clear();
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

impl Endpoints {
    /// Resolve or assign a permanently retained child and its prebound projections.
    /// # Panics
    /// Fixed domains must contain the key; bounded pools must have capacity.
    pub fn prepared_child(&mut self, root: OutputId, key: i64) -> Option<OutputId> {
        if let Some(child) = self.output(root).members.prepared_child(key) {
            return Some(child);
        }
        if !self.output(root).members.live.pooled() {
            assert!(
                !self.output(root).members.live.prepared(),
                "key outside prepared collection domain"
            );
            return None;
        }
        let child = self.outputs[root.0 as usize].members.claim(key);
        self.outputs[child.0 as usize].parent = Some((root, key));
        for position in 0..self.output(root).watchers.len() {
            let input = self.output(root).watchers[position];
            let view = self.inputs[input.0 as usize].members.claim(key);
            self.inputs[view.0 as usize].parent = Some((input, key));
        }
        Some(child)
    }
}
