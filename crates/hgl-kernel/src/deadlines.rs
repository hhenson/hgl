//! An indexed heap: replacing a deadline never retains a stale entry.
use hgl_types::EngineTime;
#[derive(Debug, Default)]
/// Indexed deadlines, with one replaceable entry per reserved slot.
pub struct Deadlines {
    heap: Vec<(EngineTime, usize)>,
    positions: Vec<usize>,
}
impl Deadlines {
    /// Cancel all entries while preserving allocated capacity.
    pub fn clear(&mut self) {
        self.heap.clear();
        self.positions.fill(usize::MAX);
    }
    /// Reserve slots during structural graph changes.
    pub fn reserve(&mut self, slots: usize) {
        if slots <= self.positions.len() {
            return;
        }
        self.positions.resize(slots, usize::MAX);
        self.heap.reserve(slots - self.heap.len());
    }
    /// Earliest deadline and its slot.
    pub fn first(&self) -> Option<(EngineTime, usize)> {
        self.heap.first().copied()
    }
    /// Replace in place; FOREVER cancels the entry.
    pub fn set(&mut self, slot: usize, time: EngineTime) {
        if time == EngineTime::FOREVER {
            self.remove(slot);
            return;
        }
        let p = self.positions[slot];
        if p == usize::MAX {
            self.positions[slot] = self.heap.len();
            self.heap.push((time, slot));
            self.repair(self.heap.len() - 1);
        } else {
            self.heap[p].0 = time;
            self.repair(p);
        }
    }
    /// Cancel the slot, without leaving a stale heap entry.
    pub fn remove(&mut self, slot: usize) {
        let p = self.positions[slot];
        if p == usize::MAX {
            return;
        }
        let last = self.heap.len() - 1;
        self.swap(p, last);
        self.heap.pop();
        self.positions[slot] = usize::MAX;
        if p < self.heap.len() {
            self.repair(p);
        }
    }
    fn swap(&mut self, a: usize, b: usize) {
        self.heap.swap(a, b);
        self.positions[self.heap[a].1] = a;
        self.positions[self.heap[b].1] = b;
    }
    fn repair(&mut self, mut p: usize) {
        while p > 0 {
            let parent = (p - 1) / 2;
            if self.heap[parent] <= self.heap[p] {
                break;
            }
            self.swap(parent, p);
            p = parent;
        }
        loop {
            let left = p * 2 + 1;
            if left >= self.heap.len() {
                break;
            }
            let right = left + 1;
            let child = if right < self.heap.len() && self.heap[right] < self.heap[left] {
                right
            } else {
                left
            };
            if self.heap[p] <= self.heap[child] {
                break;
            }
            self.swap(p, child);
            p = child;
        }
    }
}
