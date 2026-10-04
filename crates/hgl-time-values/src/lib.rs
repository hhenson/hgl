//! Owned temporal scalar data, independent of host timezone providers.
/// An instant on the UTC timeline, in microseconds. C++: hgraph's `DateTime`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineTime(i64);

/// A length of time, in microseconds. C++: hgraph's `TimeDelta`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EngineDelta(i64);

impl EngineTime {
    /// Before every time: the last modified time of something never modified,
    /// the schedule of a node that is not scheduled. The specification's
    /// `MIN_DT`. As in hgraph it is the epoch, 1970-01-01T00:00:00Z.
    pub const NEVER: Self = Self(0);

    /// The earliest a run may start: [`Self::NEVER`] plus one step. `MIN_ST`.
    pub const MIN_START: Self = Self(Self::NEVER.0 + EngineDelta::STEP.0);

    /// The latest a run may end: [`Self::FOREVER`] minus one step. `MAX_ET`.
    pub const MAX_END: Self = Self(Self::FOREVER.0 - EngineDelta::STEP.0);

    /// After every time: a graph's next scheduled time when nothing is
    /// scheduled. `MAX_DT`. As in hgraph it is 2300-01-01T00:00:00Z, which is
    /// 120,530 days after the epoch.
    pub const FOREVER: Self = Self(120_530 * 86_400 * 1_000_000);

    /// Unchecked: the range `NEVER..=FOREVER` is kept by `checked_add` and by
    /// the engine refusing a run configured outside it, not by this
    /// constructor.
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// Microseconds since the epoch.
    pub const fn micros(self) -> i64 {
        self.0
    }

    /// `None` if the result is outside `NEVER..=FOREVER`, or the addition
    /// overflows. Judged on the result alone: a `self` outside the range may
    /// come back inside it.
    pub fn checked_add(self, delta: EngineDelta) -> Option<Self> {
        let micros = self.0.checked_add(delta.0)?;
        let sum = Self(micros);
        if Self::NEVER <= sum && sum <= Self::FOREVER {
            Some(sum)
        } else {
            None
        }
    }
}

impl EngineDelta {
    /// One microsecond: the smallest gap between two cycles. `MIN_TD`.
    pub const STEP: Self = Self(1);

    /// A length of that many microseconds; negative is backwards.
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }

    /// The length in microseconds.
    pub const fn micros(self) -> i64 {
        self.0
    }
}

/// A calendar date, as days since the Unix epoch.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date(pub i64);
/// A time of day, in microseconds after midnight.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(pub i64);

/// Zone-free wall-clock fields encoded as epoch-relative microseconds.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CivilDateTime(i64);
impl CivilDateTime {
    /// Construct an already validated civil value.
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros)
    }
    /// Civil microseconds without an implied UTC instant.
    pub const fn micros(self) -> i64 {
        self.0
    }
}
/// An exact validated provider catalog name, including alias spelling.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct ZoneId(String);
impl ZoneId {
    /// Move an exact name whose provider membership the caller validated.
    pub fn from_validated_name(name: String) -> Self {
        Self(name)
    }
    /// The exact supplied catalog name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
    /// Retain the name independently with fallible allocation.
    pub fn try_clone(&self) -> Result<Self, std::collections::TryReserveError> {
        let mut name = String::new();
        name.try_reserve(self.0.len())?;
        name.push_str(&self.0);
        Ok(Self(name))
    }
}
/// An instant, exact zone identity, and validated offset at that instant.
#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct ZonedDateTime {
    instant: EngineTime,
    zone: ZoneId,
    offset: i32,
}
impl ZonedDateTime {
    /// Move independently owned parts after provider and offset validation.
    pub fn from_validated_parts(instant: EngineTime, zone: ZoneId, offset_seconds: i32) -> Self {
        Self {
            instant,
            zone,
            offset: offset_seconds,
        }
    }
    /// The absolute instant.
    pub const fn instant(&self) -> EngineTime {
        self.instant
    }
    /// The exact supplied zone identity.
    pub fn zone(&self) -> &ZoneId {
        &self.zone
    }
    /// The provider's offset in seconds at this instant.
    pub const fn offset_seconds(&self) -> i32 {
        self.offset
    }
    /// Retain all identity fields independently with fallible allocation.
    pub fn try_clone(&self) -> Result<Self, std::collections::TryReserveError> {
        Ok(Self::from_validated_parts(
            self.instant,
            self.zone.try_clone()?,
            self.offset,
        ))
    }
}
