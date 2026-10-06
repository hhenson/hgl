//! Deadline replacement and cancellation against an independent ordered model.
use hgl_kernel::deadlines::Deadlines;
use hgl_types::EngineTime;
use std::collections::BTreeMap;
#[test]
fn replacements_and_cancellations_preserve_the_minimum() {
    let mut heap = Deadlines::default();
    heap.reserve(64);
    let mut model = BTreeMap::new();
    let mut seed = 731_u64;
    for step in 0..100_000 {
        seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
        let slot = usize::try_from(seed % 64).unwrap_or_default();
        if step % 3 == 0 {
            heap.remove(slot);
            model.remove(&slot);
        } else {
            let time =
                EngineTime::from_micros(i64::try_from(seed % 10_000).unwrap_or_default() + 1);
            heap.set(slot, time);
            model.insert(slot, time);
        }
        assert_eq!(
            heap.first(),
            model.iter().map(|(&slot, &time)| (time, slot)).min()
        );
        if step % 109 == 0 {
            heap.clear();
            model.clear();
            assert_eq!(heap.first(), None);
        }
    }
}

#[test]
fn smaller_reservation_preserves_live_slots() {
    let mut deadlines = Deadlines::default();
    deadlines.reserve(8);
    let time = EngineTime::MIN_START;
    deadlines.set(7, time);
    deadlines.reserve(2);
    assert_eq!(deadlines.first(), Some((time, 7)));
    deadlines.remove(7);
    assert_eq!(deadlines.first(), None);
}
