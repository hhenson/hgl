//! The schedule: which nodes the pass in progress must reach, and which are
//! to be woken at a later time (specification: Graph, "Scheduling a node").
//!
//! Nothing here visits a node that is not scheduled. *Now* is a set of ranks
//! that gives up its lowest member; *later* is a min-heap whose top is the
//! graph's next scheduled time.

use hgl_deadlines::Deadlines;

use hgl_store::Wake;
use hgl_types::{EngineTime, NodeId};

const BITS: u32 = u64::BITS;

/// A set of ranks, as a tree of 64-way bitmaps. Adding a rank and taking the
/// lowest cost at most the depth of the tree: one level up to 64 nodes, two up
/// to 4,096, three up to 262,144. A flat bitmap would make every cycle scan a
/// sixty-fourth of the graph.
#[derive(Debug)]
struct RankSet {
    /// `levels[0]` has a bit per rank. A bit in any level above says that the
    /// word below it is not empty. The last level is a single word.
    levels: Vec<Vec<u64>>,
    /// Where the scan stands: no rank below this is in the set. Its word is
    /// tried first, because going down from the top is a chain of loads each
    /// waiting on the last, and a pass mostly finds its next node nearby.
    floor: u32,
}

impl RankSet {
    fn new(ranks: usize) -> Self {
        let mut levels = Vec::new();
        let mut words = ranks.div_ceil(BITS as usize).max(1);
        loop {
            levels.push(vec![0; words]);
            if words == 1 {
                return Self { levels, floor: 0 };
            }
            words = words.div_ceil(BITS as usize);
        }
    }

    /// Adding a rank already present changes nothing.
    #[inline]
    fn insert(&mut self, rank: u32) {
        self.floor = self.floor.min(rank);
        let mut index = rank;
        for level in &mut self.levels {
            let word = &mut level[(index / BITS) as usize];
            let was_empty = *word == 0;
            *word |= 1 << (index % BITS);
            if !was_empty {
                return;
            }
            index /= BITS;
        }
    }

    /// The lowest rank, found from the top of the tree.
    fn first(&self) -> Option<u32> {
        let mut index = 0;
        for level in self.levels.iter().rev() {
            let word = level[index as usize];
            if word == 0 {
                return None;
            }
            index = index * BITS + word.trailing_zeros();
        }
        Some(index)
    }

    /// Remove and return the lowest rank.
    #[inline]
    fn take_first(&mut self) -> Option<u32> {
        let near = self.levels[0][(self.floor / BITS) as usize];
        let rank = if near == 0 {
            self.first()?
        } else {
            self.floor / BITS * BITS + near.trailing_zeros()
        };
        self.floor = rank;
        let mut index = rank;
        for level in &mut self.levels {
            let word = &mut level[(index / BITS) as usize];
            *word &= !(1 << (index % BITS));
            if *word != 0 {
                break;
            }
            index /= BITS;
        }
        Some(rank)
    }
}

/// One graph's schedule, and the scheduler requests of its nodes.
#[derive(Debug)]
pub(crate) struct Schedule {
    /// The nodes the pass in progress has still to reach.
    ready: RankSet,
    /// One replaceable deadline per node; cancellation releases the entry.
    later: Deadlines,
    /// Per node, the time of its live entry in `later`; `NEVER` for none.
    ///
    /// Between cycles a node's entry is its request: every visit, whatever
    /// woke the node, uses the entry and re-arms it from the request (GRF-14).
    /// The one exception is schedule-on-start, whose entry is the start time
    /// until the node's first visit.
    entry_at: Vec<EngineTime>,
    /// Per node, its scheduler's one pending request; `NEVER` for none.
    request: Vec<EngineTime>,
    child_request: Vec<EngineTime>,
}

impl Schedule {
    pub(crate) fn new(nodes: usize) -> Self {
        let mut later = Deadlines::default();
        later.reserve(nodes);
        Self {
            ready: RankSet::new(nodes),
            later,
            entry_at: vec![EngineTime::NEVER; nodes],
            request: vec![EngineTime::NEVER; nodes],
            child_request: vec![EngineTime::FOREVER; nodes],
        }
    }

