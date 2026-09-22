//! Dense fixed projections and reusable structured designations.
use crate::{BindError, Bindings, InputId, Kind, OutputId, Reference};
use hgl_types::EngineTime;

impl Bindings {
    /// A fixed child output by position.
    pub fn fixed_output(&self, id: OutputId, position: usize) -> OutputId {
        self.output(id).fixed[position]
    }
    /// A fixed child view; its identity survives rebinding.
    pub fn fixed_input(&self, id: InputId, position: usize) -> InputId {
        self.input(id).fixed[position]
    }
    /// Attach a newly constructed dense child before publishing the aggregate.
    pub fn append_fixed(&mut self, parent: OutputId, child: OutputId) -> Result<(), BindError> {
        let p = self.output(parent);
        let n = p.fixed.len();
        if n >= p.kind.len()
            || p.kind.child(n) != &self.output(child).kind
            || self.output(child).parent.is_some()
            || !self.owns_child(parent, child)
        {
            return Err(BindError::ShapeMismatch);
        }
        self.endpoints.outputs[child.0 as usize].parent =
            Some((parent, i64::try_from(n).unwrap_or_else(|_| unreachable!())));
        self.endpoints.outputs[parent.0 as usize].fixed.push(child);
        Ok(())
    }
    /// Construct and intern a fixed designation during wiring.
    pub fn items_reference(
        &mut self,
        kind: Kind,
        children: Vec<Reference>,
    ) -> Result<Reference, BindError> {
        if !kind.fixed() || children.len() != kind.len() {
            return Err(BindError::ShapeMismatch);
        }
        for (n, &r) in children.iter().enumerate() {
            self.check_shape(kind.child(n), r)?;
        }
        let scope = self.scope().index;
        Ok(self
            .items
            .intern(kind, children, &mut self.scopes.entries[scope].assemblies))
    }

    pub(crate) fn check_shape(&self, kind: &Kind, r: Reference) -> Result<(), BindError> {
        if r.items.is_some() {
            if self.items.get(r).is_some_and(|(k, _)| k != kind) {
                return Err(BindError::ShapeMismatch);
            }
        } else if let Some(o) = self.resolve(r)
            && &self.output(o).kind != kind
        {
            return Err(BindError::ShapeMismatch);
        }
        Ok(())
    }
    pub(crate) fn check_designation(&self, input: InputId, r: Reference) -> Result<(), BindError> {
        self.check_shape(&self.input(input).kind, r)?;
        if let Some(o) = self.resolve(r) {
            self.check_reference(input, o)?;
        }
        if let Some((_, children)) = self.items.get(r) {
            for (pos, &child) in self.input(input).fixed.iter().enumerate() {
                self.check_designation(child, children[pos])?;
            }
        }
        Ok(())
    }
    pub(crate) fn sync_fixed(&mut self, input: InputId, r: Reference, now: EngineTime) {
        for n in 0..self.input(input).fixed.len() {
            let child = self.input(input).fixed[n];
            let target = if let Some((_, children)) = self.items.get(r) {
                children[n]
            } else {
                self.resolve(r).map_or(Reference::default(), |o| {
                    self.reference(self.fixed_output(o, n))
                })
            };
            self.apply_sample(child, target, now);
        }
    }
    pub(crate) fn apply_sample(&mut self, input: InputId, r: Reference, now: EngineTime) -> bool {
        let source = self.resolve(r);
        if self.input(input).source == source && self.input(input).designation.same_items(r) {
            return false;
        }
        self.detach(input);
        if let Some(o) = source {
            self.attach(input, o);
        }
        self.items.replace(self.input(input).designation, r);
        self.endpoints.inputs[input.0 as usize].designation = r;
        self.endpoints.inputs[input.0 as usize].sampled_at = now;
        self.sync_fixed(input, r, now);
        self.sync_members(input, now);
        self.endpoints.inputs[input.0 as usize].observed_at = EngineTime::NEVER;
        self.refresh(input, now, false);
        true
    }
    pub(crate) fn reset_observation(&mut self, input: InputId) {
        let i = &mut self.endpoints.inputs[input.0 as usize];
        i.sampled_at = EngineTime::NEVER;
        i.observed_at = EngineTime::NEVER;
        for n in 0..self.input(input).fixed.len() {
            self.reset_observation(self.input(input).fixed[n]);
        }
    }
    pub(crate) fn refresh(&mut self, input: InputId, now: EngineTime, event: bool) {
        let old = self.input(input).valid;
        let valid = self.valid(input);
        let parent = self.input(input).parent.map(|(p, _)| p);
        if let Some(p) = parent
            && self.input(p).kind.fixed()
        {
            let p = &mut self.endpoints.inputs[p.0 as usize];
            p.valid_children = p.valid_children + usize::from(valid) - usize::from(old);
        }
        self.endpoints.inputs[input.0 as usize].valid = valid;
        if self.input(input).kind.fixed() {
            if valid {
                let t = if event {
                    now
                } else if let Some(source) = self.input(input).source {
                    self.output(source)
                        .modified_at
                        .max(self.input(input).sampled_at)
                } else {
                    self.input(input)
                        .fixed
                        .iter()
                        .map(|&c| self.last_modified(c))
                        .max()
                        .unwrap_or(EngineTime::NEVER)
                };
                self.endpoints.inputs[input.0 as usize].observed_at = t;
            } else {
                self.reset_observation(input);
            }
        } else if !valid {
            self.endpoints.inputs[input.0 as usize].observed_at = if event
                && old
                && parent.is_some_and(|p| {
                    self.input(p).kind.fixed() && self.input(p).source.is_none() && self.valid(p)
                }) {
                now
            } else {
                EngineTime::NEVER
            };
        }
    }
}
