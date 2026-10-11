# Card: hgl-rust-owner-comparisons

The owner_comparisons module emits ordinary family equality and Any comparisons through independent prepared operands. It may use source_slots, any_prepared, any_values and values; optional reporting target 100 lines.


Internal surface: prepare and comparison. Each comparison reserves two independent Any scratch destinations. Family operands use static source slots; Any operands retain inline construction, local and input values in written order. Compiled codecs retain the actual concrete member identity and contents, and the established boxed comparison operation checks capabilities and compares their borrowed complete payloads. No runtime type-name lookup, native owning extraction or allocation is introduced.

Acceptance: the original admitted mixed family equality-before-boxing source runs with zero measured evaluation allocations, alongside independent constructor-copy projection and boxing.

Acceptance also includes inline Any equality/order operands inside scalar helpers,
empty and different-type boxes, and native operand count/order controls.
