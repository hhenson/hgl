# Card: hgl-eval-data

Compile-time preparation for finite eval publication traces. Depends only on
hgl-source and hgl-rust-ir; budget 350 source lines. No runtime dispatch.

Public `validate(shape, slots)` checks a fresh trace's canonical nonempty delta
admission and returns its first rejected position and reason. Sparse children
retain their membership state between positions; removals destroy that child's
validation state. This never fills omissions from held data or changes source
constructor evaluation order. This profile has no invalidation: every admitted
publication makes its target valid. A child entry survives removal of its final
member, preserving valid-empty membership until its parent map removes it. No
separate validity bit is needed for these admission decisions. Type identity and constructor shape are already
checked by the frontend. Excluded empty publications remain explicit errors.

Public `timed(entry_type, slots)` constructs an ordinary unbounded list of exact
TimedValue entries for present dense slots. It preserves absence and the separate
caller-owned dense horizon; absolute times use the specified first eval instant
and minimum interval. All values are independently owned closed compiler data.

Public `sequence(expressions, hint, check)` retains dense absence, checks each
present expression against the exact delta type, and infers a missing target
from the first explicit delta origin or scalar. Its frontend callback performs
source checking and closed ordinary evaluation; empty generic inputs cannot
invent an origin. This helper does not validate endpoint membership.
