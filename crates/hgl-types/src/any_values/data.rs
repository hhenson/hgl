use super::{
    AnyBuffer, AnyField, AnyPlan, AnyRef, AnyValue, NativeCheck, NativeCopy, index, number,
};
use super::{Frames, NodeResult};
use std::{cmp::Ordering, ops::Deref};
/// A bounded borrow retains the descriptor context of its original owner.
#[derive(Debug, Clone, Copy)]
pub struct AnyView<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) descriptors: &'a [&'static AnyPlan],
}
impl Deref for AnyView<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.bytes
    }
}
impl<'a> AnyView<'a> {
    /// Borrow a checked compiled encoding with its independently resolved descriptor context.
    pub fn from_encoded(bytes: &'a [u8], descriptors: &'a [&'static AnyPlan]) -> Self {
        Self { bytes, descriptors }
    }
    /// Restrict payload bytes without discarding their descriptor context.
    #[must_use]
    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            bytes: &self.bytes[range],
            descriptors: self.descriptors,
        }
    }
    /// Borrow an optional compiled field with its original descriptor context.
    pub fn field(self, field: &AnyField) -> AnyRef<'a> {
        AnyRef {
            plan: (!field.optional || self[0] != 0).then_some(field.plan),
            data: if field.optional {
                self.slice(1..self.len())
            } else {
                self
            },
        }
    }
    /// Check an untrusted cold box before its descriptor reaches a trusted codec.
    pub fn checked_boxed(self) -> NodeResult<AnyRef<'a>> {
        let error = || super::NodeError::new("invalid cold box descriptor");
        let bytes = self.get(..8).ok_or_else(error)?;
        let id = usize::try_from(u64::from_le_bytes(
            bytes.try_into().map_err(|_invalid| error())?,
        ))
        .map_err(|_invalid| error())?;
        let plan = if id == 0 {
            if self.len() != 8 {
                return Err(error());
            }
            None
        } else {
            Some(*self.descriptors.get(id - 1).ok_or_else(error)?)
        };
        Ok(AnyRef {
            plan,
            data: self.slice(8..self.len()),
        })
    }
    /// Read the descriptor retained with a nested box, without registry lookup.
    pub fn boxed(self) -> AnyRef<'a> {
        let index = index(number(&self[..8]));
        AnyRef {
            plan: (index != 0).then(|| self.descriptors[index - 1]),
            data: self.slice(8..self.len()),
        }
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
    fn native(&mut self, data: &[u8], copy: NativeCopy, _: NativeCheck) {
        copy(data, self);
    }

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
            self.descriptors.push(plan);
            self.descriptors.len()
        });
        self.write(&(id as u64).to_le_bytes());
    }
}
impl AnyBuffer for Vec<u8> {
    fn native(&mut self, data: &[u8], copy: NativeCopy, _: NativeCheck) {
        copy(data, self);
    }
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn write(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
    fn width(&mut self, index: usize, width: usize) {
        self[index..index + 8].copy_from_slice(&(width as u64).to_le_bytes());
    }
    fn descriptor(&mut self, _: Option<&'static AnyPlan>) {
        unreachable!("native provider writes only opaque bytes")
    }
}

impl<'a> Frames<'a> {
    /// Visit two framed sequences through a caller-selected fallible equality predicate.
    pub fn equal_by(
        self,
        other: Self,
        unordered: bool,
        mut equal: impl FnMut(AnyView<'a>, AnyView<'a>) -> NodeResult<bool>,
    ) -> NodeResult<bool> {
        if Frames(self.0).count() != Frames(other.0).count() {
            return Ok(false);
        }
        if unordered {
            for left in self {
                if !Frames(other.0).try_fold(false, |found, right| -> NodeResult<bool> {
                    Ok(found || equal(left, right)?)
                })? {
                    return Ok(false);
                }
            }
            Ok(true)
        } else {
            for (left, right) in self.zip(other) {
                if !equal(left, right)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
    /// Visit corresponding frames until a caller-selected order differs, then compare lengths.
    pub fn compare_by(
        self,
        other: Self,
        mut compare: impl FnMut(usize, AnyView<'a>, AnyView<'a>) -> NodeResult<Option<Ordering>>,
    ) -> NodeResult<Option<Ordering>> {
        let (left, right) = (self.0, other.0);
        for (index, (left, right)) in self.zip(other).enumerate() {
            let order = compare(index, left, right)?;
            if order != Some(Ordering::Equal) {
                return Ok(order);
            }
        }
        Ok(Some(Frames(left).count().cmp(&Frames(right).count())))
    }
}
