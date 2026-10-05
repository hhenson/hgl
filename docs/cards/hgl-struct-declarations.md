# Card: hgl-struct-declarations

Declaration-owned struct syntax, separate from module indexing and nominal
resolution. Depends on hgl-source; budget 200 source lines. Compilation only.

Public RequiredStruct retains generics, fields, optional positions, defaults,
type_domain, abstract_type and parent. parse(tokens) validates one declaration;
parse_domain(cursor,generics) validates the shared finite constraint grammar.
Single inheritance retains its source application. Multiple parents are explicitly
unsupported; abstract declarations remain nonconstructible in semantic checking.
Introducing null defaults preserve optionality independently of zero/empty values.
