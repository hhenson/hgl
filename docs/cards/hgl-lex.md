# Card: hgl-lex

Source tokenization shared by compiler parsing. No dependencies; budget 180
source lines. Public surface: `Token { text, span }` and `lex`.
Tokens retain original byte spans, documentation comments, normalized line
separators and complete literal spellings. Lexing does not resolve names or
choose phases. Acceptance: existing source/library/compiler parsing fixtures.
