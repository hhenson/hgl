# Card: hgl-value-bind

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `value_bind` module of `hgl-semantics` (`crates/hgl-semantics/src/value_bind.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Bind checked argument values to source signatures and resolve their concrete
types. Uses hgl-source, hgl-library, hgl-rust-ir, hgl-value-types and
hgl-value-access; budget 500 source lines.

Public surface: bind, supported_type, resolve_type, method_arguments,
order_arguments, constant. This layer preserves exact ordinary list/nominal
identity, generic bindings and contextual result types. It distinguishes fixed
configuration, ordinary value-function arguments and temporal ports. Closed
ordinary list/struct data can be configuration; aggregate temporal ports remain
unsupported. It never executes values or creates runtime graph scheduling.

Acceptance: hgl-program signature selection regressions, readonly ordinary
parameters, concrete empty-list parameter context and node configuration.

`signature_types` resolves selected signature source names and infers nested
ordinary nominal/list patterns using hgl-value-types before ordinary bind checks.
Complete ambient type substitutions are retained for generic function bodies;
concrete nominal results are specialized after parameter inference.

`native` validates an exact selected native implementation, interns its checked
signature and returns Native value IR. This moves native binding preparation out
of graph checking; it neither calls providers nor broadens wiring-time effects.

Exact recursive publication shapes are admitted at temporal boundaries;
structural Delta values are not temporal shapes. Signature inference recognizes
inverse delta<T> relationships recursively through ordinary lists/nominal
arguments before binding the selected operator. No payload-content inference
or result-only structural inference is introduced.

`instantiated` matches explicit materializations against fully resolved exact
source types, retaining nominal origins, phantom arguments and recursive shape.
Implementation binding uses the same nested signature inference as declarations.

## Prepared ordinary arguments

Expose bind_prepared(signature: &Signature, args: &[(Option<String>, Value)],
runtime: bool, hint: Option<&Ty>, declared: &BTreeMap<String, Ty>) ->
Result<(Vec<Value>, BTreeMap<String, Ty>, Ty), String>. It checks cold prepared
arguments with typed binding markers, then restores their original checked
expressions. It never evaluates an input, fabricates a scalar, or marks provider
recipes as compile-time constants. Existing runtime binding restrictions remain.

ordered_call(signature: &Signature, values: Vec<Value>, supplied:
&[(Option<String>, Value)]) -> Result<(Vec<Value>, Vec<usize>), String> maps
checked declaration-order values into supplied source order followed by omitted
defaults in declaration order. Its second vector maps parameter positions to
lexical argument positions. It never evaluates values. Ordinary helper bodies
read the once-constructed lexical parameter, including const parameters.

Parameter literal defaults admit both fixed scalar literals and contextual
TemporalLiteral recipes. Materialize an omitted contextual default at invocation
preparation, after supplied arguments; supplied arguments suppress that default.

Signal parameters retain the actual endpoint shape for prepared metadata access,
but never unify that shape as an ordinary type named signal. Existing lexical
signal restrictions still prohibit payload observation through the formal.

Delta traversal includes typed Add/Remove keys and both Keyed map operands;
retained keys obey the same constant/provenance checks as child expressions.
