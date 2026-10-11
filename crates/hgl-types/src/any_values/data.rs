use super::{AnyBuffer, AnyPlan, AnyRef, AnyValue, number};
use std::ops::Deref;
#[derive(Debug, Clone)]
pub(super) struct Descriptor {
    pub(super) offset: usize,
    pub(super) plan: &'static AnyPlan,
}
/// A bounded borrow retains the descriptor context of its original owner.
#[derive(Debug, Clone, Copy)]
pub struct AnyView<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) descriptors: &'a [Descriptor],
    pub(super) offset: usize,
}
impl Deref for AnyView<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.bytes
    }
}
impl<'a> AnyView<'a> {
    /// Restrict payload bytes without discarding their descriptor context.
    #[must_use]
    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        let (start, end) = (range.start, range.end);
        Self {
            bytes: &self.bytes[start..end],
            descriptors: self.descriptors,
            offset: self.offset + start,
        }
    }
    /// Read the descriptor retained with a nested box, without registry lookup.
    pub fn boxed(self) -> AnyRef<'a> {
        let index = usize::try_from(number(&self[..8]))
            .unwrap_or_else(|_| unreachable!("compiled descriptor index"));
        AnyRef {
            plan: (index != 0).then(|| self.descriptors[index - 1].plan),
            data: self.slice(8..self.len()),
        }
    }
    /// Retain a cold decoded subvalue with independent bytes and descriptor storage.
    pub fn retained(self) -> AnyValue {
        let mut data = AnyValue::default();
        data.append(self);
        data
    }
}
impl From<Vec<u8>> for AnyValue {
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            data: bytes,
            ..Self::default()
        }
    }
}
impl AnyBuffer for AnyValue {
    fn len(&self) -> usize {
        self.data.len()
    }
    fn write(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }
    fn width(&mut self, index: usize, width: usize) {
        self.data[index..index + 8].copy_from_slice(&(width as u64).to_le_bytes());
    }
    fn descriptor(&mut self, plan: Option<&'static AnyPlan>) {
        let id = plan.map_or(0, |plan| {
            self.descriptors.push(Descriptor {
                offset: self.data.len(),
                plan,
            });
            self.descriptors.len()
        });
        self.write(&(id as u64).to_le_bytes());
    }
}
/// Copy exact descriptor-bearing segments, relocating headers without touching payload bytes twice.
pub(super) fn append(buffer: &mut impl AnyBuffer, view: AnyView<'_>) {
    let start = view.descriptors.partition_point(|d| d.offset < view.offset);
    let mut cursor = 0;
    for descriptor in &view.descriptors[start..] {
        let offset = descriptor.offset - view.offset;
        if offset >= view.len() {
            break;
        }
        buffer.write(&view[cursor..offset]);
        buffer.descriptor(Some(descriptor.plan));
        cursor = offset + 8;
    }
    buffer.write(&view[cursor..]);
}
