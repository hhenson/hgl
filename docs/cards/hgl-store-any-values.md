# Card: hgl-store-any-values

The any_values module in hgl-store adds a dedicated Any owning column and marker. It may use global_arena, global_value, prepared_value, prepared_store, shapes, observation and keys; budget 480 lines (130 transferred from Types for recursive execution) within the unchanged crate cap.

Public surface: Any implementing GlobalValue and PreparedValue; ValueColumns::any, any_mut and compose_any; PreparedTick::any and any_from_global. Prepared bounds are encoded byte capacities, including reserved typed descriptor metadata in every owner. Provisioning reserves each destination independently. compose_any measures the compiled source encoding and validates capacity before writing, preserving the previous owner on failure. Whole-box observation/publication uses the ordinary atomic contract.

Private AnyKeys owns a cold compiled domain of exact boxes, actual fallible equality/hash and recursive non-NaN validation. Discovery order does not change identity. Key lookup and borrowed comparison use retained buffers without hot allocation. Invalid capability is not converted into a usable key by differing-identity comparison shortcuts.

Acceptance: independent retained aggregate and boxed delta owners; measured allocation-free typed copy, comparison and key lookup; shared sparse, atomic, rolling and replay/record Any tests.

operations::EXECUTION provides the descriptor's equality, partial ordering and hash callbacks. It executes existing recursive field, sequence, family and box layouts with capability and NaN checks. Nested boxes dispatch their independently retained descriptor references, without consulting a consumer module table.
