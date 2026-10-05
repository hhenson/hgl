# Card: hgl-static-values

Compiler-only lexical provenance, shared by graph selection and ordered test
preparation. Uses hgl-rust-ir, hgl-source, hgl-library, hgl-value-constant and
hgl-value-eval and hgl-composite-keys. Budget 150 source lines. No provider execution or runtime store.

StaticValues derives Default/Clone and exposes prepared/configuration Vec<Value>
and locals/cold_locals BTreeMap<usize,Value>. cold_locals are sparse-key-only
alias proofs; they never broaden general configuration or global-state key
admission. resolve follows checked cold origins; arguments
maps known const parameters into source-order lexical slots. bind records only
immutable cold recipes or closed constant origins and removes stale mutable
identities. key(Value) returns the retained checked Value and Option<Value>
for checking-time duplicate/overlap diagnostics. Local IR is never substituted
by its initializer. Closed constants use existing ordinary evaluation; contextual
recipes are never materialized here. Exact key typing belongs to delta-check.

PreparedLexicalScope derives Default/Clone and exposes bindings, next, origins
and presence facts. It persists across ordinary setup, assertions, supplied
eval values and expectations. Its metadata cannot execute a provider, create
bindings at runtime, or change ordinary access authority. Branches copy lexical
metadata while fresh binding indices remain unique throughout a named test.

Complete tuple/struct origins are proved recursively through immutable aliases;
provider leaves remain recipes, and any deferred leaf withholds known identity.
Checking-time evaluation of closed expressions is unchanged. bind only records
provenance and does not execute initializers. Retained Local operands remain in
source order; complete known Values carry exact fields and optional presence.
