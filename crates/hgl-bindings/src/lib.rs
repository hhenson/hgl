//! Logical endpoints, binding and lifetime; scalar values live in hgl-store.
mod collections;
mod fixed;
mod scopes;

use hgl_endpoints::{Assemblies, Scopes};
use hgl_types::{EngineTime, NodeId, ScalarType};

use hgl_endpoints::Endpoints;
pub use hgl_endpoints::Wake;
pub use hgl_endpoints::{Input, InputId, Kind, Output, OutputId, Reference, ScopeId};
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
    /// Attachment requires a live generation-checked peer.
    InvalidReference,
    /// A plain bind cannot replace an existing binding.
    AlreadyBound(InputId),
    /// A reference would route against graph rank.
    BackwardReference,
}
/// Endpoint tables and the notifications between graph scopes.
#[derive(Debug, Default)]
pub struct Bindings {
    endpoints: Endpoints,
    items: Assemblies,
    retired: Vec<OutputId>,
    dirty_outputs: Vec<OutputId>,
    dirty_inputs: Vec<InputId>,
    scopes: Scopes,
    now: EngineTime,
    fresh_run: bool,
}
fn index(n: usize) -> u32 {
    u32::try_from(n).unwrap_or_else(|_| unreachable!("endpoint capacity exceeded"))
}
impl Bindings {
    /// Output metadata, shared only.
    pub fn output(&self, id: OutputId) -> &Output {
        self.endpoints.output(id)
    }
    /// Input metadata, shared only.
    pub fn input(&self, id: InputId) -> &Input {
        self.endpoints.input(id)
    }
    /// Allocate an endpoint; scalar columns grow only for fresh slots.
    pub fn add_output(&mut self, owner: NodeId, kind: Kind, next_slot: u32) -> (OutputId, bool) {
        let result = self
            .endpoints
            .add_output(owner, kind, next_slot, self.scope());
        self.scopes.reserve(self.scope(), owner);
        self.endpoints.outputs[result.0.0 as usize].scope_position =
            self.scopes.record_output(result.0);
        result
    }
    /// Allocate an input and every fixed child in its graph scope.
    pub fn add_input(&mut self, owner: NodeId, kind: Kind, active: bool) -> InputId {
        let id = self.endpoints.add_input(owner, kind, active, self.scope());
        self.scopes.reserve(self.scope(), owner);
        self.endpoints.inputs[id.0 as usize].scope_position = self.scopes.record_input(id);
        for position in 0..self.input(id).kind.len() {
            let child = self.add_input(owner, self.input(id).kind.child(position).clone(), active);
            self.endpoints.inputs[child.0 as usize].parent = Some((
                id,
                i64::try_from(position).unwrap_or_else(|_| unreachable!()),
            ));
            self.endpoints.inputs[id.0 as usize].fixed.push(child);
        }
        id
    }
    fn check_reference(&self, input: InputId, output: OutputId) -> Result<(), BindError> {
        self.check(input, output)?;
        let i = self.input(input);
        let o = self.output(output);
        if !self.scopes.forward(o.scope, o.owner, i.scope, i.owner) {
            return Err(BindError::BackwardReference);
        }
        Ok(())
    }
    fn checked_endpoints(
        &self,
        input: InputId,
        output: OutputId,
    ) -> Result<(&Input, &Output), BindError> {
        let i = self
            .endpoints
            .inputs
            .get(input.0 as usize)
            .filter(|i| i.alive)
            .ok_or(BindError::UnknownInput(input))?;
        let o = self
            .endpoints
            .outputs
            .get(output.0 as usize)
            .filter(|o| o.alive)
            .ok_or(BindError::UnknownOutput(output))?;
        Ok((i, o))
    }
    fn check(&self, input: InputId, output: OutputId) -> Result<(), BindError> {
        let (i, o) = self.checked_endpoints(input, output)?;
        if let (Kind::Ts(a), Kind::Ts(b)) = (&i.kind, &o.kind)
            && a != b
        {
            return Err(BindError::TypeMismatch {
                input: *a,
                output: *b,
            });
        }
        if i.kind != o.kind {
            return Err(BindError::ShapeMismatch);
        }
        Ok(())
    }
    /// Capture a stable endpoint designation without subscribing to its values.
    pub fn bind_designation(&mut self, input: InputId, output: OutputId) -> Result<(), BindError> {
        let (i, o) = self.checked_endpoints(input, output)?;
        if !matches!(&i.kind,Kind::Reference(child) if child.as_ref()==&o.kind) {
            return Err(BindError::ShapeMismatch);
        }
        if i.source.is_some() || i.designation.output.is_some() {
            return Err(BindError::AlreadyBound(input));
        }
        if !self.scopes.forward(o.scope, o.owner, i.scope, i.owner) {
            return Err(BindError::BackwardReference);
        }
        let r = self.reference(output);
        self.endpoints.inputs[input.0 as usize].designation = r;
        Ok(())
    }
    /// Plain wiring-time bind, without sampling or notification.
    pub fn bind(&mut self, input: InputId, output: OutputId) -> Result<(), BindError> {
        self.check(input, output)?;
        if self.input(input).source.is_some() {
            return Err(BindError::AlreadyBound(input));
        }
        self.attach(input, output);
        self.sync_fixed(input, self.reference(output), EngineTime::NEVER);
        self.sync_members(input, self.now);
        self.refresh(input, self.now, false);
        Ok(())
    }
    fn attach(&mut self, input: InputId, output: OutputId) {
        let o = &mut self.endpoints.outputs[output.0 as usize];
        let position = o.watchers.len();
        o.watchers.push(input);
        let i = &mut self.endpoints.inputs[input.0 as usize];
        i.source_position = position;
        i.source = Some(output);
        i.slot = o.slot;
    }
    /// Detach silently, including a reference subscription.
    pub fn unbind(&mut self, input: InputId) {
        self.detach(input);
        for n in 0..self.input(input).fixed.len() {
            self.unbind(self.input(input).fixed[n]);
        }
        self.clear_projection(input);
        self.detach_reference(input);
        self.refresh(input, self.now, false);
        self.reset_observation(input);
        self.endpoints.inputs[input.0 as usize].notified_at = EngineTime::NEVER;
        self.items.release(self.input(input).designation);
        self.endpoints.inputs[input.0 as usize].designation = Reference::default();
        let mut parent = self.input(input).parent;
        while let Some((p, _)) = parent {
            self.refresh(p, self.now, false);
            parent = self.input(p).parent;
        }
    }
    fn detach_reference(&mut self, input: InputId) {
        if let Some(r) = self.endpoints.inputs[input.0 as usize]
            .reference_source
            .take()
        {
            let position = self.input(input).reference_position;
            let followers = &mut self.endpoints.outputs[r.0 as usize].followers;
            followers.swap_remove(position);
            if let Some(&moved) = followers.get(position) {
                self.endpoints.inputs[moved.0 as usize].reference_position = position;
            }
        }
    }
    fn clear_projection(&mut self, input: InputId) {
        while let Some((_, child)) = self.endpoints.inputs[input.0 as usize]
            .members
            .live
            .pop_first()
        {
            self.release_input(child);
        }
        while let Some((_, child)) = self.endpoints.inputs[input.0 as usize]
            .members
            .removed
            .pop_first()
        {
            self.release_input(child);
        }
        self.endpoints.inputs[input.0 as usize]
            .members
            .initial
            .clear();
        self.endpoints.inputs[input.0 as usize]
            .members
            .changed
            .clear();
    }
    fn detach(&mut self, input: InputId) {
        if let Some(o) = self.endpoints.inputs[input.0 as usize].source.take() {
            let position = self.input(input).source_position;
            let watchers = &mut self.endpoints.outputs[o.0 as usize].watchers;
            watchers.swap_remove(position);
            if let Some(&moved) = watchers.get(position) {
                self.endpoints.inputs[moved.0 as usize].source_position = position;
            }
        }
    }
    /// Change notification admission, without changing observations.
    pub fn set_active(&mut self, input: InputId, active: bool) {
        self.endpoints.inputs[input.0 as usize].active = active;
        for n in 0..self.input(input).fixed.len() {
            self.set_active(self.input(input).fixed[n], active);
        }
        let members = std::mem::take(&mut self.endpoints.inputs[input.0 as usize].members);
        for &child in members.live.values().chain(members.removed.values()) {
            self.set_active(child, active);
        }
        self.endpoints.inputs[input.0 as usize].members = members;
    }
    /// Cached input time, including invalidation within a valid assembly.
    pub fn last_modified(&self, input: InputId) -> EngineTime {
        let i = self.input(input);
        if i.kind.fixed() || i.source.is_none() {
            return i.observed_at;
        }
        let t = self
            .output(i.source.unwrap_or_else(|| unreachable!()))
            .modified_at;
        if t == EngineTime::NEVER {
            i.observed_at
        } else {
            t.max(i.sampled_at)
        }
    }
    /// Value availability, independent of a retained invalidation time.
    pub fn valid(&self, input: InputId) -> bool {
        let i = self.input(input);
        if matches!(i.kind, Kind::Reference(_)) && i.source.is_none() {
            self.resolve(i.designation).is_some()
        } else if i.kind.fixed() && i.source.is_none() {
            i.valid_children > 0
        } else {
            i.source
                .is_some_and(|o| self.output(o).modified_at != EngineTime::NEVER)
        }
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
                items: None,
            }
        } else {
            Reference::default()
        }
    }
    /// Resolve without retaining or reviving the endpoint.
    pub fn resolve(&self, r: Reference) -> Option<OutputId> {
        r.output.filter(|id| {
            self.endpoints
                .outputs
                .get(id.0 as usize)
                .is_some_and(|o| o.alive && o.generation == r.generation)
        })
    }
    /// The logical reference value, empty once its target expires.
    pub fn reference_value(&self, output: OutputId) -> Reference {
        let r = self.output(output).reference;
        if self.items.get(r).is_some() || self.resolve(r).is_some() {
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
        self.check_designation(input, r)?;
        if self.apply_sample(input, r, now) {
            self.notify_input(input, now, false, wake);
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
        let Kind::Reference(target) = &self.output(reference).kind else {
            return Err(BindError::ShapeMismatch);
        };
        if &self.input(input).kind != target.as_ref() {
            return Err(BindError::ShapeMismatch);
        }
        let i = self.input(input);
        let o = self.output(reference);
        if !self.scopes.forward(o.scope, o.owner, i.scope, i.owner) {
            return Err(BindError::BackwardReference);
        }
        let r = self.reference_value(reference);
        self.check_designation(input, r)?;
        if self.input(input).reference_source == Some(reference) {
            return Ok(());
        }
        self.detach_reference(input);
        self.endpoints.inputs[input.0 as usize].reference_position =
            self.output(reference).followers.len();
        self.endpoints.outputs[reference.0 as usize]
            .followers
            .push(input);
        self.endpoints.inputs[input.0 as usize].reference_source = Some(reference);
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
        let Kind::Reference(expected) = &self.output(output).kind else {
            return Err(BindError::ShapeMismatch);
        };
        self.check_shape(expected, r)?;
        for &input in &self.output(output).followers {
            self.check_designation(input, r)?;
        }
        if self.output(output).modified_at != EngineTime::NEVER
            && self.output(output).reference == r
        {
            return Ok(());
        }
        for n in 0..self.output(output).followers.len() {
            let i = self.output(output).followers[n];
            self.sample(i, r, now, wake)?;
        }
        self.items.replace(self.output(output).reference, r);
        self.endpoints.outputs[output.0 as usize].reference = r;
        self.publish(output, now, wake);
        Ok(())
    }
    /// Stamp and notify once per cycle, including owning dictionaries.
    pub fn publish<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W) {
        self.endpoints.outputs[output.0 as usize].modified_at = now;
        self.notify_output(output, now, wake);
    }
    /// Invalidate the child while recording its parent's change.
    pub fn invalidate<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W) {
        if self.output(output).modified_at == EngineTime::NEVER {
            return;
        }
        match &self.output(output).kind {
            Kind::Dictionary(_) | Kind::Set(_) => {
                let children =
                    std::mem::take(&mut self.endpoints.outputs[output.0 as usize].members.live);
                for &child in children.values() {
                    self.invalidate(child, now, wake);
                }
                self.endpoints.outputs[output.0 as usize].members.live = children;
            }
            Kind::Reference(_) => {
                for n in 0..self.output(output).followers.len() {
                    let input = self.output(output).followers[n];
                    let result = self.sample(input, Reference::default(), now, wake);
                    debug_assert!(result.is_ok());
                }
                self.items.release(self.output(output).reference);
                self.endpoints.outputs[output.0 as usize].reference = Reference::default();
            }
            Kind::List(..) | Kind::Bundle(_) => {
                for n in 0..self.output(output).fixed.len() {
                    self.invalidate(self.output(output).fixed[n], now, wake);
                }
            }
            Kind::Ts(_) => {}
        }
        self.endpoints.outputs[output.0 as usize].modified_at = EngineTime::NEVER;
        self.notify_output(output, now, wake);
    }
    fn notify_output<W: Wake>(&mut self, output: OutputId, now: EngineTime, wake: &mut W) {
        let valid = self.output(output).modified_at != EngineTime::NEVER;
        if let Some((parent, key)) = self.output(output).parent {
            if self.output(parent).kind.fixed() {
                let old = self.output(output).parent_valid;
                self.endpoints.outputs[output.0 as usize].parent_valid = valid;
                let p = &mut self.endpoints.outputs[parent.0 as usize];
                p.valid_children = p.valid_children + usize::from(valid) - usize::from(old);
                p.modified_at = if p.valid_children == 0 && p.modified_at == EngineTime::NEVER {
                    EngineTime::NEVER
                } else {
                    now
                };
                self.notify_output(parent, now, wake);
            } else {
                if self.output(output).parent_at != now {
                    self.endpoints.outputs[output.0 as usize].parent_at = now;
                    self.change_output(parent, key, now);
                }
                self.publish(parent, now, wake);
            }
        }
        let o = &mut self.endpoints.outputs[output.0 as usize];
        if o.notified_at == now && o.notified_valid == valid {
            return;
        }
        o.notified_at = now;
        o.notified_valid = valid;
        for n in 0..self.output(output).watchers.len() {
            let i = self.output(output).watchers[n];
            self.notify_input(i, now, true, wake);
        }
    }
    fn notify_input<W: Wake>(
        &mut self,
        input: InputId,
        now: EngineTime,
        event: bool,
        wake: &mut W,
    ) {
        self.refresh(input, now, event);
        if let Some((p, key)) = self.input(input).parent {
            if !self.input(p).kind.fixed() && self.input(input).parent_at != now {
                self.endpoints.inputs[input.0 as usize].parent_at = now;
                self.change_input(p, key, now);
            }
            self.notify_input(p, now, event, wake);
        } else {
            let i = &mut self.endpoints.inputs[input.0 as usize];
            if i.active && i.notified_at != now {
                i.notified_at = now;
                self.scopes.wake(i.scope, i.owner, wake);
            }
        }
    }
    /// Retain until the first later engine cycle, even with no later writes.
    pub(crate) fn retire(&mut self, output: OutputId, now: EngineTime) {
        let o = &mut self.endpoints.outputs[output.0 as usize];
        o.retired_at = now;
        if !o.retirement_queued {
            o.retirement_queued = true;
            self.retired.push(output);
        }
    }
    fn expire(&mut self, id: OutputId) {
        if !self.output(id).alive {
            return;
        }
        for n in 0..self.output(id).fixed.len() {
            self.expire(self.output(id).fixed[n]);
        }
        let children = std::mem::take(&mut self.endpoints.outputs[id.0 as usize].members);
        for child in children
            .live
            .into_values()
            .chain(children.removed.into_values())
        {
            self.expire(child);
        }
        while let Some(i) = self.endpoints.outputs[id.0 as usize].watchers.pop() {
            self.endpoints.inputs[i.0 as usize].source = None;
            self.endpoints.inputs[i.0 as usize].sampled_at = EngineTime::NEVER;
            let reference = self.endpoints.inputs[i.0 as usize].reference_source.take();
            self.unbind(i);
            self.endpoints.inputs[i.0 as usize].reference_source = reference;
        }
        while let Some(i) = self.endpoints.outputs[id.0 as usize].followers.pop() {
            self.endpoints.inputs[i.0 as usize].reference_source = None;
            self.unbind(i);
        }
        self.items.release(self.output(id).reference);
        self.forget_output(id);
        let o = &mut self.endpoints.outputs[id.0 as usize];
        o.alive = false;
        o.retired_at = EngineTime::NEVER;
        o.retirement_queued = false;
        o.modified_at = EngineTime::NEVER;
        if let Some(g) = o.generation.checked_add(1) {
            o.generation = g;
            self.endpoints.free_outputs.push(id);
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
                self.endpoints.outputs[id.0 as usize].retirement_queued = false;
                self.retired.swap_remove(n);
                continue;
            }
            if t >= now && !fresh_run {
                n += 1;
                continue;
            }
            self.expire(id);
            self.retired.swap_remove(n);
        }
        self.scopes.reclaim(now, fresh_run, &mut self.items);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhausted_generation_is_never_reused() {
        let mut bindings = Bindings::default();
        let kind = Kind::Ts(ScalarType::I64);
        let (old, _) = bindings.add_output(NodeId(0), kind.clone(), 0);
        bindings.endpoints.outputs[old.0 as usize].generation = u32::MAX;
        let saved = bindings.reference(old);
        bindings.retire(old, EngineTime::from_micros(1));
        bindings.begin_cycle(EngineTime::from_micros(2));
        let (new, _) = bindings.add_output(NodeId(0), kind, 1);
        assert_ne!(old, new);
        assert!(bindings.resolve(saved).is_none());
    }
}
