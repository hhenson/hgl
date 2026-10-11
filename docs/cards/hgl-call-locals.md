# Card: hgl-call-locals

The semantics call_locals module assigns node-wide storage identities to local
declarations in each checked ordinary helper invocation. Its inputs are IR; it
executes no source or provider bodies. Formal IDs remain unchanged and nested
helper references remain scoped. It may use only ir and ir_visit.

Acceptance: Any helper shadowing, repeated callsites, and argument once/order
controls in the generated zero-allocation evaluation suite. Native invocation
metadata and operands survive the same structural traversal.
