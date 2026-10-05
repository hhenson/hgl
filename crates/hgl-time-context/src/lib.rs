//! Construction-only timezone validation against an exact provider catalog.
use hgl_literals::{Literal, TemporalLiteral};
use hgl_time_values::{EngineTime, Time, ZoneId, ZonedDateTime, ZonedTime};
use jiff::{Timestamp, tz::TimeZoneDatabase};
use std::collections::BTreeSet;

/// Provider state retained outside graph execution; constructed values own no provider.
#[derive(Debug)]
pub struct RunContext {
    database: TimeZoneDatabase,
    names: BTreeSet<String>,
}
impl RunContext {
    /// Capture the default host provider and its exact catalog.
    pub fn from_env() -> Result<Self, String> {
        Self::from_database(TimeZoneDatabase::from_env())
    }
    /// Capture Jiff's bundled catalog explicitly, independently of host packaging.
    pub fn from_bundled() -> Result<Self, String> {
        Self::from_database(TimeZoneDatabase::bundled())
    }
    /// Capture the catalog of an explicitly configured provider.
    pub fn from_database(database: TimeZoneDatabase) -> Result<Self, String> {
        let names = database
            .available()
            .map(|name| name.as_str().to_owned())
            .collect::<BTreeSet<_>>();
        if names.is_empty() {
            return Err("timezone provider catalog is empty".into());
        }
        Ok(Self { database, names })
    }
    /// Validate one recipe before start and construct an independently owned scalar.
    pub fn materialize(&mut self, literal: &TemporalLiteral) -> Result<Literal, String> {
        let name = match literal {
            TemporalLiteral::TimeZone(name)
            | TemporalLiteral::ZonedTime { zone: name, .. }
            | TemporalLiteral::ZonedDateTime { zone: name, .. } => name,
        };
        if !self.names.contains(name) {
            return Err(format!(
                "timezone name is absent from exact provider catalog: {name}"
            ));
        }
        let zone = self
            .database
            .get(name)
            .map_err(|error| format!("timezone lookup failed for {name}: {error}"))?;
        let mut owned = String::new();
        owned
            .try_reserve(name.len())
            .map_err(|error| error.to_string())?;
        owned.push_str(name);
        let identity = ZoneId::from_validated_name(owned);
        match literal {
            TemporalLiteral::TimeZone(_) => Ok(Literal::TimeZone(identity)),
            TemporalLiteral::ZonedTime { time_micros, .. } => {
                if !(0..86_400_000_000).contains(time_micros) {
                    return Err("zoned time range: time must be within one day".into());
                }
                Ok(Literal::ZonedTime(ZonedTime::from_validated_parts(
                    Time(*time_micros),
                    identity,
                )))
            }
            TemporalLiteral::ZonedDateTime {
                instant_micros,
                offset_seconds,
                ..
            } => {
                let instant = Timestamp::from_microsecond(*instant_micros)
                    .map_err(|error| format!("zoned datetime range: {error}"))?;
                let actual = zone.to_offset(instant).seconds();
                if actual != *offset_seconds {
                    return Err(format!(
                        "zoned datetime offset mismatch for {name}: supplied {offset_seconds}, provider {actual}"
                    ));
                }
                Ok(Literal::ZonedDateTime(ZonedDateTime::from_validated_parts(
                    EngineTime::from_micros(*instant_micros),
                    identity,
                    actual,
                )))
            }
        }
    }
}
