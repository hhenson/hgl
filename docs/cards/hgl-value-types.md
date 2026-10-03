# Card: hgl-value-types

Resolve finite ordinary schemas against source declarations and imports. Uses
hgl-source, hgl-library and hgl-value-check; budget 350 source lines.

Public surface: identity, declaration, resolve. Nominal required-field structs
preserve qualified identity; ordinary lists preserve recursive element type and
exact fixedness. Visibility and recursive-schema checks remain source checks.
Generic, optional/default and recursive struct schemas are not admitted.

Acceptance: existing imported/nominal struct checks and ordinary nested
struct/list construction and global configuration fixtures in hgl-program.
