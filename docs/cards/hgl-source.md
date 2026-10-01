# Card: hgl-source

Lexer and body syntax shared by library indexing and graph checking. Uses only
`hgl-calendar`; budget 700 lines. No runtime dependency.

Surface: `Token { text, span }`, `lex(&str) -> Result<Vec<Token>, String>`;
`Ty` and `Ty::{parse,name}`; `Literal` and `Literal::ty`; `Expr` and
`Expr::{fixed,handler_guard}`; `Stmt`; `Cursor { tokens, pos }` with `new`, `peek`, `at`,
`take`, `need`, `consume`, `lines`, `name`, `type_name`, `expr`, `block`.
The enums describe literals, names, calls, unary/binary expressions, dense
sequences, lets, returns, assignments, calls, conditionals and element loops.
Spans and documentation tokens retain source text for diagnostics/emission.

Calendar literals are validated; fixed duration arithmetic is checked. Explicit
`ref<ref<T>>` is rejected. Aliases and full recursive type checking belong to
the compiler, not this parser. `_` is admitted only as a dense sequence cell.

Acceptance: shared library parsing and the compiler's negative/type tests;
calendar boundary tests exercise literal conversion separately.

`Expr::handler_guard(self, inputs: &[String]) -> Expr` normalizes a temporal
handler predicate: contextual empty valid/modified selectors expand to the
supplied input names; missing top-level conjunction selectors are prepended.
Selectors under disjunction/negation/residual calls do not suppress defaults.
This pure AST transform is independent of types and runtime representation;
backends choose their admitted handler profile before applying it.

`Expr::{Null,Index}` represents contextual absence and postfix indexing;
`Stmt::{Var,Exit}` distinguishes mutable locals and a bare runtime return from
immutable `Let` and value-return statements. `Ty::Nullable` is an internal
inferred expression type, never a source annotation accepted by `Ty::parse`.
These forms add no general optional type or null harness-cell alias. Source
checking determines which indexed receivers and nullable uses are admitted.

`Expr::Property(Box<Expr>, String)` represents ordinary postfix property/field
selection, separately from `::` qualified names. It is not a method call.
`Stmt::{Assign,Add}` retain an expression target for checking: the parser does
not treat a dotted target as an ordinary variable name. The checker determines
whether the selected field is writable; no new writable field domain is admitted.

`Stmt::{Let,Var}` carry an optional source type annotation alongside their
name and initializer. Annotation resolution and initializer compatibility are
frontend checks; a typed local supplies ordinary expected-value context.
