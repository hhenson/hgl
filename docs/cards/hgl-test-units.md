# hgl-test-units

Status: accepted

Recover declaration and named-test extents for annotation ownership without
changing ordinary semantic checking. units returns module/name/kind and original
byte/line ranges; inner declarations own annotations inside unnamed test contexts.
mask replaces excluded ranges with spaces while preserving every byte offset and
newline. Safe body delimiters and ordinary declaration terminators establish
boundaries; unresolved delimiter damage before a neighboring declaration fails
admission. Sole final declarations can end at EOF. No stubs or fabricated values.
May use hgl-source. Provider bodies use existing lexical token boundaries.

Unit.first_on_line distinguishes a declaration header from a context header on
the same physical line; annotations cannot transfer ownership past that header.