    /// Schedule `node` for `now`, the evaluation time: its entry becomes
    /// `now`, whatever it held (Graph, "Scheduling a node"). An input's wake
    /// needs no entry, as the pass it is for is already under way.
    pub(crate) fn schedule_now(&mut self, node: NodeId, now: EngineTime) {
        let entry = &mut self.entry_at[node.0 as usize];
        if *entry != now {
            *entry = now;
            self.later.set(node.0 as usize, now);
        }
    }

    /// Forget every entry and request: a graph whose start failed has
    /// nothing scheduled.
    pub(crate) fn clear(&mut self) {
        self.later.clear();
        self.entry_at.fill(EngineTime::NEVER);
        self.request.fill(EngineTime::NEVER);
        self.child_request.fill(EngineTime::FOREVER);
    }

    /// The node's scheduler is asked for `time`, replacing what it held.
    #[inline]
    pub(crate) fn set_request(&mut self, node: NodeId, time: EngineTime) {
        self.request[node.0 as usize] = time;
    }

    pub(crate) fn set_child_request(&mut self, node: NodeId, time: EngineTime) {
        self.child_request[node.0 as usize] = time;
    }

    /// Use up the node's request if it fell due at `now`, and say whether it
    /// did (NOD-12).
    #[inline]
    pub(crate) fn take_due_request(&mut self, node: NodeId, now: EngineTime) -> bool {
        let request = &mut self.request[node.0 as usize];
        let due = *request == now;
        if due {
            *request = EngineTime::NEVER;
        }
        due
    }

    /// After a node's start or visit: its entry becomes its pending request,
    /// earlier or later than the entry it had, since a visit uses the entry
    /// however the node was woken (GRF-14). Replacement is indexed; repeated
    /// rearming cannot accumulate stale entries.
    #[inline]
    pub(crate) fn rearm(&mut self, node: NodeId) {
        let index = node.0 as usize;
        let request = self.request[index];
        let request = if request == EngineTime::NEVER {
            self.child_request[index]
        } else {
            request.min(self.child_request[index])
        };
        let request = if request == EngineTime::FOREVER {
            EngineTime::NEVER
        } else {
            request
        };
        if self.entry_at[index] != request {
            self.entry_at[index] = request;
            self.later.set(
                index,
                if request == EngineTime::NEVER {
                    EngineTime::FOREVER
                } else {
                    request
                },
            );
        }
    }

    /// Make ready every node whose entry is due at `now`.
    pub(crate) fn begin_pass(&mut self, now: EngineTime) {
        while let Some((time, slot)) = self.later.first()
            && time <= now
        {
            self.later.remove(slot);
            let node = NodeId(u32::try_from(slot).unwrap_or_else(|_| unreachable!("node rank")));
            let entry = &mut self.entry_at[node.0 as usize];
            if *entry == time {
                debug_assert!(time == now, "ENG-4: a scheduled time was skipped");
                *entry = EngineTime::NEVER;
                self.ready.insert(node.0);
            }
        }
    }

    /// The lowest-ranked node the pass has still to reach.
    #[inline]
    pub(crate) fn take_ready(&mut self) -> Option<NodeId> {
        self.ready.take_first().map(NodeId)
    }

    /// End a pass that a failure cut short, leaving the schedule as a
    /// completed pass would: the nodes it had still to reach lose this cycle,
    /// and so does what they had asked for in it.
    pub(crate) fn abandon_pass(&mut self, now: EngineTime) {
        while let Some(node) = self.take_ready() {
            self.take_due_request(node, now);
            self.rearm(node);
        }
    }

    /// `FOREVER` when nothing is scheduled.
    pub(crate) fn next_time(&self) -> EngineTime {
        match self.later.first() {
            Some((time, _)) => time,
            None => EngineTime::FOREVER,
        }
    }
}

impl Wake for Schedule {
    /// The store wakes only while a node's eval is writing, so this is always
    /// for the pass in progress. A set bit is idempotent (GRF-16). The entry
    /// is left alone: the visit this wake brings re-arms it.
    #[inline]
    fn wake(&mut self, node: NodeId) {
        self.ready.insert(node.0);
    }
}
