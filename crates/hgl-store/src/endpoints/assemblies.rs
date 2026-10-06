//! Structural records outlive their creating scope only while explicitly in use.
use crate::endpoints::{Kind, Reference};
use std::collections::HashMap;

type Key = (Kind, Vec<Reference>);
#[derive(Debug)]
struct Slot {
    generation: u32,
    key: Option<Key>,
    users: usize,
}

/// Generation-checked interning with ownership only on structural transitions.
#[derive(Debug, Default)]
pub struct Assemblies {
    slots: Vec<Slot>,
    free: Vec<usize>,
    index: HashMap<Key, usize>,
}
impl Assemblies {
    /// Share an identical tree and record at most one construction claim per scope.
    pub fn intern(
        &mut self,
        kind: Kind,
        children: Vec<Reference>,
        claims: &mut Vec<Reference>,
    ) -> Reference {
        let key = (kind, children);
        let n = if let Some(&n) = self.index.get(&key) {
            n
        } else {
            for &child in &key.1 {
                self.retain(child);
            }
            let n = self.free.pop().unwrap_or(self.slots.len());
            if n == self.slots.len() {
                self.slots.push(Slot {
                    generation: 1,
                    key: None,
                    users: 0,
                });
            }
            self.slots[n].key = Some(key.clone());
            self.index.insert(key, n);
            n
        };
        let reference = Reference {
            items: Some(n),
            generation: self.slots[n].generation,
            output: None,
        };
        if !claims.contains(&reference) {
            self.retain(reference);
            claims.push(reference);
        }
        reference
    }
    /// Expired assembly handles resolve to nothing, including after slot reuse.
    pub fn get(&self, reference: Reference) -> Option<&Key> {
        self.slots
            .get(reference.items?)
            .filter(|s| s.generation == reference.generation)?
            .key
            .as_ref()
    }
    /// Retain only assembly metadata, never the referenced endpoints.
    pub fn retain(&mut self, reference: Reference) {
        if self.get(reference).is_some() {
            self.slots[reference.items.unwrap_or_else(|| unreachable!())].users += 1;
        }
    }
    /// Release recursively once no construction scope, binding or carrier needs it.
    pub fn release(&mut self, reference: Reference) {
        if self.get(reference).is_none() {
            return;
        }
        let n = reference.items.unwrap_or_else(|| unreachable!());
        let slot = &mut self.slots[n];
        debug_assert!(slot.users > 0);
        slot.users -= 1;
        if slot.users != 0 {
            return;
        }
        let key = slot.key.take().unwrap_or_else(|| unreachable!());
        self.index.remove(&key);
        if let Some(generation) = slot.generation.checked_add(1) {
            slot.generation = generation;
            self.free.push(n);
        }
        for child in key.1 {
            self.release(child);
        }
    }
    /// Acquire first so an enclosing tree may safely replace one of its children.
    pub fn replace(&mut self, old: Reference, new: Reference) {
        if old != new {
            self.retain(new);
            self.release(old);
        }
    }
    /// Retained slots and live records, including nested assemblies.
    pub fn counts(&self) -> [usize; 2] {
        [self.slots.len(), self.index.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hgl_types::ScalarType;

    #[test]
    fn exhausted_generation_retires_slot_permanently() {
        let mut arena = Assemblies::default();
        let mut claims = Vec::new();
        let kind = Kind::List(Box::new(Kind::Ts(ScalarType::I64)), 0);
        let mut last = arena.intern(kind.clone(), Vec::new(), &mut claims);
        arena.slots[0].generation = u32::MAX;
        last.generation = u32::MAX;
        arena.release(last);
        let next = arena.intern(kind, Vec::new(), &mut Vec::new());
        assert_ne!(last.items, next.items);
        assert!(arena.get(last).is_none());
        assert_eq!(arena.counts(), [2, 1]);
    }
}
