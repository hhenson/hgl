//! The per-tick path allocates nothing (card, "Done when";
//! `docs/explorations/0009-designing-for-speed.md`, banned on the per-tick
//! path, 1). In a file of its own because it replaces the global allocator.

use hgl_alloc_count::{CountingAllocator, count_in};
use hgl_store::{In, Out, Scalar, Store, Wake};
use hgl_types::{EngineTime, NodeId};

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

const WARM_UP: i64 = 10;
const ROUNDS: i64 = 10_000;

/// Every wake of one cycle. Cleared, never shrunk, between cycles, as the
/// kernel's schedule keeps its capacity.
struct Woken(Vec<NodeId>);

impl Wake for Woken {
    fn wake(&mut self, node: NodeId) {
        self.0.push(node);
    }
}

/// An output with an active and a passive watcher.
struct Watched<T: Scalar> {
    owner: NodeId,
    output: Out<T>,
    active: In<T>,
    passive: In<T>,
}

fn watched<T: Scalar>(store: &mut Store, first_node: u32) -> Watched<T> {
    let owner = NodeId(first_node);
    let output = store.add_output::<T>(owner);
    let active = store.add_input::<T>(NodeId(first_node + 1), true);
    let passive = store.add_input::<T>(NodeId(first_node + 2), false);
    assert_eq!(store.bind(active.id(), output.id()), Ok(()));
    assert_eq!(store.bind(passive.id(), output.id()), Ok(()));
    Watched {
        owner,
        output,
        active,
        passive,
    }
}

struct Scenario {
    store: Store,
    flag: Watched<bool>,
    count: Watched<i64>,
    price: Watched<f64>,
    woken: Woken,
}

impl Scenario {
    /// One cycle: every output ticks, one of them twice, and every input is
    /// read. Returns how many nodes were woken and the sum of what was read.
    fn cycle(&mut self, round: i64) -> (usize, i64) {
        let now = EngineTime::from_micros(round);
        let (store, woken) = (&mut self.store, &mut self.woken);
        woken.0.clear();
        let (flag, count, price) = (&self.flag, &self.count, &self.price);
        store.set(flag.output, round % 2 == 0, now, flag.owner, woken);
        store.set(count.output, -round, now, count.owner, woken);
        store.set(count.output, round, now, count.owner, woken);
        store.set(price.output, 0.5, now, price.owner, woken);

        let mut read = store.get(self.count.active) + store.get(self.count.passive);
        read += i64::from(store.get(self.flag.active)) + i64::from(store.get(self.flag.passive));
        let halves = store.get(self.price.active) + store.get(self.price.passive);
        read += i64::from(halves >= 1.0);
        read += i64::from(store.valid(self.count.active));
        read += i64::from(store.modified(self.count.passive, now));
        (woken.0.len(), read)
    }
}

// Card, "Done when": after warm-up, 10,000 `set` + `get` rounds make zero
// allocations.
#[test]
fn ten_thousand_set_and_get_rounds_allocate_nothing() {
    let mut store = Store::new();
    let flag = watched::<bool>(&mut store, 0);
    let count = watched::<i64>(&mut store, 3);
    let price = watched::<f64>(&mut store, 6);
    let mut scenario = Scenario {
        store,
        flag,
        count,
        price,
        woken: Woken(Vec::with_capacity(3)),
    };
    for round in 1..=WARM_UP {
        scenario.cycle(round);
    }

    let ((woken, read), allocations) = count_in(|| {
        let mut totals = (0, 0);
        for round in WARM_UP + 1..=WARM_UP + ROUNDS {
            let (woken, read) = scenario.cycle(round);
            totals = (totals.0 + woken, totals.1 + read);
        }
        totals
    });

    assert_eq!(allocations, 0);
    assert_eq!(woken, 30_000, "three active watchers, once a cycle each");
    // Per round: `round` twice, the flag twice on even rounds, and one each
    // for the two halves, for valid and for modified.
    let rounds_sum: i64 = (WARM_UP + 1..=WARM_UP + ROUNDS).sum();
    assert_eq!(read, 2 * rounds_sum + 2 * (ROUNDS / 2) + 3 * ROUNDS);
}
