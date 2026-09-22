//! Graph scope mailbox storage.
use crate::{InputId, OutputId, ScopeId, Wake};
use hgl_types::NodeId;
#[derive(Debug, Default, PartialEq, Eq)]
/// Scope lifecycle admission.
pub enum Phase {
    #[default]
    /// Lifecycle phase.
    Constructing,
    /// Lifecycle phase.
    Running,
    /// Lifecycle phase.
    Releasing,
    /// Lifecycle phase.
    Dead,
}
#[derive(Debug, Default)]
/// Graph scope storage, owned privately by bindings.
pub struct Scope {
    /// Scope mailbox storage.
    pub generation: u64,
    /// Scope mailbox storage.
    pub phase: Phase,
    /// Scope mailbox storage.
    pub parent: Option<(ScopeId, NodeId)>,
    /// Scope mailbox storage.
    pub pending: Vec<NodeId>,
    /// Scope mailbox storage.
    pub queued: Vec<bool>,
    /// Scope mailbox storage.
    pub children: Vec<Vec<ScopeId>>,
    /// Scope mailbox storage.
    pub child_count: Vec<usize>,
    /// Scope mailbox storage.
    pub enqueued: bool,
    /// Scope mailbox storage.
    pub inputs: Vec<InputId>,
    /// Scope mailbox storage.
    pub outputs: Vec<OutputId>,
}
#[derive(Debug)]
/// Graph scope storage, owned privately by bindings.
pub struct Scopes {
    /// Scope mailbox storage.
    pub entries: Vec<Scope>,
    /// Scope mailbox storage.
    pub free: Vec<usize>,
    /// Stopped scopes whose output references retain ancestry until the next cycle.
    pub retired: Vec<usize>,
    /// Scope mailbox storage.
    pub current: ScopeId,
}
impl Default for Scopes {
    fn default() -> Self {
        Self {
            entries: vec![Scope {
                phase: Phase::Running,
                ..Scope::default()
            }],
            free: Vec::new(),
            retired: Vec::new(),
            current: ScopeId::default(),
        }
    }
}
impl Scopes {
    /// Whether a scope is still live.
    pub fn alive(&self, id: ScopeId) -> bool {
        self.entries
            .get(id.index)
            .is_some_and(|s| s.phase != Phase::Dead && s.generation == id.generation)
    }
    /// Scope allocation and notification bookkeeping.
    pub fn reserve(&mut self, id: ScopeId, node: NodeId) {
        let s = &mut self.entries[id.index];
        let count = node.0 as usize + 1;
        if count > s.queued.len() {
            s.queued.resize(count, false);
            s.children.resize_with(count, Vec::new);
            s.child_count.resize(count, 0);
            s.pending.reserve(count - s.pending.len());
        }
    }
    /// Scope allocation and notification bookkeeping.
    pub fn record_output(&mut self, id: OutputId) -> usize {
        let scope = &mut self.entries[self.current.index];
        let position = scope.outputs.len();
        if self.current.index != 0 {
            scope.outputs.push(id);
        }
        position
    }
    /// Scope allocation and notification bookkeeping.
    pub fn record_input(&mut self, id: InputId) -> usize {
        let scope = &mut self.entries[self.current.index];
        let position = scope.inputs.len();
        if self.current.index != 0 {
            scope.inputs.push(id);
        }
        position
    }
    /// Scope allocation and notification bookkeeping.
    pub fn wake<W: Wake>(&mut self, mut scope: ScopeId, mut node: NodeId, wake: &mut W) {
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

impl Scopes {
    /// Whether a reference follows local and enclosing graph rank.
    pub fn forward(
        &self,
        mut out_scope: ScopeId,
        mut out: NodeId,
        mut in_scope: ScopeId,
        mut input: NodeId,
    ) -> bool {
        let depth = |mut scope: ScopeId| {
            let mut n = 0;
            while let Some((p, _)) = self.entries[scope.index].parent {
                n += 1;
                scope = p;
            }
            n
        };
        let (mut a, mut b) = (depth(out_scope), depth(in_scope));
        while out_scope != in_scope {
            if a >= b {
                let Some((p, n)) = self.entries[out_scope.index].parent else {
                    return false;
                };
                out_scope = p;
                out = n;
                a -= 1;
            } else {
                let Some((p, n)) = self.entries[in_scope.index].parent else {
                    return false;
                };
                in_scope = p;
                input = n;
                b -= 1;
            }
        }
        out < input
    }
}
