//! Graph-local ranks and notifications crossing their ownership boundaries.
use crate::{Bindings, InputId, OutputId, Wake};
use hgl_types::{EngineTime, NodeId};

/// A graph lifetime; local node ranks are meaningful only within this scope.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ScopeId {
    index: usize,
    generation: u64,
}
#[derive(Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Constructing,
    Running,
    Releasing,
    Dead,
}
#[derive(Debug, Default)]
struct Scope {
    generation: u64,
    phase: Phase,
    parent: Option<(ScopeId, NodeId)>,
    pending: Vec<NodeId>,
    queued: Vec<bool>,
    children: Vec<Vec<ScopeId>>,
    child_count: Vec<usize>,
    enqueued: bool,
    inputs: Vec<InputId>,
    outputs: Vec<OutputId>,
}
#[derive(Debug)]
pub(crate) struct Scopes {
    entries: Vec<Scope>,
    free: Vec<usize>,
    pub(crate) current: ScopeId,
}
impl Default for Scopes {
    fn default() -> Self {
        Self {
            entries: vec![Scope {
                phase: Phase::Running,
                ..Scope::default()
            }],
            free: Vec::new(),
            current: ScopeId::default(),
        }
    }
}
impl Scopes {
    fn alive(&self, id: ScopeId) -> bool {
        self.entries
            .get(id.index)
            .is_some_and(|s| s.phase != Phase::Dead && s.generation == id.generation)
    }
    pub(crate) fn reserve(&mut self, id: ScopeId, node: NodeId) {
        let s = &mut self.entries[id.index];
        let count = node.0 as usize + 1;
        if count > s.queued.len() {
            s.queued.resize(count, false);
            s.children.resize_with(count, Vec::new);
            s.child_count.resize(count, 0);
            s.pending.reserve(count - s.pending.len());
        }
    }
    pub(crate) fn record_output(&mut self, id: OutputId) -> usize {
        let scope = &mut self.entries[self.current.index];
        let position = scope.outputs.len();
        if self.current.index != 0 {
            scope.outputs.push(id);
        }
        position
    }
    pub(crate) fn record_input(&mut self, id: InputId) -> usize {
        let scope = &mut self.entries[self.current.index];
        let position = scope.inputs.len();
        if self.current.index != 0 {
            scope.inputs.push(id);
        }
        position
    }
    pub(crate) fn wake<W: Wake>(&mut self, mut scope: ScopeId, mut node: NodeId, wake: &mut W) {
        loop {
            if !self.alive(scope) {
                return;
            }
            if scope == self.current && self.entries[scope.index].phase == Phase::Running {
                wake.wake(node);
                return;
            }
            let s = &mut self.entries[scope.index];
            if !s.queued[node.0 as usize] {
                s.queued[node.0 as usize] = true;
                s.pending.push(node);
            }
            if scope == self.current {
                return;
            }
            let Some((parent, owner)) = s.parent else {
                return;
            };
            if !s.enqueued {
                s.enqueued = true;
                self.entries[parent.index].children[owner.0 as usize].push(scope);
            }
            scope = parent;
            node = owner;
        }
    }
}
impl Bindings {
    pub(crate) fn owns_child(&self, dict: OutputId, child: OutputId) -> bool {
        let parent = self.output(dict);
        let endpoint = self.output(child);
        let (mut scope, mut owner) = (endpoint.scope, endpoint.owner);
        while scope != parent.scope {
            let Some((next, node)) = self.scopes.entries[scope.index].parent else {
                return false;
            };
            scope = next;
            owner = node;
        }
        owner == parent.owner
    }
    pub(crate) fn forward(
        &self,
        mut out_scope: ScopeId,
        mut out: NodeId,
        mut in_scope: ScopeId,
        mut input: NodeId,
    ) -> bool {
        let depth = |mut scope: ScopeId| {
            let mut n = 0;
            while let Some((p, _)) = self.scopes.entries[scope.index].parent {
                n += 1;
                scope = p;
            }
            n
        };
        let (mut a, mut b) = (depth(out_scope), depth(in_scope));
        while out_scope != in_scope {
            if a >= b {
                let Some((p, n)) = self.scopes.entries[out_scope.index].parent else {
                    return false;
                };
                out_scope = p;
                out = n;
                a -= 1;
            } else {
                let Some((p, n)) = self.scopes.entries[in_scope.index].parent else {
                    return false;
                };
                in_scope = p;
                input = n;
                b -= 1;
            }
        }
        out < input
    }
    /// Retained output, input and scope slots, then live subscriptions.
    /// Diagnostic only; computing subscriptions visits the output table.
    pub fn storage_counts(&self) -> [usize; 4] {
        [
            self.outputs.len(),
            self.inputs.len(),
            self.scopes.entries.len(),
            self.outputs
                .iter()
                .map(|o| o.watchers.len() + o.followers.len())
                .sum(),
        ]
    }
    /// Enter a graph; return the previous scope for restoration after the hook.
    /// # Panics
    /// The scope must still belong to a live graph.
    pub fn enter_scope(&mut self, scope: ScopeId) -> ScopeId {
        assert!(self.scopes.alive(scope), "expired graph scope");
        std::mem::replace(&mut self.scopes.current, scope)
    }
    /// The current graph's scope.
    pub fn scope(&self) -> ScopeId {
        self.scopes.current
    }
    /// A child's scope, allocated during structural graph creation.
    pub fn child_scope(&mut self, owner: NodeId) -> ScopeId {
        let parent = self.scopes.current;
        self.scopes.reserve(parent, owner);
        let p = &mut self.scopes.entries[parent.index];
        let n = owner.0 as usize;
        p.child_count[n] += 1;
        let additional = p.child_count[n] - p.children[n].len();
        p.children[n].reserve(additional);
        let index = self.scopes.free.pop().unwrap_or(self.scopes.entries.len());
        let generation = if index == self.scopes.entries.len() {
            0
        } else {
            self.scopes.entries[index].generation
        };
        let scope = Scope {
            generation,
            parent: Some((parent, owner)),
            ..Scope::default()
        };
        if index == self.scopes.entries.len() {
            self.scopes.entries.push(scope);
        } else {
            self.scopes.entries[index] = scope;
        }
        ScopeId { index, generation }
    }
    /// Reserve all mailbox storage before the graph can be notified.
    pub fn reserve_scope(&mut self, scope: ScopeId, nodes: usize) {
        self.scopes.entries[scope.index].phase = Phase::Running;
        if nodes > 0 {
            self.scopes.reserve(scope, NodeId(super::index(nodes - 1)));
        }
    }
    /// A pending local node. Removing it does not consume its timer.
    pub fn take_wake(&mut self, scope: ScopeId) -> Option<NodeId> {
        let s = &mut self.scopes.entries[scope.index];
        let n = s.pending.pop()?;
        s.queued[n.0 as usize] = false;
        Some(n)
    }
    /// One direct child needing its owner to evaluate it.
    pub fn take_child(&mut self, owner: NodeId) -> Option<ScopeId> {
        loop {
            let id = self.scopes.entries[self.scopes.current.index]
                .children
                .get_mut(owner.0 as usize)?
                .pop()?;
            if self.scopes.alive(id) {
                self.scopes.entries[id.index].enqueued = false;
                return Some(id);
            }
        }
    }
    /// Disconnect a stopped child's ports; outputs expire at the next cycle.
    pub fn release_scope(&mut self, scope: ScopeId, now: EngineTime) {
        if !self.scopes.alive(scope) || scope.index == 0 {
            return;
        }
        self.scopes.entries[scope.index].phase = Phase::Releasing;
        while let Some(i) = self.scopes.entries[scope.index].inputs.pop() {
            if self.input(i).alive && self.input(i).scope == scope {
                self.release_input(i);
            }
        }
        while let Some(o) = self.scopes.entries[scope.index].outputs.pop() {
            if self.output(o).alive && self.output(o).scope == scope {
                self.retire(o, now);
            }
        }
        if let Some((parent, owner)) = self.scopes.entries[scope.index].parent {
            let parent = &mut self.scopes.entries[parent.index];
            parent.child_count[owner.0 as usize] -= 1;
            parent.children[owner.0 as usize].retain(|&id| id != scope);
        }
        let s = &mut self.scopes.entries[scope.index];
        s.phase = Phase::Dead;
        if let Some(g) = s.generation.checked_add(1) {
            s.generation = g;
            self.scopes.free.push(scope.index);
        }
    }
    pub(crate) fn forget_output(&mut self, output: OutputId) {
        let scope = self.output(output).scope;
        if scope.index == 0 || !self.scopes.alive(scope) {
            return;
        }
        let position = self.output(output).scope_position;
        let ids = &mut self.scopes.entries[scope.index].outputs;
        ids.swap_remove(position);
        if let Some(&moved) = ids.get(position) {
            self.outputs[moved.0 as usize].scope_position = position;
        }
    }
    pub(crate) fn release_input(&mut self, input: InputId) {
        self.unbind(input);
        self.dirty_inputs.retain(|&id| id != input);
        let scope = self.input(input).scope;
        if scope.index != 0 && self.scopes.entries[scope.index].phase != Phase::Releasing {
            let position = self.input(input).scope_position;
            let ids = &mut self.scopes.entries[scope.index].inputs;
            ids.swap_remove(position);
            if let Some(&moved) = ids.get(position) {
                self.inputs[moved.0 as usize].scope_position = position;
            }
        }
        let i = &mut self.inputs[input.0 as usize];
        if i.alive {
            i.alive = false;
            self.free_inputs.push(input);
        }
    }
}
