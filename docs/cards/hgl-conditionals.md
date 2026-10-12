# Card: hgl-conditionals

The internal conditionals module of hgl-rust lowers checked temporal branch
ownership. Initial informational ceiling: 240 lines. It uses checked IR,
finite_domains, cold_wiring and graph_assemblies; no third-party dependencies.

Plan.conditionals retains an owner and two branch node sets with actual REF
producers. The resolver checks definite assignment of one declared reference
export slot on both paths. It records external captures separately from owned
nodes, without storing an ordinary REF value or creating an empty fallback.

Scoped descriptions retain fixed path edges, typed capture boundaries and nested
child descriptions. Child REF outputs may match the owner's declared REF schema.
Cold preparation constructs independent PreparedChild instances in exact owner
scopes, reserves payload slots and caches capture handles. The generated owner
has a PreparedSwitch, a bool selector and a REF output; captures are passive.
Selection stops the previous child, samples current captures, starts a fresh
child and drives its notifications/deadlines. Publication exports the child's
actual designation, preserving endpoint identity and child ownership.

The Rust target requires a finite prepared transport proof, checked direct source
arrivals and replay lengths. The cold bound also includes startup hooks and the
dense horizon; it is not a row-count cap. Child generators, scheduler hooks and owning cache resets at activation
remain explicit target gaps. Unknown plans remain checked source but produce
a precise Rust diagnostic. Exhaustion is an error, never lifetime reuse.
Recursive cold pools currently grow with ownership depth. The hot path performs
no construction, name lookup, value cloning or resource growth.

reference_conditional executes all four immutable authored ownership groups and
eight evaluations with zero successful-cycle allocations in both profiles,
including startup and cleanup. A separate source control covers eight extra
selector timestamps after one input row. Kernel controls cover exact ownership,
fresh state, failed activation, no-hook descendant cleanup and pool exhaustion.
