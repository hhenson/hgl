//! Exact prepared key domains and allocation-free lookup.
use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::keys::{Key, Keys};
use hgl_types::{NodeResult, Time, ZoneId, ZonedTime};
#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;
#[test]
fn owning_keys_are_retained_cold_and_repeated_lookup_allocates_nothing() -> NodeResult {
    let mut keys = Keys::default();
    let mut text = String::from("first");
    <String as Key>::prepare(&mut keys, &text)?;
    let alias = ZoneId::from_validated_name("US/Eastern".into());
    let canonical = ZoneId::from_validated_name("America/New_York".into());
    ZoneId::prepare(&mut keys, &alias)?;
    ZoneId::prepare(&mut keys, &canonical)?;
    let wall = ZonedTime::from_validated_parts(Time(123), alias.clone());
    ZonedTime::prepare(&mut keys, &wall)?;
    let (result, allocations) = count_in(|| -> NodeResult {
        for _ in 0..100 {
            assert_eq!(<String as Key>::id(&keys, &text)?, 0);
            assert_ne!(ZoneId::id(&keys, &alias)?, ZoneId::id(&keys, &canonical)?);
            assert_eq!(ZonedTime::id(&keys, &wall)?, 0);
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    text.clear();
    assert_eq!(<String as Key>::value(&keys, 0)?, "first");
    assert!(<String as Key>::id(&keys, &text).is_err());
    Ok(())
}
#[test]
fn ieee_keys_share_zero_and_preserve_infinities() -> NodeResult {
    let mut keys = Keys::default();
    for value in [-0.0, 0.0, f64::INFINITY, f64::NEG_INFINITY] {
        f64::prepare(&mut keys, &value)?;
    }
    assert_eq!(f64::ids(&keys).len(), 3);
    assert_eq!(f64::id(&keys, &-0.0)?, f64::id(&keys, &0.0)?);
    for value in [f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            f64::value(&keys, f64::id(&keys, &value)?)?.to_bits(),
            value.to_bits()
        );
    }
    assert!(f64::prepare(&mut keys, &f64::NAN).is_err());
    Ok(())
}
#[test]
fn composite_domains_retain_full_tokens_and_presence_without_lookup_allocations() -> NodeResult {
    let mut keys = Keys::default();
    let mut source = [1, 7, 0, 0];
    keys.prepare_composite(5, &source)?;
    keys.prepare_composite(5, &[1, 7, 1, 0])?;
    keys.prepare_composite(1, &[1, 8, 0, 0])?;
    source[1] = 99;
    let (result, allocations) = count_in(|| -> NodeResult {
        for _ in 0..100 {
            assert_eq!(keys.composite_id(5, &[1, 7, 0, 0])?, 0);
            assert_eq!(keys.composite_id(5, &[1, 7, 1, 0])?, 1);
            assert_eq!(keys.composite_parts(5, 0)?, &[1, 7, 0, 0]);
            assert_eq!(keys.composite_ids(1), &[0]);
        }
        Ok(())
    });
    result?;
    assert_eq!(allocations, 0);
    assert!(keys.composite_id(5, &source).is_err());
    assert!(keys.composite_id(1, &[1, 7, 0, 0]).is_err());
    Ok(())
}
