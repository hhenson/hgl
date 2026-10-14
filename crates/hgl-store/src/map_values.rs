//! Reusable hash indexes preserve ordinary dictionary entry ownership and order.
use hgl_types::{NodeError, NodeResult};
use std::hash::{DefaultHasher, Hash, Hasher};
/// Hash a statically selected ordinary key representation.
pub fn hash<T: Hash + ?Sized>(value: &T) -> u64 {
    let mut state = DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}
/// Exact composite keys combine ordered field hashes and presence.
pub fn combine(parts: &[u64]) -> u64 {
    hash(parts)
}
/// Index metadata contains no owning key or payload copies.
#[derive(Debug, Default)]
pub struct Index {
    hashes: Vec<u64>,
    buckets: Vec<usize>,
}
impl Index {
    /// Provision a complete finite dictionary before execution, preserving existing entries.
    pub fn prepare(&mut self, capacity: usize) -> NodeResult {
        let capacity = capacity.max(self.hashes.len());
        self.hashes
            .try_reserve(capacity.saturating_sub(self.hashes.len()))
            .map_err(|e| NodeError::new(e.to_string()))?;
        let buckets = capacity
            .checked_mul(2)
            .and_then(|v| v.checked_add(1))
            .and_then(usize::checked_next_power_of_two)
            .ok_or_else(|| NodeError::new("dictionary index capacity overflow"))?;
        self.buckets
            .try_reserve(buckets.saturating_sub(self.buckets.len()))
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.buckets.resize(buckets.max(self.buckets.len()), 0);
        self.rebuild();
        Ok(())
    }
    /// Retain independent metadata at an ordinary owning boundary.
    pub fn retained(&self) -> NodeResult<Self> {
        let mut result = Self::default();
        result.prepare(self.hashes.len())?;
        result.copy(self);
        Ok(result)
    }
    /// Copy preflighted metadata without allocating or invoking providers.
    /// # Panics
    /// The destination must have been prepared for the source length.
    pub fn copy(&mut self, source: &Self) {
        assert!(
            self.hashes.capacity() >= source.hashes.len(),
            "dictionary index was not prepared"
        );
        self.hashes.clear();
        self.hashes.extend_from_slice(&source.hashes);
        self.rebuild();
    }
    /// Construct the index once from already retained keys.
    pub fn from_hashes(hashes: Vec<u64>) -> NodeResult<Self> {
        let mut index = Self::default();
        index.prepare(hashes.len())?;
        index.hashes = hashes;
        index.rebuild();
        Ok(index)
    }
    fn rebuild(&mut self) {
        self.buckets.fill(0);
        for (position, &hash) in self.hashes.iter().enumerate() {
            let mut bucket = Self::bucket(hash, self.buckets.len());
            while self.buckets[bucket] != 0 {
                bucket = (bucket + 1) & (self.buckets.len() - 1);
            }
            self.buckets[bucket] = position + 1;
        }
    }
    fn bucket(hash: u64, count: usize) -> usize {
        usize::try_from(hash & (count - 1) as u64)
            .unwrap_or_else(|_| unreachable!("masked bucket index"))
    }
    /// Probe one hash bucket; equality remains authoritative for collisions.
    pub fn find(
        &self,
        hash: u64,
        mut equal: impl FnMut(usize) -> NodeResult<bool>,
    ) -> NodeResult<Option<usize>> {
        if self.hashes.is_empty() {
            return Ok(None);
        }
        let mut bucket = Self::bucket(hash, self.buckets.len());
        loop {
            let encoded = self.buckets[bucket];
            if encoded == 0 {
                return Ok(None);
            }
            let position = encoded - 1;
            if self.hashes[position] == hash && equal(position)? {
                return Ok(Some(position));
            }
            bucket = (bucket + 1) & (self.buckets.len() - 1);
        }
    }
}
/// An independently owned dictionary retains its index with its ordered entries.
#[derive(Debug)]
pub struct MapValue<T> {
    entries: Vec<T>,
    index: Index,
}
impl<T> MapValue<T> {
    /// Retain an index already checked during complete key construction.
    pub fn from_parts(entries: Vec<T>, index: Index) -> Self {
        Self { entries, index }
    }
    /// Construct exact key hashes once, in ordinary construction order.
    pub fn new(entries: Vec<T>, mut hash: impl FnMut(&T) -> NodeResult<u64>) -> NodeResult<Self> {
        let mut hashes = Vec::new();
        hashes
            .try_reserve(entries.len())
            .map_err(|e| NodeError::new(e.to_string()))?;
        for entry in &entries {
            hashes.push(hash(entry)?);
        }
        Ok(Self::from_parts(entries, Index::from_hashes(hashes)?))
    }
    /// Existing homogeneous retention uses the complete entries.
    pub fn entries(&self) -> &Vec<T> {
        &self.entries
    }
    /// Reusable lookup metadata, independent of descendant ownership.
    pub fn index(&self) -> &Index {
        &self.index
    }
    /// Move ownership into its existing typed storage protocol.
    pub fn into_parts(self) -> (Vec<T>, Index) {
        (self.entries, self.index)
    }
}
impl<T> std::ops::Deref for MapValue<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.entries
    }
}
impl<'a, T> IntoIterator for &'a MapValue<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}
impl<T> IntoIterator for MapValue<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
