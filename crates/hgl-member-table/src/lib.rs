//! Membership tables with an optional immutable cold key domain.
use std::collections::BTreeMap;
#[derive(Debug)]
struct Slot<T> {
    key: i64,
    value: Option<T>,
    position: usize,
}
/// Dynamic legacy storage or reusable slots prepared for one finite domain.
#[derive(Debug)]
pub struct Table<T> {
    dynamic: BTreeMap<i64, T>,
    slots: Vec<Slot<T>>,
    active: Vec<usize>,
    prepared: bool,
}
impl<T> Default for Table<T> {
    fn default() -> Self {
        Self {
            dynamic: BTreeMap::new(),
            slots: Vec::new(),
            active: Vec::new(),
            prepared: false,
        }
    }
}
impl<T> Table<T> {
    /// Establish an immutable key domain while the table is empty.
    /// # Panics
    /// The table must have no occupied slots.
    pub fn prepare(&mut self, keys: &[i64]) {
        assert!(self.is_empty(), "prepare membership before publication");
        self.slots = keys
            .iter()
            .map(|&key| Slot {
                key,
                value: None,
                position: 0,
            })
            .collect();
        self.slots.sort_unstable_by_key(|slot| slot.key);
        self.slots.dedup_by_key(|slot| slot.key);
        self.active = Vec::with_capacity(self.slots.len());
        self.prepared = true;
    }
    /// Whether storage has a fixed prepared domain.
    pub fn prepared(&self) -> bool {
        self.prepared
    }
    fn index(&self, key: i64) -> Option<usize> {
        self.slots.binary_search_by_key(&key, |slot| slot.key).ok()
    }
    /// Find an occupied slot.
    pub fn get(&self, key: i64) -> Option<&T> {
        if self.prepared {
            self.slots
                .get(self.index(key)?)
                .and_then(|slot| slot.value.as_ref())
        } else {
            self.dynamic.get(&key)
        }
    }
    /// Whether the key currently has a value.
    pub fn contains_key(&self, key: i64) -> bool {
        self.get(key).is_some()
    }
    /// Number of occupied slots, independent of the prepared domain size.
    pub fn len(&self) -> usize {
        self.dynamic.len() + self.active.len()
    }
    /// Whether no slot is occupied.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Replace one value without allocating on the prepared path.
    pub fn insert(&mut self, key: i64, value: T) -> Option<T> {
        if !self.prepared {
            return self.dynamic.insert(key, value);
        }
        let index = self
            .index(key)
            .unwrap_or_else(|| unreachable!("key outside prepared domain"));
        let slot = &mut self.slots[index];
        if slot.value.is_none() {
            slot.position = self.active.len();
            self.active.push(index);
        }
        slot.value.replace(value)
    }
    /// Retain the first value observed at a key.
    pub fn insert_initial(&mut self, key: i64, value: T) {
        if !self.contains_key(key) {
            self.insert(key, value);
        }
    }
    /// Vacate a slot while preserving its allocation and domain.
    pub fn remove(&mut self, key: i64) -> Option<T> {
        if !self.prepared {
            return self.dynamic.remove(&key);
        }
        let index = self.index(key)?;
        let value = self.slots[index].value.take()?;
        let position = self.slots[index].position;
        self.active.swap_remove(position);
        if let Some(&moved) = self.active.get(position) {
            self.slots[moved].position = position;
        }
        Some(value)
    }
    /// Remove an occupied entry; prepared removal order is unspecified.
    pub fn pop_first(&mut self) -> Option<(i64, T)> {
        if !self.prepared {
            return self.dynamic.pop_first();
        }
        let index = *self.active.last()?;
        let key = self.slots[index].key;
        self.remove(key).map(|value| (key, value))
    }
    /// Clear only occupied slots, retaining all prepared capacity.
    pub fn clear(&mut self) {
        self.dynamic.clear();
        while let Some(index) = self.active.pop() {
            self.slots[index].value = None;
        }
    }
    /// An occupied entry by dense position; prepared lookup does not scan absent keys.
    pub fn item(&self, position: usize) -> Option<(i64, &T)> {
        if !self.prepared {
            return self
                .dynamic
                .iter()
                .nth(position)
                .map(|(&key, value)| (key, value));
        }
        let slot = &self.slots[*self.active.get(position)?];
        slot.value.as_ref().map(|value| (slot.key, value))
    }
    /// Next legacy dynamic entry in key order, without rescanning earlier entries.
    pub fn dynamic_after(&self, previous: Option<i64>) -> Option<(i64, &T)> {
        use std::ops::Bound::{Excluded, Unbounded};
        let lower = previous.map_or(Unbounded, Excluded);
        self.dynamic
            .range((lower, Unbounded))
            .next()
            .map(|(&key, value)| (key, value))
    }
    /// Occupied entries; prepared traversal visits only active slots.
    pub fn iter(&self) -> impl Iterator<Item = (&i64, &T)> {
        self.dynamic.iter().chain(self.active.iter().map(|&index| {
            let slot = &self.slots[index];
            (
                &slot.key,
                slot.value.as_ref().unwrap_or_else(|| unreachable!()),
            )
        }))
    }
    /// Occupied keys in table iteration order.
    pub fn keys(&self) -> impl Iterator<Item = &i64> {
        self.iter().map(|(key, _)| key)
    }
    /// Occupied values in table iteration order.
    pub fn values(&self) -> impl Iterator<Item = &T> {
        self.iter().map(|(_, value)| value)
    }
    /// Move all occupied values out of a retired table.
    pub fn into_values(self) -> impl Iterator<Item = T> {
        self.dynamic
            .into_values()
            .chain(self.slots.into_iter().filter_map(|slot| slot.value))
    }
}
