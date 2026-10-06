# Card: hgl-rust-families

Statically selected owning abstract family layouts and conversion emission.
Depends on source, rust-ir and rust-type-data; budget 200 source lines. No runtime crate.

family_storage(FamilyType) returns an internal exact nominal struct with an i64
discriminator and a typed optional position for every canonical concrete member.
Exactly one position is present in an owning family value. family_member forms
that representation; family_coerce remaps an explicit subfamily into its ancestor
while retaining the concrete payload. Member sets are declaration-fixed.

decode(family,expression,child_decode) validates checked membership before start;
encode(family,expression,child_encode) retains the concrete checked tag and fields.
Existing GlobalValue/Optional/PreparedValue operations allocate descendants cold,
preflight whole writes and copy into independent slots without per-tick allocation.
The schema uses a reserved internal family nominal name so no concrete declaration
can alias its discriminator/member layout. Equality includes the concrete tag.

prepare(&Plan)->Plan is a deterministic, idempotent cold emission preparation pass.
Only literal-only complete family publications in Return/Yield/TimedYield become
appended immutable node configurations. Calls, provider recipes and native hooks
are excluded. All emission entry points normalize consistently so indices agree.
Existing configuration slot publication bypasses an owning hot intermediary.
