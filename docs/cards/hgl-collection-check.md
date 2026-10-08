# Card: hgl-collection-check

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `collection_check` module of `hgl-semantics` (`crates/hgl-semantics/src/collection_check.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Exact typed ordinary set/map construction checks. Uses source, checked IR,
ordinary evaluation, context-free classification and collection value identity;
budget 180 source lines, no runtime or third-party dependencies.

`constructor(&Ty, &[(Option<String>, Expr)], check: impl FnMut(&Expr,&Ty) ->
Result<Value,String>) -> Result<Value,String>` requires exactly `items`, checks
complete finite K and ordinary V, and preserves each source expression in order.
Known duplicate keys/members fail during checking, including signed zeros.
Runtime/provider expressions remain deferred. The result is Kind::List with its
exact Set or Map type; map elements are typed tuple Construct entries.

Acceptance: shared atomic container fixtures; missing/unknown argument, key/value
mismatch, malformed entry and signed-zero duplicate rejection.

Validate source K/V ordinary grammar before projecting nominal atomic fields
into complete ordinary payloads. Exact nominal arguments retain source identity;
field payloads use ordinary projection, including inside nested containers.
Explicit composite atomic wrappers in K/V remain outside ordinary grammar.

constructor_checked preserves typed Issue failures through recursive key/value callbacks and returns Issue; constructor adapts existing String callbacks.

items_scope fixes key and child endpoint locals for items(map-or-list) and its modified selector before checking the body. operation admits positional scalar-child map/list effects only on output endpoints, and contains on input endpoints. This profile uses i64 keys, exact scalar payloads and existing Store effects; unsupported child payloads fail checking.

operation also admits scalar-child i64 Map upsert with exact key/payload types.
Unsupported recursive payload effects remain rejected; complete ordinary output
assignment is checked by structural_admission, independently of these operations.
