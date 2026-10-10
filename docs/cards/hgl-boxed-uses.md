# Card: hgl-boxed-uses

The boxed_uses analysis selects immutable lexical locals whose complete values are consumed exclusively by Any construction. It may use ir and ir_visit; initial budget 80 lines within the unchanged semantics cap.

Public surface: exclusive. Caller-selected constructor candidates retain declaration identity and source order. Ordinary reads, projections, aliases and helper arguments disqualify an owner; helper-local scopes remain separate. The backend may reserve a private box owner for the selected value without changing source admission or executing its initializer earlier.

Acceptance: recursive constructor local retains concrete identity without native Box/list allocations; mixed ordinary-use controls preserve their original representation.
