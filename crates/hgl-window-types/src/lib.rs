//! Exact resolved rolling bounds shared by source shapes and cold runtime schemas.
/// Tick count or elapsed microseconds; kinds never implicitly convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WindowKind {
    /// Count arrivals.
    Ticks,
    /// Retain arrivals within an inclusive age bound.
    Duration,
}
/// Canonical positive maximum and admitted minimum of one exact kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Window {
    kind: WindowKind,
    max: i64,
    min: i64,
}
impl Window {
    /// Check resolved size identity before any endpoint is constructed.
    pub fn new(kind: WindowKind, max: i64, min: i64) -> Result<Self, String> {
        if max <= 0 || min < 0 || min > max || (kind == WindowKind::Ticks && min == 0) {
            return Err(
                "rolling requires positive maximum, admitted minimum and Min <= Max".into(),
            );
        }
        Ok(Self { kind, max, min })
    }
    /// Read the statically selected size kind.
    pub fn kind(self) -> WindowKind {
        self.kind
    }
    /// Maximum count or duration in microseconds.
    pub fn maximum(self) -> i64 {
        self.max
    }
    /// Minimum count or retained span in microseconds.
    pub fn minimum(self) -> i64 {
        self.min
    }
    /// Canonical bounds after ordinary size-expression normalization.
    pub fn parse(max: &str, min: Option<&str>) -> Option<Self> {
        let part = |value: &str| {
            if let Some(value) = value.strip_suffix("us") {
                Some((WindowKind::Duration, value.parse().ok()?))
            } else {
                Some((WindowKind::Ticks, value.parse().ok()?))
            }
        };
        let (kind, max) = part(max)?;
        let (minimum_kind, min) = min.map_or(Some((kind, max)), part)?;
        (kind == minimum_kind)
            .then(|| Self::new(kind, max, min).ok())
            .flatten()
    }
    /// Canonical source arguments with explicit minimum and exact size kind.
    pub fn source_name(self) -> String {
        let suffix = if self.kind == WindowKind::Duration {
            "us"
        } else {
            ""
        };
        format!("{}{suffix},{}{suffix}", self.max, self.min)
    }
}
