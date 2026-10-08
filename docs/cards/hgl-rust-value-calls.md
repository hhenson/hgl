# Card: hgl-rust-value-calls

Status: accepted for cold direct ordinary call argument representation.

Own representation selection for supplied ordinary value-call arguments. The
module has an initial 100 source-line cap within the unchanged hgl-rust 8940
limit; values remains capped at 800. It uses checked IR and source literals.

Public surface: `argument(&Value, usize, &[Statement], emit) -> String` selects
an effect-free LiteralStr borrow only when structured IR traversal proves that
no formal-local reference requires an owned String. Other arguments use the
existing emitted evaluation and retention exactly once. No source type,
category, order, constant phase or argument side effect changes. Const-formal
phase checking remains with value_bind; native/value call effects are not fixed
arguments. No rendered identifier scanning or per-cycle type test is used.

Nested bodies are checked conservatively, including their formal/local IDs.
A nested same-number local may prevent borrowing; it can never hide a use and
cause ownership to disappear. Pure substituted constant labels have no local
reads and reuse literal text without constructing redundant per-cycle Strings.
Used ordinary text parameters retain their existing owned ABI.

Acceptance: captured logger Tuple traces 2,1,3,2,1 and unchanged complete values;
allocation-counted first/repeated Tuple publications remain at zero. Effectful
fixed labels reject with the existing phase diagnostic; used non-const text
arguments execute through owned storage. Existing argument-failure/effect order
controls remain intact. No dependency, source budget or suppression is added.
