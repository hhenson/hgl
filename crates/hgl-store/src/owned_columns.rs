//! Reusable owning buffers shared by boxed and opaque ordinary leaves.
use hgl_types::{NodeError, NodeResult};
#[derive(Debug, Default)]
pub(crate) struct Owned<T> {
    values: Vec<T>,
    free: Vec<usize>,
}
impl<T: Default> Owned<T> {
    pub(crate) fn reserve(&mut self, count: usize) -> NodeResult {
        let extra = count.saturating_sub(self.free.len());
        self.values
            .try_reserve(extra)
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.free
            .try_reserve(
                self.values
                    .len()
                    .saturating_add(extra)
                    .saturating_sub(self.free.len()),
            )
            .map_err(|e| NodeError::new(e.to_string()))
    }
    pub(crate) fn insert(&mut self, value: T) -> usize {
        if let Some(slot) = self.free.pop() {
            self.values[slot] = value;
            slot
        } else {
            let slot = self.values.len();
            self.values.push(value);
            slot
        }
    }
    pub(crate) fn release(&mut self, slot: usize) {
        self.values[slot] = T::default();
        self.free.push(slot);
    }
    pub(crate) fn get(&self, slot: usize) -> &T {
        &self.values[slot]
    }
    pub(crate) fn get_mut(&mut self, slot: usize) -> &mut T {
        &mut self.values[slot]
    }
    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
    pub(crate) fn copy(&mut self, from: usize, to: usize, copy: impl FnOnce(&T, &mut T)) {
        if from < to {
            let (left, right) = self.values.split_at_mut(to);
            copy(&left[from], &mut right[0]);
        } else if from > to {
            let (left, right) = self.values.split_at_mut(from);
            copy(&right[0], &mut left[to]);
        }
    }
}
