//! Membership and input projections, independent of scalar value columns.
use crate::{BindError, Bindings, InputId, Kind, OutputId, Reference, Wake};
use hgl_types::EngineTime;

impl Bindings {
    fn touch_output(&mut self, id: OutputId, now: EngineTime) {
        if self.output(id).members.epoch != now {
            self.endpoints.outputs[id.0 as usize].members.epoch = now;
            self.dirty_outputs.push(id);
        }
    }
    fn touch_input(&mut self, id: InputId, now: EngineTime) {
        if self.input(id).members.epoch != now {
            self.endpoints.inputs[id.0 as usize].members.epoch = now;
            self.dirty_inputs.push(id);
        }
    }
    pub(crate) fn change_output(&mut self, id: OutputId, key: i64, now: EngineTime) {
        self.touch_output(id, now);
        let members = &mut self.endpoints.outputs[id.0 as usize].members;
        members.changed.push(key);
    }
    pub(crate) fn change_input(&mut self, id: InputId, key: i64, now: EngineTime) {
        self.touch_input(id, now);
        let members = &mut self.endpoints.inputs[id.0 as usize].members;
        members.changed.push(key);
    }
    /// The live child of an output dictionary, independent of child validity.
    pub fn child_output(&self, id: OutputId, key: i64) -> Option<OutputId> {
        self.output(id).members.live.get(&key).copied()
    }
    /// A child view; membership is independent of validity.
    pub fn child_input(&self, id: InputId, key: i64) -> Option<InputId> {
        self.input(id).members.live.get(&key).copied()
    }
    /// The retained removed child, available only in its removal cycle.
    pub fn removed_output(&self, id: OutputId, key: i64) -> Option<OutputId> {
        self.output(id).members.removed.get(&key).copied()
    }
    /// A removed output whose writer is still alive and can retain ownership.
    pub fn restorable_output(&self, id: OutputId, key: i64) -> Option<OutputId> {
        self.removed_output(id, key)
            .filter(|&id| self.output(id).alive && self.scopes.alive(self.output(id).scope))
    }
    /// Retained removed input view.
    pub fn removed_input(&self, id: InputId, key: i64) -> Option<InputId> {
        self.input(id).members.removed.get(&key).copied()
    }
    /// Current input keys, including invalid children.
    pub fn keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_ {
        self.input(id).members.live.keys().copied()
    }
    /// Modified member keys. Each key is reported at most once per cycle.
    pub fn changed_keys(&self, id: InputId) -> &[i64] {
        &self.input(id).members.changed
    }
    /// Added keys compare membership, not first publication.
    pub fn added_keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_ {
        let m = &self.input(id).members;
        m.initial
            .iter()
            .filter_map(|(&k, &was)| (!was && m.live.contains_key(&k)).then_some(k))
    }
    /// Removed keys exclude same-cycle restoration.
    pub fn removed_keys(&self, id: InputId) -> impl Iterator<Item = i64> + '_ {
        let m = &self.input(id).members;
        m.initial
            .iter()
            .filter_map(|(&k, &was)| (was && !m.live.contains_key(&k)).then_some(k))
    }
    /// A dictionary is all valid iff it and each immediate live child are valid.
    pub fn all_valid(&self, id: InputId) -> bool {
        self.valid(id)
            && if self.input(id).kind.fixed() {
                self.input(id).valid_children == self.input(id).fixed.len()
            } else {
                self.input(id).members.live.values().all(|&i| self.valid(i))
            }
    }
    /// Attach an existing output, retaining its graph's writing ownership.
    pub fn insert<W: Wake>(
        &mut self,
        dict: OutputId,
        key: i64,
        child: OutputId,
        now: EngineTime,
        wake: &mut W,
    ) -> Result<(), BindError> {
        for id in [dict, child] {
            if self
                .endpoints
                .outputs
                .get(id.0 as usize)
                .is_none_or(|o| !o.alive || !self.scopes.alive(o.scope))
            {
                return Err(BindError::UnknownOutput(id));
            }
        }
        if self.output(dict).kind.member() != Some(&self.output(child).kind) {
            return Err(BindError::ShapeMismatch);
        }
        if self
            .output(child)
            .parent
            .is_some_and(|parent| parent != (dict, key))
            || !self.owns_child(dict, child)
        {
            return Err(BindError::ShapeMismatch);
        }
        if self.child_output(dict, key).is_some() {
            return Err(BindError::ShapeMismatch);
        }
        self.begin_cycle(now);
        self.touch_output(dict, now);
        let m = &mut self.endpoints.outputs[dict.0 as usize].members;
        let was = m.removed.contains_key(&key);
        m.initial.entry(key).or_insert(was);
        m.removed.remove(&key);
        m.live.insert(key, child);
        m.changed.reserve(m.live.len() + m.removed.len());
        let c = &mut self.endpoints.outputs[child.0 as usize];
        c.parent = Some((dict, key));
        c.retired_at = EngineTime::NEVER;
        self.sync_watchers(dict, key, now);
        self.notify_output(child, now, wake);
        self.publish(dict, now, wake);
        Ok(())
    }
    /// Remove membership now; retain the child until the next engine cycle.
    pub fn remove<W: Wake>(&mut self, dict: OutputId, key: i64, now: EngineTime, wake: &mut W) {
        self.begin_cycle(now);
        let Some(child) = self.child_output(dict, key) else {
            return;
        };
        self.touch_output(dict, now);
        let m = &mut self.endpoints.outputs[dict.0 as usize].members;
        m.initial.entry(key).or_insert(true);
        m.live.remove(&key);
        m.removed.insert(key, child);
        self.retire(child, now);
        self.sync_watchers(dict, key, now);
        self.publish(dict, now, wake);
    }
    fn sync_watchers(&mut self, dict: OutputId, key: i64, now: EngineTime) {
        let output = self.child_output(dict, key);
        for n in 0..self.output(dict).watchers.len() {
            let input = self.output(dict).watchers[n];
            self.sync_key(input, key, output, now, false);
        }
    }
    fn sync_key(
        &mut self,
        input: InputId,
        key: i64,
        output: Option<OutputId>,
        now: EngineTime,
        sampled: bool,
    ) {
        self.touch_input(input, now);
        let Some(output) = output else {
            let m = &mut self.endpoints.inputs[input.0 as usize].members;
            if let Some(child) = m.live.remove(&key) {
                m.initial.entry(key).or_insert(true);
                m.removed.insert(key, child);
            }
            return;
        };
        let child = self.ensure_child_input(input, key);
        self.endpoints.inputs[child.0 as usize].parent = Some((input, key));
        if self.input(child).source != Some(output) {
            self.detach(child);
            self.attach(child, output);
        }
        if sampled && self.output(output).modified_at != EngineTime::NEVER {
            self.endpoints.inputs[child.0 as usize].sampled_at = now;
        }
        self.sync_fixed(
            child,
            self.reference(output),
            if sampled { now } else { EngineTime::NEVER },
        );
        self.sync_members(child, now);
        self.refresh(child, now, false);
        let m = &mut self.endpoints.inputs[input.0 as usize].members;
        m.changed.reserve(
            m.live.len() + m.removed.len() - m.changed.len().min(m.live.len() + m.removed.len()),
        );
        if (sampled || self.output(output).modified_at == now) && self.input(child).parent_at != now
        {
            self.endpoints.inputs[child.0 as usize].parent_at = now;
            self.change_input(input, key, now);
        }
    }
    pub(crate) fn sync_members(&mut self, input: InputId, now: EngineTime) {
        if !matches!(self.input(input).kind, Kind::Dictionary(_) | Kind::Set(_)) {
            return;
        }
        self.touch_input(input, now);
        let source = self.input(input).source;
        let sampled = self.input(input).sampled_at == now && now != EngineTime::NEVER;
        let old: Vec<_> = self.input(input).members.live.keys().copied().collect();
        for key in old {
            if source.is_none_or(|s| self.child_output(s, key).is_none()) {
                self.sync_key(input, key, None, now, sampled);
            }
        }
        if let Some(source) = source {
            let live: Vec<_> = self
                .output(source)
                .members
                .live
                .iter()
                .map(|(&key, &output)| (key, output))
                .collect();
            for (key, output) in live {
                self.sync_key(input, key, Some(output), now, sampled);
            }
        } else {
            self.endpoints.inputs[input.0 as usize].withdrawal = now;
        }
    }
    fn ensure_child_input(&mut self, input: InputId, key: i64) -> InputId {
        if let Some(child) = self.child_input(input, key) {
            return child;
        }
        let child = self.endpoints.inputs[input.0 as usize]
            .members
            .removed
            .remove(&key)
            .unwrap_or_else(|| {
                let i = self.input(input);
                let (owner, active, scope) = (i.owner, i.active, i.scope);
                let kind = i
                    .kind
                    .member()
                    .unwrap_or_else(|| unreachable!("membership input"))
                    .clone();
                let previous = self.enter_scope(scope);
                let child = self.add_input(owner, kind, active);
                self.enter_scope(previous);
                child
            });
        let m = &mut self.endpoints.inputs[input.0 as usize].members;
        m.initial.entry(key).or_insert(false);
        m.live.insert(key, child);
        child
    }
    pub(crate) fn clear_members(&mut self) {
        while let Some(id) = self.dirty_outputs.pop() {
            let m = &mut self.endpoints.outputs[id.0 as usize].members;
            m.removed.clear();
            m.initial.clear();
            m.changed.clear();
        }
        while let Some(id) = self.dirty_inputs.pop() {
            while let Some((_, child)) = self.endpoints.inputs[id.0 as usize]
                .members
                .removed
                .pop_first()
            {
                self.release_input(child);
            }
            let m = &mut self.endpoints.inputs[id.0 as usize].members;
            m.initial.clear();
            m.changed.clear();
        }
    }
    /// A current binding, excluding retained removed-member projections.
    pub fn has_peer(&self, input: InputId) -> bool {
        self.endpoints.has_peer(input)
    }
    /// Reference to an input's current source, without copying the value.
    pub fn input_reference(&self, input: InputId) -> Reference {
        let i = self.input(input);
        if matches!(i.kind, Kind::Reference(_)) {
            i.source.map_or_else(
                || {
                    self.resolve(i.designation)
                        .map_or_else(Reference::default, |o| self.reference(o))
                },
                |o| self.reference_value(o),
            )
        } else {
            i.source.map_or(i.designation, |o| self.reference(o))
        }
    }
}
