//! Typed scalar storage, selected at compile time.
//!
//! The value columns, and how code generic over a scalar type reaches the
//! column of that type without deciding anything at run time.

use std::fmt::Debug;

use hgl_types::{ScalarType, ScalarValue};

/// One vector per scalar type. A `TS<f64>` is eight bytes among other `f64`s:
/// no tag, no box, no pointer to chase.
///
/// Public because the sealed scalar operations mention it. Another crate can make one of
/// its own, through `Default`, but never reach a store's: that is a private
/// field.
#[derive(Debug, Default)]
pub struct Columns {
    bools: Vec<bool>,
    i64s: Vec<i64>,
    f64s: Vec<f64>,
    texts: Vec<String>,
    durations: Vec<hgl_types::EngineDelta>,
    datetimes: Vec<hgl_types::EngineTime>,
    times: Vec<hgl_types::Time>,
    dates: Vec<hgl_types::Date>,
    civil_datetimes: Vec<hgl_types::CivilDateTime>,
    zones: Vec<hgl_types::ZoneId>,
    zoned_datetimes: Vec<hgl_types::ZonedDateTime>,
    zoned_times: Vec<hgl_types::ZonedTime>,
}

impl Columns {
    /// The erased read: it decides the type per value, so it is for tests and
    /// tools and never for a node's eval.
    pub fn value(&self, scalar_type: ScalarType, slot: usize) -> ScalarValue {
        match scalar_type {
            ScalarType::Bool => ScalarValue::Bool(self.bools[slot]),
            ScalarType::I64 => ScalarValue::I64(self.i64s[slot]),
            ScalarType::F64 => ScalarValue::F64(self.f64s[slot]),
            ScalarType::Date => ScalarValue::Date(self.dates[slot]),
            ScalarType::Time => ScalarValue::Time(self.times[slot]),
            ScalarType::DateTime => ScalarValue::DateTime(self.datetimes[slot]),
            ScalarType::Duration => ScalarValue::Duration(self.durations[slot]),
            ScalarType::Text => ScalarValue::Text(self.texts[slot].clone()),
            ScalarType::CivilDateTime => ScalarValue::CivilDateTime(self.civil_datetimes[slot]),
            ScalarType::TimeZone => ScalarValue::TimeZone(self.zones[slot].clone()),
            ScalarType::ZonedTime => ScalarValue::ZonedTime(self.zoned_times[slot].clone()),
            ScalarType::ZonedDateTime => {
                ScalarValue::ZonedDateTime(self.zoned_datetimes[slot].clone())
            }
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

/// A scalar the store has a column for: bool, integers, floats, text and calendar
/// values. The seal is a supertrait in a private module
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
/// impl crate::columns::Scalar for Mine {
///     const TYPE: ScalarType = ScalarType::Bool;
///     fn into_value(self) -> ScalarValue {
///         ScalarValue::Bool(true)
///     }
///     fn from_value(_value: ScalarValue) -> Option<Self> {
///         None
///     }
/// }
/// ```
pub trait Scalar: crate::scalar_copy::ScalarCopy + PartialEq + Debug + Column + 'static {
    /// Independently copy an ordinary value, translating allocation failure.
    fn try_clone(&self) -> Result<Self, Box<hgl_types::NodeError>> {
        Ok(self.clone())
    }
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
        if let ScalarValue::Bool(value) = value {
            Some(value)
        } else {
            None
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
        if let ScalarValue::I64(value) = value {
            Some(value)
        } else {
            None
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
        if let ScalarValue::F64(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for String {
    fn column(columns: &Columns) -> &[Self] {
        &columns.texts
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.texts
    }
}
impl Scalar for String {
    fn try_clone(&self) -> Result<Self, Box<hgl_types::NodeError>> {
        let mut owned = Self::new();
        owned
            .try_reserve(self.len())
            .map_err(|error| hgl_types::NodeError::new(error.to_string()))?;
        owned.push_str(self);
        Ok(owned)
    }
    const TYPE: ScalarType = ScalarType::Text;
    fn into_value(self) -> ScalarValue {
        ScalarValue::Text(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::Text(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::Date {
    fn column(columns: &Columns) -> &[Self] {
        &columns.dates
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.dates
    }
}
impl Scalar for hgl_types::Date {
    const TYPE: ScalarType = ScalarType::Date;
    fn into_value(self) -> ScalarValue {
        ScalarValue::Date(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::Date(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::Time {
    fn column(columns: &Columns) -> &[Self] {
        &columns.times
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.times
    }
}
impl Scalar for hgl_types::Time {
    const TYPE: ScalarType = ScalarType::Time;
    fn into_value(self) -> ScalarValue {
        ScalarValue::Time(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::Time(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::EngineTime {
    fn column(columns: &Columns) -> &[Self] {
        &columns.datetimes
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.datetimes
    }
}
impl Scalar for hgl_types::EngineTime {
    const TYPE: ScalarType = ScalarType::DateTime;
    fn into_value(self) -> ScalarValue {
        ScalarValue::DateTime(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::DateTime(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::EngineDelta {
    fn column(columns: &Columns) -> &[Self] {
        &columns.durations
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.durations
    }
}
impl Scalar for hgl_types::EngineDelta {
    const TYPE: ScalarType = ScalarType::Duration;
    fn into_value(self) -> ScalarValue {
        ScalarValue::Duration(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::Duration(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::CivilDateTime {
    fn column(columns: &Columns) -> &[Self] {
        &columns.civil_datetimes
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.civil_datetimes
    }
}
impl Scalar for hgl_types::CivilDateTime {
    const TYPE: ScalarType = ScalarType::CivilDateTime;
    fn into_value(self) -> ScalarValue {
        ScalarValue::CivilDateTime(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::CivilDateTime(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::ZoneId {
    fn column(columns: &Columns) -> &[Self] {
        &columns.zones
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.zones
    }
}
impl Scalar for hgl_types::ZoneId {
    fn try_clone(&self) -> Result<Self, Box<hgl_types::NodeError>> {
        self.try_clone()
            .map_err(|error| hgl_types::NodeError::new(error.to_string()))
    }
    const TYPE: ScalarType = ScalarType::TimeZone;
    fn into_value(self) -> ScalarValue {
        ScalarValue::TimeZone(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::TimeZone(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::ZonedDateTime {
    fn column(columns: &Columns) -> &[Self] {
        &columns.zoned_datetimes
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.zoned_datetimes
    }
}
impl Scalar for hgl_types::ZonedDateTime {
    fn try_clone(&self) -> Result<Self, Box<hgl_types::NodeError>> {
        self.try_clone()
            .map_err(|error| hgl_types::NodeError::new(error.to_string()))
    }
    const TYPE: ScalarType = ScalarType::ZonedDateTime;
    fn into_value(self) -> ScalarValue {
        ScalarValue::ZonedDateTime(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::ZonedDateTime(value) = value {
            Some(value)
        } else {
            None
        }
    }
}

impl Column for hgl_types::ZonedTime {
    fn column(columns: &Columns) -> &[Self] {
        &columns.zoned_times
    }
    fn column_mut(columns: &mut Columns) -> &mut Vec<Self> {
        &mut columns.zoned_times
    }
}
impl Scalar for hgl_types::ZonedTime {
    fn try_clone(&self) -> Result<Self, Box<hgl_types::NodeError>> {
        self.try_clone()
            .map_err(|error| hgl_types::NodeError::new(error.to_string()))
    }
    const TYPE: ScalarType = ScalarType::ZonedTime;
    fn into_value(self) -> ScalarValue {
        ScalarValue::ZonedTime(self)
    }
    fn from_value(value: ScalarValue) -> Option<Self> {
        if let ScalarValue::ZonedTime(value) = value {
            Some(value)
        } else {
            None
        }
    }
}
