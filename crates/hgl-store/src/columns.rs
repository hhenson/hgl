//! The value columns, and how code generic over a scalar type reaches the
//! column of that type without deciding anything at run time.

use std::fmt::Debug;

use hgl_types::{ScalarType, ScalarValue};

/// One vector per scalar type. A `TS<f64>` is eight bytes among other `f64`s:
/// no tag, no box, no pointer to chase.
///
/// Public only because [`Column`] mentions it. Another crate can make one of
/// its own, through `Default`, but never reach a store's: that is a private
/// field.
#[derive(Debug, Default)]
pub struct Columns {
    bools: Vec<bool>,
    i64s: Vec<i64>,
    f64s: Vec<f64>,
}

impl Columns {
    /// The erased read: it decides the type per value, so it is for tests and
    /// tools and never for a node's eval.
    pub(crate) fn value(&self, scalar_type: ScalarType, slot: usize) -> ScalarValue {
        match scalar_type {
            ScalarType::Bool => ScalarValue::Bool(self.bools[slot]),
            ScalarType::I64 => ScalarValue::I64(self.i64s[slot]),
            ScalarType::F64 => ScalarValue::F64(self.f64s[slot]),
        }
    }
}

/// Seals [`Scalar`], and names the one column that holds `Self`.
///
/// Each impl is a field access, so `T::column(columns)` is chosen when the
/// caller is compiled and costs nothing when it runs. This module is private,
/// so another crate cannot name this trait and so cannot implement it. It can
/// still call these methods through a `T: Scalar` bound, but only on a
/// [`Columns`] of its own making, never a store's.
pub trait Column: Default {
    /// The column holding every value of this type.
    fn column(columns: &Columns) -> &[Self];
    /// The same column, to write a slot or to add one.
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self>;
}

/// A scalar the store has a column for. Sealed: implemented for `bool`, `i64`
/// and `f64`. The seal is a supertrait in a private module
/// (`+ columns::Column`), which another crate cannot implement. Its methods
/// can still be reached through a `T: Scalar` bound, but only on a `Columns`
/// of the caller's own making, never a store's.
///
/// ```compile_fail
/// use hgl_types::{ScalarType, ScalarValue};
///
/// #[derive(Debug, Default, Clone, Copy, PartialEq)]
/// struct Mine;
///
/// // Refused: the store has no column for `Mine`.
/// impl hgl_store::Scalar for Mine {
///     const TYPE: ScalarType = ScalarType::Bool;
///     fn into_value(self) -> ScalarValue {
///         ScalarValue::Bool(true)
///     }
///     fn from_value(_value: ScalarValue) -> Option<Self> {
///         None
///     }
/// }
/// ```
pub trait Scalar: Copy + PartialEq + Debug + Column + 'static {
    /// The run-time name of this type.
    const TYPE: ScalarType;
    /// This value, carrying its type with it.
    fn into_value(self) -> ScalarValue;
    /// `None` if `value` is of another type.
    fn from_value(value: ScalarValue) -> Option<Self>;
}

impl Column for bool {
    #[inline]
    fn column(columns: &Columns) -> &[Self] {
        &columns.bools
    }
    #[inline]
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.bools
    }
}

impl Scalar for bool {
    const TYPE: ScalarType = ScalarType::Bool;
    fn into_value(self) -> ScalarValue {
        ScalarValue::Bool(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::Bool(value) => Some(value),
            ScalarValue::I64(_) | ScalarValue::F64(_) => None,
        }
    }
}

impl Column for i64 {
    #[inline]
    fn column(columns: &Columns) -> &[Self] {
        &columns.i64s
    }
    #[inline]
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.i64s
    }
}

impl Scalar for i64 {
    const TYPE: ScalarType = ScalarType::I64;
    fn into_value(self) -> ScalarValue {
        ScalarValue::I64(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::I64(value) => Some(value),
            ScalarValue::Bool(_) | ScalarValue::F64(_) => None,
        }
    }
}

impl Column for f64 {
    #[inline]
    fn column(columns: &Columns) -> &[Self] {
        &columns.f64s
    }
    #[inline]
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.f64s
    }
}

impl Scalar for f64 {
    const TYPE: ScalarType = ScalarType::F64;
    fn into_value(self) -> ScalarValue {
        ScalarValue::F64(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        match value {
            ScalarValue::F64(value) => Some(value),
            ScalarValue::Bool(_) | ScalarValue::I64(_) => None,
        }
    }
}
