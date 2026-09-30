# Card: hgl-source

Lexer and body syntax shared by library indexing and graph checking. Uses only
`hgl-calendar`; budget 700 lines. No runtime dependency.

Surface: `Token { text, span }`, `lex(&str) -> Result<Vec<Token>, String>`;
`Ty` and `Ty::{parse,name}`; `Literal` and `Literal::ty`; `Expr` and
`Expr::fixed`; `Stmt`; `Cursor { tokens, pos }` with `new`, `peek`, `at`,
`take`, `need`, `consume`, `lines`, `name`, `type_name`, `expr`, `block`.
The enums describe literals, names, calls, unary/binary expressions, dense
sequences, lets, returns, assignments, calls, conditionals and element loops.
Spans and documentation tokens retain source text for diagnostics/emission.

Calendar literals are validated; fixed duration arithmetic is checked. Explicit
`ref<ref<T>>` is rejected. Aliases and full recursive type checking belong to
the compiler, not this parser. `_` is admitted only as a dense sequence cell.

Acceptance: shared library parsing and the compiler's negative/type tests;
calendar boundary tests exercise literal conversion separately.
