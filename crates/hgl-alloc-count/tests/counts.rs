//! The counting allocator installed as a real global allocator.

use hgl_alloc_count::{CountingAllocator, allocations, count_in};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

#[test]
fn work_that_does_not_allocate_counts_zero() {
    let mut total = 0_u64;
    let ((), count) = count_in(|| {
        for value in 0..1_000_u64 {
            total = total.wrapping_add(value);
        }
    });
    assert_eq!(count, 0);
    assert_eq!(total, 499_500);
}

#[test]
fn each_allocation_is_counted_once() {
    let (buffer, count) = count_in(|| Vec::<u8>::with_capacity(64));
    assert_eq!(count, 1);
    assert!(buffer.capacity() >= 64);
}

#[test]
fn growing_a_buffer_counts_as_an_allocation() {
    let mut buffer = Vec::<u64>::with_capacity(1);
    buffer.push(1);
    let ((), count) = count_in(|| buffer.extend(0..1_000));
    assert!(count >= 1, "a reallocation must be visible to the test");
}

#[test]
fn a_buffer_that_keeps_its_capacity_does_not_allocate_again() {
    let mut buffer = Vec::<u64>::with_capacity(1_000);
    let ((), count) = count_in(|| {
        for round in 0..10 {
            buffer.clear();
            buffer.extend(0..1_000_u64.saturating_sub(round));
        }
    });
    assert_eq!(count, 0, "this is the pattern the per-tick path relies on");
}

#[test]
fn the_running_total_only_grows() {
    let before = allocations();
    let boxed = Box::new(7_u32);
    assert!(allocations() > before);
    assert_eq!(*boxed, 7);
}
