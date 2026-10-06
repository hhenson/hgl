//! Capacity-preserving independent scalar copies for finite prepared evaluation.
use hgl_types::{NodeError, NodeResult};
/// Owned scalar storage with explicit cold capacity and prevalidated hot copying.
pub trait ScalarCopy: Clone + Default {
    /// Bytes required by the owning variable-sized part.
    fn size(&self) -> usize {
        0
    }
    /// Bytes reserved for the owning variable-sized part.
    fn capacity(&self) -> usize {
        0
    }
    /// Reserve a maximum size before evaluation.
    fn reserve(&mut self, _: usize) -> NodeResult {
        Ok(())
    }
    /// Copy after verifying the destination capacity fits the complete value.
    fn copy_from(&mut self, source: &Self) {
        self.clone_from(source);
    }
}
impl ScalarCopy for bool {}
impl ScalarCopy for i64 {}
impl ScalarCopy for f64 {}
impl ScalarCopy for hgl_types::Date {}
impl ScalarCopy for hgl_types::Time {}
impl ScalarCopy for hgl_types::EngineTime {}
impl ScalarCopy for hgl_types::EngineDelta {}
impl ScalarCopy for hgl_types::CivilDateTime {}
impl ScalarCopy for String {
    fn size(&self) -> usize {
        self.len()
    }
    fn capacity(&self) -> usize {
        self.capacity()
    }
    fn reserve(&mut self, size: usize) -> NodeResult {
        self.try_reserve(size.saturating_sub(self.len()))
            .map_err(|e| NodeError::new(e.to_string()))
    }
    fn copy_from(&mut self, source: &Self) {
        self.clear();
        self.push_str(source);
    }
}
impl ScalarCopy for hgl_types::ZoneId {
    fn size(&self) -> usize {
        self.as_str().len()
    }
    fn capacity(&self) -> usize {
        self.name_capacity()
    }
    fn reserve(&mut self, size: usize) -> NodeResult {
        self.reserve_name(size)
            .map_err(|e| NodeError::new(e.to_string()))
    }
    fn copy_from(&mut self, source: &Self) {
        self.copy_prepared(source);
    }
}
impl ScalarCopy for hgl_types::ZonedTime {
    fn size(&self) -> usize {
        self.zone().as_str().len()
    }
    fn capacity(&self) -> usize {
        self.name_capacity()
    }
    fn reserve(&mut self, size: usize) -> NodeResult {
        self.reserve_name(size)
            .map_err(|e| NodeError::new(e.to_string()))
    }
    fn copy_from(&mut self, source: &Self) {
        self.copy_prepared(source);
    }
}
impl ScalarCopy for hgl_types::ZonedDateTime {
    fn size(&self) -> usize {
        self.zone().as_str().len()
    }
    fn capacity(&self) -> usize {
        self.name_capacity()
    }
    fn reserve(&mut self, size: usize) -> NodeResult {
        self.reserve_name(size)
            .map_err(|e| NodeError::new(e.to_string()))
    }
    fn copy_from(&mut self, source: &Self) {
        self.copy_prepared(source);
    }
}
