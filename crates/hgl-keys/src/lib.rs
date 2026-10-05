//! Cold owning key domains with statically selected allocation-free lookup.
use hgl_global_value::GlobalValue;
use hgl_types::{
    CivilDateTime, Date, EngineDelta, EngineTime, NodeError, NodeResult, Time, ZoneId,
    ZonedDateTime, ZonedTime,
};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
/// A prepared exact scalar key representation; endpoint metadata owns nominal identity.
pub trait Key: GlobalValue {
    /// Retain a constant key before graph startup.
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult;
    /// Find its already prepared membership token without allocating.
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64>;
    /// Retain the exact ordinary value during owning capture.
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value>;
    /// Visit the retained exact value without allocating an owning copy.
    fn with_value<R>(keys: &Keys, id: i64, visit: impl FnOnce(&Self::Value) -> R) -> NodeResult<R> {
        Ok(visit(&Self::value(keys, id)?))
    }
    /// Complete finite key domain prepared before graph startup.
    fn ids(keys: &Keys) -> &[i64];
}
#[derive(Debug, Default)]
struct Domain {
    known: HashSet<i64>,
    ids: Vec<i64>,
}
impl Domain {
    fn prepare(&mut self, id: i64) -> NodeResult {
        if self.known.contains(&id) {
            return Ok(());
        }
        self.known
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.ids
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.known.insert(id);
        self.ids.push(id);
        Ok(())
    }
    fn require(&self, id: i64) -> NodeResult<i64> {
        if self.known.contains(&id) {
            Ok(id)
        } else {
            Err(NodeError::new(
                "collection key was not prepared before start",
            ))
        }
    }
}
#[derive(Debug, Default)]
struct Table<T> {
    values: Vec<T>,
    buckets: HashMap<u64, Vec<i64>>,
    ids: Vec<i64>,
}
fn hash<T: Hash>(value: &T) -> u64 {
    let mut state = DefaultHasher::new();
    value.hash(&mut state);
    state.finish()
}
impl<T: PartialEq + Hash> Table<T> {
    fn find(&self, value: &T) -> Option<i64> {
        self.buckets.get(&hash(value))?.iter().copied().find(|&id| {
            self.values[usize::try_from(id).unwrap_or_else(|_| unreachable!("prepared ID"))]
                == *value
        })
    }
    fn prepare(&mut self, value: T) -> NodeResult {
        if self.find(&value).is_some() {
            return Ok(());
        }
        self.values
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.ids
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        self.buckets
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        let id = i64::try_from(self.values.len()).map_err(|e| NodeError::new(e.to_string()))?;
        let bucket = self.buckets.entry(hash(&value)).or_default();
        bucket
            .try_reserve(1)
            .map_err(|e| NodeError::new(e.to_string()))?;
        bucket.push(id);
        self.values.push(value);
        self.ids.push(id);
        Ok(())
    }
    fn require(&self, value: &T) -> NodeResult<i64> {
        self.find(value)
            .ok_or_else(|| NodeError::new("collection key was not prepared before start"))
    }
    fn get(&self, id: i64) -> NodeResult<&T> {
        usize::try_from(id)
            .ok()
            .and_then(|i| self.values.get(i))
            .ok_or_else(|| NodeError::new("unknown prepared key ID"))
    }
}
/// Run-owned exact typed key domains, populated exclusively before graph startup.
#[derive(Debug, Default)]
pub struct Keys {
    bools: Domain,
    integers: Domain,
    floats: Domain,
    dates: Domain,
    times: Domain,
    datetimes: Domain,
    durations: Domain,
    civils: Domain,
    strings: Table<String>,
    zones: Table<ZoneId>,
    zoned_times: Table<ZonedTime>,
    zoned_datetimes: Table<ZonedDateTime>,
}
fn float_id(value: f64) -> NodeResult<i64> {
    if value.is_nan() {
        return Err(NodeError::new("NaN collection keys are unsupported"));
    }
    Ok(i64::from_ne_bytes(
        (if value == 0.0 { 0 } else { value.to_bits() }).to_ne_bytes(),
    ))
}
impl Key for bool {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.bools.prepare(i64::from(*value))
    }
    fn id(_: &Keys, value: &Self::Value) -> NodeResult<i64> {
        Ok(i64::from(*value))
    }
    fn value(_: &Keys, id: i64) -> NodeResult<Self::Value> {
        Ok(id != 0)
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.bools.ids
    }
}
impl Key for i64 {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.integers.prepare(*value)
    }
    fn id(_: &Keys, value: &Self::Value) -> NodeResult<i64> {
        Ok(*value)
    }
    fn value(_: &Keys, id: i64) -> NodeResult<Self::Value> {
        Ok(id)
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.integers.ids
    }
}
impl Key for f64 {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.floats.prepare(float_id(*value)?)
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.floats.require(float_id(*value)?)
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        keys.floats.require(id)?;
        Ok(f64::from_bits(u64::from_ne_bytes(id.to_ne_bytes())))
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.floats.ids
    }
}
impl Key for Date {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.dates.prepare(value.0)
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.dates.require(value.0)
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        keys.dates.require(id)?;
        Ok(Date(id))
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.dates.ids
    }
}
impl Key for Time {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.times.prepare(value.0)
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.times.require(value.0)
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        keys.times.require(id)?;
        Ok(Time(id))
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.times.ids
    }
}
impl Key for EngineTime {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.datetimes.prepare(value.micros())
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.datetimes.require(value.micros())
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        keys.datetimes.require(id)?;
        Ok(EngineTime::from_micros(id))
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.datetimes.ids
    }
}
impl Key for EngineDelta {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.durations.prepare(value.micros())
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.durations.require(value.micros())
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        keys.durations.require(id)?;
        Ok(EngineDelta::from_micros(id))
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.durations.ids
    }
}
impl Key for CivilDateTime {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        keys.civils.prepare(value.micros())
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.civils.require(value.micros())
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        keys.civils.require(id)?;
        Ok(CivilDateTime::from_micros(id))
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.civils.ids
    }
}
impl Key for String {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        if keys.strings.find(value).is_some() {
            return Ok(());
        }
        keys.strings.prepare(<Self as GlobalValue>::retain(value)?)
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.strings.require(value)
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        <Self as GlobalValue>::retain(keys.strings.get(id)?)
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.strings.ids
    }
    fn with_value<R>(keys: &Keys, id: i64, visit: impl FnOnce(&Self::Value) -> R) -> NodeResult<R> {
        Ok(visit(keys.strings.get(id)?))
    }
}
impl Key for ZoneId {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        if keys.zones.find(value).is_some() {
            return Ok(());
        }
        keys.zones.prepare(<Self as GlobalValue>::retain(value)?)
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.zones.require(value)
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        <Self as GlobalValue>::retain(keys.zones.get(id)?)
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.zones.ids
    }
    fn with_value<R>(keys: &Keys, id: i64, visit: impl FnOnce(&Self::Value) -> R) -> NodeResult<R> {
        Ok(visit(keys.zones.get(id)?))
    }
}
impl Key for ZonedTime {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        if keys.zoned_times.find(value).is_some() {
            return Ok(());
        }
        keys.zoned_times
            .prepare(<Self as GlobalValue>::retain(value)?)
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.zoned_times.require(value)
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        <Self as GlobalValue>::retain(keys.zoned_times.get(id)?)
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.zoned_times.ids
    }
    fn with_value<R>(keys: &Keys, id: i64, visit: impl FnOnce(&Self::Value) -> R) -> NodeResult<R> {
        Ok(visit(keys.zoned_times.get(id)?))
    }
}
impl Key for ZonedDateTime {
    fn prepare(keys: &mut Keys, value: &Self::Value) -> NodeResult {
        if keys.zoned_datetimes.find(value).is_some() {
            return Ok(());
        }
        keys.zoned_datetimes
            .prepare(<Self as GlobalValue>::retain(value)?)
    }
    fn id(keys: &Keys, value: &Self::Value) -> NodeResult<i64> {
        keys.zoned_datetimes.require(value)
    }
    fn value(keys: &Keys, id: i64) -> NodeResult<Self::Value> {
        <Self as GlobalValue>::retain(keys.zoned_datetimes.get(id)?)
    }
    fn ids(keys: &Keys) -> &[i64] {
        &keys.zoned_datetimes.ids
    }
    fn with_value<R>(keys: &Keys, id: i64, visit: impl FnOnce(&Self::Value) -> R) -> NodeResult<R> {
        Ok(visit(keys.zoned_datetimes.get(id)?))
    }
}
#[cfg(test)]
mod tests {
    use super::{Hash, Hasher, NodeResult, Table};
    #[derive(Debug, PartialEq)]
    struct Collision(u8);
    impl Hash for Collision {
        fn hash<H: Hasher>(&self, state: &mut H) {
            0_u8.hash(state);
        }
    }
    #[test]
    fn equal_hashes_still_require_equal_values() -> NodeResult {
        let mut table = Table {
            values: Vec::new(),
            buckets: std::collections::HashMap::new(),
            ids: Vec::new(),
        };
        table.prepare(Collision(1))?;
        table.prepare(Collision(2))?;
        assert_eq!(table.find(&Collision(1)), Some(0));
        assert_eq!(table.find(&Collision(2)), Some(1));
        assert_eq!(table.find(&Collision(3)), None);
        Ok(())
    }
}
