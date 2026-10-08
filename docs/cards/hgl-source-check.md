# Card: hgl-source-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `source_check` module of `hgl-semantics` (`crates/hgl-semantics/src/source_check.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Declaration-wide structural admission, shared by normal compile/test and
compile-rejection fixtures. Dependencies diagnostics, source, library, eval-data,
type-sizes and value-check; budget 550 source lines. Public validate_sources
returns every recovered primary Diagnostic, ensure_sources renders those errors
for existing command/program boundaries. Parse every declaration signature and
body before selecting reachable graphs. Reuse ordinary constant normalization
and binary type rules for catalogued failures; preserve unresolved generic
applications for normal specialization. Original annotation and operand tokens
locate failures, including inside test-only helper contexts. No graph or named test executes.

The initial pass validates syntax, closed rolling sizes, statically inferable
yield-time operands and test grammar/phase/code rules. General overload,
capability, nominal and dependent generic checking remains the normal semantic
checker's responsibility. Uncatalogued parser errors remain errors and cannot
satisfy a coded expectation. No diagnostic is classified from its message.

yield_time applies the one normative type rule to a resolved Ty and original
operand range. Both declaration prevalidation and ordinary resolved-body
checking use it; conservative inference never chooses an arbitrary overload.

May use hgl-value-types for existing concrete nominal resolution. Inference
projects declared fields through Ty::structure rather than guessing scalar
identity from property spelling; unresolved applications remain deferred.

with_semantics combines structural admission with a caller-supplied ordinary
semantic checker per structurally admitted declaration. It continues independent
declarations after failures, preserves earlier owning source, and deduplicates
repeated coded reports of the same primary origin when helpers are checked both
directly and through calls. Uncoded errors remain unexpected. May use hgl-enums
for ordinary per-declaration enum resolution and validation.

concrete_signature selects callable declarations without unresolved generic or
required constant arguments for ordinary semantic checking. It returns None
when specialization requires a call; malformed signatures remain typed errors.

Native declarations admit only the existing `throws` suffix and an optional
outer provider body. Provider contents stay opaque; an invalid signature suffix
cannot hide in an unused native declaration.

Deferred generic and required-constant function bodies resolve callable names
through hgl-name-check before specialization. Lexical service bindings and
requirements are retained; generic types and constant values are not invented.

with_module_semantics checks production dependencies and root-module test scopes;
other modules retain their own test-only declaration scopes.

argument_hint supplies unique concrete argument context and the written delta requirement; argument_requirement applies an explicitly written delta parameter after earlier arguments constrain generics. bound_type preserves catalogued excluded substituted shapes at the constraining call. replay_type resolves the source-owned TimedValue specialization. These checks retain reduced scalar delta provenance without transferring it into ordinary destinations. May use IR for checked argument identity.
