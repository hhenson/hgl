# Card: hgl-reference-routes

The internal reference_routes module of hgl-rust emits cold reservation from
checked finite graph topology. It uses checked IR and hgl-source only; it has
an initial 120-line informational budget inside the existing crate.

Internal reserve(Plan) walks declared REF capture dependencies and the internal
fixed projection opcode to gather concrete output paths. It counts possible
ordinary followers, reserves each target's subscriptions and prepares each
consumer's absent child views before startup. It never runs a routing handler
or inspects a target payload; duplicate routes share capacity metadata.

First, equal, silent, invalid and retained scalar/Native routes, fixed/nominal
projections and sparse structural deltas execute with zero successful-cycle
allocations in the 24-group reference transport matrix. Lifecycle-owned routes
remain the separate conditional compiler slice.
