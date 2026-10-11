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

impl ScalarCopy for Vec<u8> {
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
        self.extend_from_slice(source);
    }
}

/// Preflight all octets before preserving a prepared destination's capacity.
pub fn bytes_from_octets(
    destination: &mut Vec<u8>,
    octets: impl Iterator<Item = i64> + Clone,
) -> NodeResult {
    let mut length = 0;
    for octet in octets.clone() {
        if !(0..=255).contains(&octet) {
            return Err(NodeError::coded(
                "byte octet must be between 0 and 255",
                "value.byte_range",
            ));
        }
        length += 1;
    }
    if destination.capacity() < length {
        return Err(NodeError::new("prepared scalar capacity exceeded"));
    }
    destination.clear();
    for octet in octets {
        destination
            .push(u8::try_from(octet).unwrap_or_else(|_| unreachable!("validated byte octet")));
    }
    Ok(())
}
