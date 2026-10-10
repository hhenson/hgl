//! Owning boxes use wiring-selected layouts; encoded data contains no borrowed handles.
use crate::{NodeError, NodeResult};
use std::{
    any::TypeId,
    cmp::Ordering,
    hash::{Hash, Hasher},
};

/// An exact concrete layout and its declared value capabilities.
#[derive(Debug)]
pub struct AnyPlan {
    /// Position in the generated, closed descriptor table.
    pub index: usize,
    /// Canonical nominal marker identity, independent of its native representation.
    pub identity: fn() -> TypeId,
    /// Precompiled operations over this layout.
    pub operation: AnyOperation,
    /// Equality, hash and order capability declarations.
    pub capabilities: [bool; 3],
}
/// A field's existing optional presence is part of its owning data.
#[derive(Debug)]
pub struct AnyField {
    /// Concrete child layout.
    pub plan: &'static AnyPlan,
    /// Whether a presence bit precedes the child frame.
    pub optional: bool,
}
/// Storage operations selected during compilation, never inferred from payload tags.
#[derive(Debug)]
pub enum AnyOperation {
    /// Operations supplied by one checked provider over its opaque owning encoding.
    Native {
        /// Provider equality.
        equal: fn(&[u8], &[u8]) -> NodeResult<bool>,
        /// Provider hash.
        hash: fn(&[u8]) -> NodeResult<u64>,
        /// Provider partial order.
        compare: fn(&[u8], &[u8]) -> NodeResult<Option<Ordering>>,
    },
    /// Signed integer or enum member.
    Integer,
    /// IEEE floating-point value, preserving NaN and signed-zero semantics.
    Float,
    /// Exact octets, including text and bool encodings.
    Octets,
    /// Declaration-ordered independent fields.
    Fields(&'static [AnyField]),
    /// Complete list or unordered set/map entries.
    Sequence(&'static AnyPlan, bool),
    /// A closed abstract family retains its concrete member.
    Family(&'static [&'static AnyPlan]),
    /// A nested box selects an already compiled descriptor.
    Boxed(&'static [&'static AnyPlan]),
    /// Owning data with no equality, hash or order, such as a structural delta.
    Data,
}
/// Complete retained bytes and the concrete layout selected before publication.
#[derive(Debug, Default, Clone)]
pub struct AnyValue {
    plan: Option<&'static AnyPlan>,
    data: Vec<u8>,
}
impl AnyValue {
    /// Install independently owned data produced by its statically selected codec.
    pub fn from_encoded(plan: &'static AnyPlan, data: Vec<u8>) -> Self {
        Self {
            plan: Some(plan),
            data,
        }
    }
    /// Read the descriptor without inspecting or unboxing source values.
    pub fn plan(&self) -> Option<&'static AnyPlan> {
        self.plan
    }
    /// Borrow encoded owning storage within the consumer's existing lifetime.
    pub fn data(&self) -> &[u8] {
        &self.data
    }
    /// Reserve owning capacity before hooks execute.
    pub fn reserve(&mut self, size: usize) -> NodeResult {
        self.data
            .try_reserve(size.saturating_sub(self.data.len()))
            .map_err(|error| NodeError::new(error.to_string()))
    }
    /// Prepared width is the complete encoded size.
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }
    /// Copy only after the caller has checked the complete prepared capacity.
    pub fn copy_prepared(&mut self, source: &Self) {
        self.data.clear();
        self.data.extend_from_slice(&source.data);
        self.plan = source.plan;
    }
    /// Assemble a completely preflighted publication in reserved owning capacity.
    pub fn encode_prepared(
        &mut self,
        plan: Option<&'static AnyPlan>,
        encode: impl FnOnce(&mut Vec<u8>),
    ) {
        self.data.clear();
        encode(&mut self.data);
        self.plan = plan;
    }
    /// Retain independently with allocation failures on the ordinary error channel.
    pub fn retain(&self) -> NodeResult<Self> {
        let mut retained = Self::default();
        retained.reserve(self.data.len())?;
        retained.copy_prepared(self);
        Ok(retained)
    }
    /// Compare concrete identity before requesting contained equality.
    pub fn equal(&self, other: &Self) -> NodeResult<bool> {
        match (self.plan, other.plan) {
            (None, None) => Ok(true),
            (Some(a), Some(b)) if (a.identity)() == (b.identity)() => {
                a.equal(&self.data, &other.data)
            }
            _ => Ok(false),
        }
    }
    /// Empty precedes populated; different concrete types remain unordered.
    pub fn compare(&self, other: &Self) -> NodeResult<Option<Ordering>> {
        match (self.plan, other.plan) {
            (None, None) => Ok(Some(Ordering::Equal)),
            (None, Some(_)) => Ok(Some(Ordering::Less)),
            (Some(_), None) => Ok(Some(Ordering::Greater)),
            (Some(a), Some(b)) if (a.identity)() == (b.identity)() => {
                a.compare(&self.data, &other.data)
            }
            _ => Ok(None),
        }
    }
    /// Validate every contained capability and floating key restriction before key use.
    pub fn key_hash(&self) -> NodeResult<u64> {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        if let Some(plan) = self.plan {
            (plan.identity)().hash(&mut hash);
            plan.hash(&self.data)?.hash(&mut hash);
        }
        Ok(hash.finish())
    }
}
fn capability<T>() -> NodeResult<T> {
    Err(NodeError::coded(
        "contained value lacks the required capability",
        "value.capability",
    ))
}
fn number(data: &[u8]) -> u64 {
    u64::from_le_bytes(
        data.try_into()
            .unwrap_or_else(|_| unreachable!("compiled scalar layout")),
    )
}
fn index(value: u64) -> usize {
    usize::try_from(value).unwrap_or_else(|_| unreachable!("compiled descriptor index"))
}
/// Borrow complete child frames at cold capture boundaries.
#[derive(Debug)]
pub struct Frames<'a>(pub &'a [u8]);
impl<'a> Iterator for Frames<'a> {
    type Item = &'a [u8];
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.is_empty() {
            return None;
        }
        let size = usize::try_from(number(&self.0[..8]))
            .unwrap_or_else(|_| unreachable!("compiled frame size"));
        let value = &self.0[8..8 + size];
        self.0 = &self.0[8 + size..];
        Some(value)
    }
}
impl AnyPlan {
    /// Delegate equality through the precompiled descendant layout.
    pub fn equal(&self, a: &[u8], b: &[u8]) -> NodeResult<bool> {
        if !self.capabilities[0] {
            return capability();
        }
        match &self.operation {
            AnyOperation::Native { equal, .. } => equal(a, b),
            AnyOperation::Float => Ok(f64::from_bits(number(a))
                .partial_cmp(&f64::from_bits(number(b)))
                == Some(Ordering::Equal)),
            AnyOperation::Integer | AnyOperation::Octets => Ok(a == b),
            AnyOperation::Fields(fields) => {
                let (mut left, mut right) = (Frames(a), Frames(b));
                for field in *fields {
                    if !field.equal(
                        left.next()
                            .unwrap_or_else(|| unreachable!("compiled field")),
                        right
                            .next()
                            .unwrap_or_else(|| unreachable!("compiled field")),
                    )? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            AnyOperation::Sequence(child, unordered) => child.sequence_equal(a, b, *unordered),
            AnyOperation::Family(members) => {
                let (left, right) = (number(&a[..8]), number(&b[..8]));
                Ok(left == right && members[index(left)].equal(&a[8..], &b[8..])?)
            }
            AnyOperation::Boxed(plans) => {
                let (left, right) = (number(&a[..8]), number(&b[..8]));
                if left == 0 || right == 0 {
                    return Ok(left == right);
                }
                let (left, right) = (plans[index(left) - 1], plans[index(right) - 1]);
                Ok((left.identity)() == (right.identity)() && left.equal(&a[8..], &b[8..])?)
            }
            AnyOperation::Data => capability(),
        }
    }
    fn sequence_equal(&self, a: &[u8], b: &[u8], unordered: bool) -> NodeResult<bool> {
        if Frames(a).count() != Frames(b).count() {
            return Ok(false);
        }
        if unordered {
            for left in Frames(a) {
                if !Frames(b).try_fold(false, |found, right| -> NodeResult<bool> {
                    Ok(found || self.equal(left, right)?)
                })? {
                    return Ok(false);
                }
            }
        } else {
            for (left, right) in Frames(a).zip(Frames(b)) {
                if !self.equal(left, right)? {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
    /// Return partial order without type-name ordering or numeric coercion.
    pub fn compare(&self, a: &[u8], b: &[u8]) -> NodeResult<Option<Ordering>> {
        if !self.capabilities[2] {
            return capability();
        }
        match &self.operation {
            AnyOperation::Native { compare, .. } => compare(a, b),
            AnyOperation::Integer => Ok(Some(
                i64::from_le_bytes(number(a).to_le_bytes())
                    .cmp(&i64::from_le_bytes(number(b).to_le_bytes())),
            )),
            AnyOperation::Float => {
                Ok(f64::from_bits(number(a)).partial_cmp(&f64::from_bits(number(b))))
            }
            AnyOperation::Octets => Ok(Some(a.cmp(b))),
            AnyOperation::Fields(fields) => {
                for ((left, right), field) in Frames(a).zip(Frames(b)).zip(*fields) {
                    let order = field.compare(left, right)?;
                    if order != Some(Ordering::Equal) {
                        return Ok(order);
                    }
                }
                Ok(Some(Ordering::Equal))
            }
            AnyOperation::Sequence(child, false) => {
                for (left, right) in Frames(a).zip(Frames(b)) {
                    let order = child.compare(left, right)?;
                    if order != Some(Ordering::Equal) {
                        return Ok(order);
                    }
                }
                Ok(Some(Frames(a).count().cmp(&Frames(b).count())))
            }
            AnyOperation::Family(members) => {
                let (left, right) = (number(&a[..8]), number(&b[..8]));
                if left != right {
                    return Ok(None);
                }
                members[index(left)].compare(&a[8..], &b[8..])
            }
            AnyOperation::Boxed(plans) => {
                let (left, right) = (number(&a[..8]), number(&b[..8]));
                if left == 0 || right == 0 {
                    return Ok(Some(left.cmp(&right)));
                }
                let (left, right) = (plans[index(left) - 1], plans[index(right) - 1]);
                if (left.identity)() != (right.identity)() {
                    return Ok(None);
                }
                left.compare(&a[8..], &b[8..])
            }
            AnyOperation::Sequence(_, true) | AnyOperation::Data => capability(),
        }
    }
    /// Hash complete contents; unordered containers combine member hashes independently.
    pub fn hash(&self, data: &[u8]) -> NodeResult<u64> {
        if !self.capabilities[0] || !self.capabilities[1] {
            return capability();
        }
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        match &self.operation {
            AnyOperation::Native { hash, .. } => return hash(data),
            AnyOperation::Float => {
                let value = f64::from_bits(number(data));
                if value.is_nan() {
                    return Err(NodeError::new(
                        "NaN collection keys and members are unsupported",
                    ));
                }
                (if value == 0.0 { 0 } else { value.to_bits() }).hash(&mut hash);
            }
            AnyOperation::Integer | AnyOperation::Octets => data.hash(&mut hash),
            AnyOperation::Fields(fields) => {
                for (field, data) in fields.iter().zip(Frames(data)) {
                    field.hash(data)?.hash(&mut hash);
                }
            }
            AnyOperation::Sequence(child, unordered) => {
                let mut combined = 0;
                for data in Frames(data) {
                    let member = child.hash(data)?;
                    if *unordered {
                        combined ^= member;
                    } else {
                        member.hash(&mut hash);
                    }
                }
                if *unordered {
                    combined.hash(&mut hash);
                }
            }
            AnyOperation::Family(members) => {
                let member = number(&data[..8]);
                member.hash(&mut hash);
                members[index(member)].hash(&data[8..])?.hash(&mut hash);
            }
            AnyOperation::Boxed(plans) => {
                let id = number(&data[..8]);
                if id != 0 {
                    let plan = plans[index(id) - 1];
                    (plan.identity)().hash(&mut hash);
                    plan.hash(&data[8..])?.hash(&mut hash);
                }
            }
            AnyOperation::Data => return capability(),
        }
        Ok(hash.finish())
    }
}
impl AnyField {
    fn parts<'a>(&self, data: &'a [u8]) -> Option<&'a [u8]> {
        if !self.optional {
            Some(data)
        } else if data[0] == 0 {
            None
        } else {
            Some(&data[1..])
        }
    }
    fn equal(&self, a: &[u8], b: &[u8]) -> NodeResult<bool> {
        match (self.parts(a), self.parts(b)) {
            (None, None) => Ok(true),
            (Some(a), Some(b)) => self.plan.equal(a, b),
            _ => Ok(false),
        }
    }
    fn compare(&self, a: &[u8], b: &[u8]) -> NodeResult<Option<Ordering>> {
        match (self.parts(a), self.parts(b)) {
            (None, None) => Ok(Some(Ordering::Equal)),
            (None, Some(_)) => Ok(Some(Ordering::Less)),
            (Some(_), None) => Ok(Some(Ordering::Greater)),
            (Some(a), Some(b)) => self.plan.compare(a, b),
        }
    }
    fn hash(&self, data: &[u8]) -> NodeResult<u64> {
        self.parts(data).map_or(Ok(0), |data| {
            self.plan.hash(data).map(|hash| hash.wrapping_add(1))
        })
    }
}

/// A statically selected encoded sink, either preflight counting or reserved storage.
pub trait AnyBuffer {
    /// Current complete encoded width.
    fn len(&self) -> usize;
    /// Whether no encoded data has been written.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Append exact data; storage callers preflight capacity before entering the codec.
    fn write(&mut self, bytes: &[u8]);
    /// Finish a frame's width at its reserved header.
    fn width(&mut self, index: usize, width: usize);
    /// Retain a complete child frame without temporary buffers.
    fn frame(&mut self, encode: impl FnOnce(&mut Self))
    where
        Self: Sized,
    {
        let start = self.len();
        self.write(&[0; 8]);
        encode(self);
        self.width(start, self.len() - start - 8);
    }
}
impl AnyBuffer for Vec<u8> {
    fn len(&self) -> usize {
        Vec::len(self)
    }
    fn write(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
    fn width(&mut self, index: usize, width: usize) {
        self[index..index + 8].copy_from_slice(&(width as u64).to_le_bytes());
    }
}
/// Preflight sink used before mutating an owning destination.
#[derive(Debug, Default)]
pub struct AnyWidth(pub usize);
impl AnyBuffer for AnyWidth {
    fn len(&self) -> usize {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        self.0 += bytes.len();
    }
    fn width(&mut self, _: usize, _: usize) {}
}
