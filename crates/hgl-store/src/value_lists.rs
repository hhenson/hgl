//! Active lengths and retained descendant positions for ordinary list descriptors.
use hgl_types::{NodeError, NodeResult};
/// Compile-time interpreted element field positions.
pub type ListData = Vec<Vec<usize>>;
#[derive(Debug, Default)]
struct Entry {
    items: ListData,
    active: Option<usize>,
}
/// Stable descriptor indices; prepared vacant elements remain independently owned.
#[derive(Debug, Default)]
pub struct Lists {
    entries: Vec<Entry>,
    free: Vec<usize>,
}
impl Lists {
    /// Reserve descriptor and reclamation capacity before installation.
    pub fn reserve(&mut self, count: usize) -> NodeResult {
        let extra = count.saturating_sub(self.free.len());
        self.entries
            .try_reserve(extra)
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.free
            .try_reserve(
                self.entries
                    .len()
                    .saturating_add(extra)
                    .saturating_sub(self.free.len()),
            )
            .map_err(|e| NodeError::new(e.to_string()))
    }
    /// Install a dynamic descriptor with all supplied elements active.
    pub fn insert(&mut self, items: ListData) -> usize {
        let value = Entry {
            items,
            active: None,
        };
        if let Some(slot) = self.free.pop() {
            self.entries[slot] = value;
            slot
        } else {
            let slot = self.entries.len();
            self.entries.push(value);
            slot
        }
    }
    /// Active elements only, never prepared vacant positions.
    pub fn get(&self, slot: usize) -> &[Vec<usize>] {
        let value = &self.entries[slot];
        &value.items[..value.active.unwrap_or(value.items.len())]
    }
    /// Dynamic descriptor access, excluded from prepared evaluation.
    /// # Panics
    /// A prepared descriptor cannot use the dynamic mutation protocol.
    pub fn get_mut(&mut self, slot: usize) -> &mut ListData {
        assert!(
            self.entries[slot].active.is_none(),
            "prepared descriptor requires prepared copying"
        );
        &mut self.entries[slot].items
    }
    /// Replace all descendants, returning even inactive prepared positions for reclamation.
    pub fn replace(&mut self, slot: usize, items: ListData) -> ListData {
        self.entries[slot].active = None;
        std::mem::replace(&mut self.entries[slot].items, items)
    }
    /// Recycle a descriptor after typed descendant reclamation.
    pub fn release(&mut self, slot: usize) {
        self.entries[slot].items.clear();
        self.entries[slot].active = None;
        self.free.push(slot);
    }
    /// All cold-prepared positions, including vacant descendants.
    pub fn prepared(&self, slot: usize) -> &[Vec<usize>] {
        &self.entries[slot].items
    }
    /// Publish a checked logical length without changing physical descendants.
    /// # Panics
    /// The caller must preflight the capacity before changing any field.
    pub fn set_len(&mut self, slot: usize, length: usize) {
        assert!(
            length <= self.entries[slot].items.len(),
            "prepared list capacity exceeded"
        );
        self.entries[slot].active = Some(length);
    }
    /// Physical descriptor count, including reusable slots.
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    /// Whether no descriptors have been allocated.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
