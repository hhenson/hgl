# Card: hgl-family-types

Declaration-fixed abstract membership and scoped inherited field typing. Depends
on source, library, inheritance, struct-names and value-access; budget 300 lines.
All operations are cold source checks. Resolution/unification callbacks preserve
the existing ordinary type resolver and avoid cyclic crate dependencies.

field_type resolves a scoped Pattern with root bindings; infer_field unifies its
actual value and propagates invariant arguments back to those bindings. resolve
collects every declared concrete descendant compatible with the exact requested
family specialization, retains explicit ancestor identities and rejects members
with unresolved invariant arguments or unsupported payload schemas. Replay inputs
never determine the member closure. Nonmembers, unapplied specializations and
multiple parents remain errors; families add no temporal field projection.

Ancestor arguments preserve complete nested nominal schemas and fixed list lengths;
identity never substitutes an empty-field placeholder for a concrete argument.
