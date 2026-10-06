# Card: hgl-struct-declarations

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `struct_declarations` module of `hgl-semantics` (`crates/hgl-semantics/src/struct_declarations.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Declaration-owned struct syntax, separate from module indexing and nominal
resolution. Depends on hgl-source; budget 200 source lines. Compilation only.

Public RequiredStruct retains generics, fields, optional positions, defaults,
type_domain, abstract_type and parent. parse(tokens) validates one declaration;
parse_domain(cursor,generics) validates the shared finite constraint grammar.
Single inheritance retains its source application. Multiple parents are explicitly
unsupported; abstract declarations remain nonconstructible in semantic checking.
Introducing null defaults preserve optionality independently of zero/empty values.
