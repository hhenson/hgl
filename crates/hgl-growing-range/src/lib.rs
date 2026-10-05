//! Allocation-free canonical dense-index publication validation.
/// Validate append/update indices and a complete removed tail against prior length.
/// Iterators may arrive in any order; repeated indices and overlap are rejected.
pub fn validate(
    length: usize,
    items: impl Iterator<Item = i64> + Clone,
    removed: impl Iterator<Item = i64> + Clone,
) -> Result<(), &'static str> {
    check(&items)?;
    check(&removed)?;
    validate_distinct(length, items, removed)
}
/// Validate state-dependent ranges when each iterator is already distinct and nonnegative.
/// Checked source constructors and publication membership tables establish this invariant.
pub fn validate_distinct(
    length: usize,
    mut items: impl Iterator<Item = i64> + Clone,
    mut removed: impl Iterator<Item = i64> + Clone,
) -> Result<(), &'static str> {
    let length = i64::try_from(length).map_err(|_range| "growing list length overflow")?;
    if let Some(cut) = removed.clone().min() {
        let count = i64::try_from(removed.clone().count())
            .map_err(|_range| "growing list length overflow")?;
        if cut >= length || count != length - cut || removed.any(|index| index >= length) {
            return Err("growing list removal must be the complete current tail");
        }
        if items.any(|index| index >= cut) {
            return Err("growing list removal overlaps or precedes an updated index");
        }
    } else {
        let appended = i64::try_from(items.clone().filter(|index| *index >= length).count())
            .map_err(|_range| "growing list length overflow")?;
        let end = length
            .checked_add(appended)
            .ok_or("growing list length overflow")?;
        if items.any(|index| index >= end) {
            return Err("growing list append has a gap");
        }
    }
    Ok(())
}
fn check(indices: &(impl Iterator<Item = i64> + Clone)) -> Result<(), &'static str> {
    for (position, index) in (*indices).clone().enumerate() {
        if index < 0 {
            return Err("growing list index must be nonnegative");
        }
        if (*indices)
            .clone()
            .take(position)
            .any(|prior| prior == index)
        {
            return Err("duplicate growing list index");
        }
    }
    Ok(())
}
