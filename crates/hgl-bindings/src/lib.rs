//! Logical endpoints, binding and lifetime; scalar values live in hgl-store.
mod collections;
mod scopes;

use hgl_types::{EngineTime, NodeId, ScalarType};
pub use scopes::ScopeId;
use scopes::Scopes;
use std::collections::BTreeMap;

/// One output slot in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct OutputId(pub u32);
/// One input slot in a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputId(pub u32);
/// The admitted endpoint shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A scalar output or view.
    Scalar(ScalarType),
    /// A dictionary with i64 keys and scalar children.
    Dictionary(ScalarType),
    /// A reference to a scalar or dictionary endpoint.
    Reference {
        /// Child scalar type.
        scalar: ScalarType,
        /// Whether the target is a dictionary.
        dictionary: bool,
    },
}
impl Kind {
    /// The scalar column used by this shape's values.
    pub fn scalar(self) -> ScalarType {
        match self {
            Self::Scalar(t) | Self::Dictionary(t) | Self::Reference { scalar: t, .. } => t,
        }
    }
}
/// A designation; retaining it does not retain its endpoint.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Reference {
    output: Option<OutputId>,
    generation: u32,
}
/// Who is woken by a publication in the current scope.
pub trait Wake {
    /// Make a local node ready, idempotently.
    fn wake(&mut self, node: NodeId);
}
/// A binding rejected before changing its previous state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindError {
    /// Unknown input.
    UnknownInput(InputId),
    /// Unknown or expired output.
    UnknownOutput(OutputId),
    /// Incompatible scalar columns.
    TypeMismatch {
        /// Input scalar.
        input: ScalarType,
        /// Output scalar.
        output: ScalarType,
    },
    /// Different endpoint shapes.
    ShapeMismatch,
    /// A plain bind cannot replace an existing binding.
    AlreadyBound(InputId),
    /// A reference would route against graph rank.
    BackwardReference,
}
#[derive(Debug)]
pub(crate) struct Members<I> {
    pub(crate) live: BTreeMap<i64, I>,
    pub(crate) removed: BTreeMap<i64, I>,
    pub(crate) initial: BTreeMap<i64, bool>,
    pub(crate) changed: Vec<i64>,
    pub(crate) epoch: EngineTime,
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
    scope_position: usize,
    /// NEVER while invalid.
    pub modified_at: EngineTime,
    /// Lifetime generation; exhausted generations are never reused.
    pub generation: u32,
    /// Whether this slot still designates an endpoint.
    pub alive: bool,
    notified_at: EngineTime,
    parent: Option<(OutputId, i64)>,
    parent_at: EngineTime,
    watchers: Vec<InputId>,
    followers: Vec<InputId>,
    reference: Reference,
    retired_at: EngineTime,
    retirement_queued: bool,
    members: Members<OutputId>,
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
    owner: NodeId,
    scope: ScopeId,
    scope_position: usize,
    active: bool,
    alive: bool,
    sampled_at: EngineTime,
    reference_source: Option<OutputId>,
    source_position: usize,
    reference_position: usize,
    parent: Option<(InputId, i64)>,
    parent_at: EngineTime,
    members: Members<InputId>,
    withdrawal: EngineTime,
}
/// Endpoint tables and the notifications between graph scopes.
#[derive(Debug)]
pub struct Bindings {
    outputs: Vec<Output>,
    inputs: Vec<Input>,
    free_outputs: Vec<OutputId>,
    free_inputs: Vec<InputId>,
    retired: Vec<OutputId>,
    dirty_outputs: Vec<OutputId>,
    dirty_inputs: Vec<InputId>,
    scopes: Scopes,
    now: EngineTime,
    fresh_run: bool,
}
impl Default for Bindings {
    fn default() -> Self {
        Self {
            outputs: Vec::new(),
            inputs: Vec::new(),
            free_outputs: Vec::new(),
            free_inputs: Vec::new(),
            retired: Vec::new(),
            dirty_outputs: Vec::new(),
            dirty_inputs: Vec::new(),
            scopes: Scopes::default(),
            now: EngineTime::NEVER,
            fresh_run: false,
        }
    }
}
fn index(n: usize) -> u32 {
    u32::try_from(n).unwrap_or_else(|_| unreachable!("endpoint capacity exceeded"))
}
impl Bindings {
    /// Output metadata. An unknown id is a caller error.
    pub fn output(&self, id: OutputId) -> &Output {
        &self.outputs[id.0 as usize]
    }
    /// Input metadata. An unknown id is a caller error.
    pub fn input(&self, id: InputId) -> &Input {
        &self.inputs[id.0 as usize]
    }
    /// Allocate or reuse a slot; the bool says whether a scalar column must grow.
    pub fn add_output(&mut self, owner: NodeId, kind: Kind, next_slot: u32) -> (OutputId, bool) {
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
            scope: self.scopes.current,
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
        };
        if reused.is_some() {
            self.outputs[id.0 as usize] = output;
        } else {
            self.outputs.push(output);
        }
        self.scopes.reserve(self.scopes.current, owner);
        self.outputs[id.0 as usize].scope_position = self.scopes.record_output(id);
        (id, reused.is_none())
    }
    /// Allocate an unbound view.
    pub fn add_input(&mut self, owner: NodeId, kind: Kind, active: bool) -> InputId {
        let id = self
            .free_inputs
            .pop()
            .unwrap_or(InputId(index(self.inputs.len())));
        let input = Input {
            source: None,
            slot: 0,
            kind,
            owner,
            scope: self.scopes.current,
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
        };
        if id.0 as usize == self.inputs.len() {
            self.inputs.push(input);
        } else {
            self.inputs[id.0 as usize] = input;
        }
        self.scopes.reserve(self.scopes.current, owner);
        self.inputs[id.0 as usize].scope_position = self.scopes.record_input(id);
        id
    }
    fn check_reference(&self, input: InputId, output: OutputId) -> Result<(), BindError> {
        self.check(input, output)?;
        let i = self.input(input);
        let o = self.output(output);
        if !self.forward(o.scope, o.owner, i.scope, i.owner) {
            return Err(BindError::BackwardReference);
        }
        Ok(())
    }
    fn check(&self, input: InputId, output: OutputId) -> Result<(), BindError> {
        let i = self
            .inputs
            .get(input.0 as usize)
            .filter(|i| i.alive)
            .ok_or(BindError::UnknownInput(input))?;
        let o = self
            .outputs
            .get(output.0 as usize)
            .filter(|o| o.alive)
            .ok_or(BindError::UnknownOutput(output))?;
        if i.kind.scalar() != o.kind.scalar() {
            return Err(BindError::TypeMismatch {
                input: i.kind.scalar(),
                output: o.kind.scalar(),
            });
        }
        if i.kind != o.kind {
            return Err(BindError::ShapeMismatch);
        }
        Ok(())
    }
    /// Plain wiring-time bind, without sampling or notification.
    pub fn bind(&mut self, input: InputId, output: OutputId) -> Result<(), BindError> {
        self.check(input, output)?;
        if self.input(input).source.is_some() {
            return Err(BindError::AlreadyBound(input));
        }
        self.attach(input, output);
        self.sync_members(input, self.now);
        Ok(())
    }
    fn attach(&mut self, input: InputId, output: OutputId) {
        let o = &mut self.outputs[output.0 as usize];
        let position = o.watchers.len();
        o.watchers.push(input);
        let i = &mut self.inputs[input.0 as usize];
        i.source_position = position;
        i.source = Some(output);
        i.slot = o.slot;
    }
    /// Detach silently, including a reference subscription.
    pub fn unbind(&mut self, input: InputId) {
        self.detach(input);
        self.clear_projection(input);
        if let Some(r) = self.inputs[input.0 as usize].reference_source.take() {
            let position = self.input(input).reference_position;
            let followers = &mut self.outputs[r.0 as usize].followers;
            followers.swap_remove(position);
            if let Some(&moved) = followers.get(position) {
                self.inputs[moved.0 as usize].reference_position = position;
            }
        }
        self.inputs[input.0 as usize].sampled_at = EngineTime::NEVER;
    }
    fn clear_projection(&mut self, input: InputId) {
        while let Some((_, child)) = self.inputs[input.0 as usize].members.live.pop_first() {
            self.release_input(child);
        }
        while let Some((_, child)) = self.inputs[input.0 as usize].members.removed.pop_first() {
            self.release_input(child);
        }
        self.inputs[input.0 as usize].members.initial.clear();
        self.inputs[input.0 as usize].members.changed.clear();
    }
    fn detach(&mut self, input: InputId) {
        if let Some(o) = self.inputs[input.0 as usize].source.take() {
            let position = self.input(input).source_position;
            let watchers = &mut self.outputs[o.0 as usize].watchers;
            watchers.swap_remove(position);
            if let Some(&moved) = watchers.get(position) {
                self.inputs[moved.0 as usize].source_position = position;
            }
        }
    }
    /// Change notification admission, without changing observations.
    pub fn set_active(&mut self, input: InputId, active: bool) {
        self.inputs[input.0 as usize].active = active;
        let members = std::mem::take(&mut self.inputs[input.0 as usize].members);
        for &child in members.live.values().chain(members.removed.values()) {
            self.set_active(child, active);
        }
        self.inputs[input.0 as usize].members = members;
    }
    /// Sampled input time; invalid views always report NEVER.
    pub fn last_modified(&self, input: InputId) -> EngineTime {
        let i = self.input(input);
        i.source.map_or(EngineTime::NEVER, |o| {
            let t = self.output(o).modified_at;
            if t == EngineTime::NEVER {
                t
            } else {
                t.max(i.sampled_at)
            }
        })
    }
    /// Includes dictionary withdrawal while invalid.
    pub fn modified(&self, input: InputId, now: EngineTime) -> bool {
        self.last_modified(input) == now || self.input(input).withdrawal == now
    }
    /// A lifetime-checked designation to an endpoint.
    pub fn reference(&self, output: OutputId) -> Reference {
        let o = self.output(output);
        if o.alive {
            Reference {
                output: Some(output),
                generation: o.generation,
            }
        } else {
            Reference::default()
        }
    }
    /// Resolve without retaining or reviving the endpoint.
    pub fn resolve(&self, r: Reference) -> Option<OutputId> {
        r.output.filter(|id| {
            self.outputs
                .get(id.0 as usize)
                .is_some_and(|o| o.alive && o.generation == r.generation)
        })
    }
    /// The logical reference value, empty once its target expires.
    pub fn reference_value(&self, output: OutputId) -> Reference {
        let r = self.output(output).reference;
        if self.resolve(r).is_some() {
            r
        } else {
            Reference::default()
        }
    }
    /// Bind through a designation; a repeated target does not sample again.
    pub fn sample<W: Wake>(
        &mut self,
        input: InputId,
        r: Reference,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        let source = self.resolve(r);
        if let Some(o) = source {
            self.check_reference(input, o)?;
        }
        if self.input(input).source == source {
            return Ok(());
        }
        self.detach(input);
        if let Some(o) = source {
            self.attach(input, o);
        }
        self.inputs[input.0 as usize].sampled_at = now;
        self.sync_members(input, now);
        if source.is_some_and(|o| self.output(o).modified_at != EngineTime::NEVER)
            || matches!(self.input(input).kind, Kind::Dictionary(_))
        {
            self.notify_input(input, now, wake);
        }
        Ok(())
    }
    /// Subscribe to designation ticks separately from target ticks.
    pub fn follow<W: Wake>(
        &mut self,
        input: InputId,
        reference: OutputId,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        let target = match self.output(reference).kind {
            Kind::Reference { scalar, dictionary } => {
                if dictionary {
                    Kind::Dictionary(scalar)
                } else {
                    Kind::Scalar(scalar)
                }
            }
            Kind::Scalar(_) | Kind::Dictionary(_) => return Err(BindError::ShapeMismatch),
        };
        if self.input(input).kind != target {
            return Err(BindError::ShapeMismatch);
        }
        let r = self.reference_value(reference);
        if let Some(o) = self.resolve(r) {
            self.check_reference(input, o)?;
        }
        if self.input(input).reference_source == Some(reference) {
            return Ok(());
        }
        self.unbind(input);
        self.inputs[input.0 as usize].reference_position = self.output(reference).followers.len();
        self.outputs[reference.0 as usize].followers.push(input);
        self.inputs[input.0 as usize].reference_source = Some(reference);
        self.sample(input, r, now, wake)
    }
    /// Publish a designation, leaving target publication independent.
    pub fn set_reference<W: Wake>(
        &mut self,
        output: OutputId,
        r: Reference,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        let expected = match self.output(output).kind {
            Kind::Reference { scalar, dictionary } => {
                if dictionary {
                    Kind::Dictionary(scalar)
                } else {
                    Kind::Scalar(scalar)
                }
            }
            Kind::Scalar(_) | Kind::Dictionary(_) => return Err(BindError::ShapeMismatch),
        };
        if self
            .resolve(r)
            .is_some_and(|o| self.output(o).kind != expected)
        {
            return Err(BindError::ShapeMismatch);
        }
        if let Some(target) = self.resolve(r) {
            for &input in &self.output(output).followers {
                self.check_reference(input, target)?;
            }
        }
        for n in 0..self.output(output).followers.len() {
            let i = self.output(output).followers[n];
            self.sample(i, r, now, wake)?;
        }
        self.outputs[output.0 as usize].reference = r;
        self.publish(output, now, wake);
        Ok(())
    }
    /// Stamp and notify once per cycle, including owning dictionaries.
    pub fn publish<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W) {
        self.outputs[output.0 as usize].modified_at = now;
        self.notify_output(output, now, wake);
    }
    /// Invalidate the child while recording its parent's change.
    pub fn invalidate<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W) {
        match self.output(output).kind {
            Kind::Dictionary(_) => {
                let children = std::mem::take(&mut self.outputs[output.0 as usize].members.live);
                for &child in children.values() {
                    self.invalidate(child, now, wake);
                }
                self.outputs[output.0 as usize].members.live = children;
            }
            Kind::Reference { .. } => {
                for n in 0..self.output(output).followers.len() {
                    let input = self.output(output).followers[n];
                    let result = self.sample(input, Reference::default(), now, wake);
                    debug_assert!(result.is_ok());
                }
                self.outputs[output.0 as usize].reference = Reference::default();
            }
            Kind::Scalar(_) => {}
        }
        self.outputs[output.0 as usize].modified_at = EngineTime::NEVER;
        self.notify_output(output, now, wake);
    }
    fn notify_output<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W) {
        if let Some((parent, key)) = self.output(output).parent {
            if self.output(output).parent_at != now {
                self.outputs[output.0 as usize].parent_at = now;
                self.change_output(parent, key, now);
            }
            self.publish(parent, now, wake);
        }
        let o = &mut self.outputs[output.0 as usize];
        if o.notified_at == now {
            return;
        }
        o.notified_at = now;
        for n in 0..self.output(output).watchers.len() {
            let i = self.output(output).watchers[n];
            self.notify_input(i, now, wake);
        }
    }
    fn notify_input<W: Wake>(&mut self, input: InputId, now: EngineTime, wake: &mut W) {
        if let Some((p, key)) = self.input(input).parent
            && self.input(input).parent_at != now
        {
            self.inputs[input.0 as usize].parent_at = now;
            self.change_input(p, key, now);
        }
        let i = self.input(input);
        if i.active {
            let (scope, node) = (i.scope, i.owner);
            self.scopes.wake(scope, node, wake);
        }
    }
    /// Retain until the first later engine cycle, even with no later writes.
    pub(crate) fn retire(&mut self, output: OutputId, now: EngineTime) {
        let o = &mut self.outputs[output.0 as usize];
        o.retired_at = now;
        if !o.retirement_queued {
            o.retirement_queued = true;
            self.retired.push(output);
        }
    }
    /// Start an independent root run without resetting any endpoint values.
    /// Its first cycle ends previous-run removal observations, even if its
    /// logical clock starts earlier. Child startup must not call this.
    pub fn start_run(&mut self) {
        self.now = EngineTime::NEVER;
        self.fresh_run = true;
    }
    /// Expire retired endpoints and previous-cycle membership observations.
    /// # Panics
    /// Engine time must not move backwards.
    pub fn begin_cycle(&mut self, now: EngineTime) {
        if now == self.now {
            return;
        }
        assert!(now > self.now, "engine time moved backwards");
        self.now = now;
        let fresh_run = std::mem::take(&mut self.fresh_run);
        self.clear_members();
        let mut n = 0;
        while n < self.retired.len() {
            let id = self.retired[n];
            let t = self.output(id).retired_at;
            if t == EngineTime::NEVER {
                self.outputs[id.0 as usize].retirement_queued = false;
                self.retired.swap_remove(n);
                continue;
            }
            if t >= now && !fresh_run {
                n += 1;
                continue;
            }
            while let Some(i) = self.outputs[id.0 as usize].watchers.pop() {
                self.inputs[i.0 as usize].source = None;
                self.inputs[i.0 as usize].sampled_at = EngineTime::NEVER;
                self.clear_projection(i);
            }
            while let Some(i) = self.outputs[id.0 as usize].followers.pop() {
                self.inputs[i.0 as usize].reference_source = None;
                self.unbind(i);
            }
            self.forget_output(id);
            let o = &mut self.outputs[id.0 as usize];
            o.alive = false;
            o.retired_at = EngineTime::NEVER;
            o.retirement_queued = false;
            o.modified_at = EngineTime::NEVER;
            if let Some(g) = o.generation.checked_add(1) {
                o.generation = g;
                self.free_outputs.push(id);
            }
            self.retired.swap_remove(n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_generation_is_never_reused() {
        let mut bindings = Bindings::default();
        let kind = Kind::Scalar(ScalarType::I64);
        let (old, _) = bindings.add_output(NodeId(0), kind, 0);
        bindings.outputs[old.0 as usize].generation = u32::MAX;
        let saved = bindings.reference(old);
        bindings.retire(old, EngineTime::from_micros(1));
        bindings.begin_cycle(EngineTime::from_micros(2));
        let (new, _) = bindings.add_output(NodeId(0), kind, 1);
        assert_ne!(old, new);
        assert!(bindings.resolve(saved).is_none());
    }
}
