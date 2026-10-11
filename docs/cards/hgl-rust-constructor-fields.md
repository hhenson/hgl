# Card: hgl-rust-constructor-fields

Constructor field emission writes complete values into reserved typed slots. It may use layouts and the checked IR; initial module budget 100 lines. The crate budget remains unchanged.

Private surface: fields. Omitted optional fields clear presence before replacement; supplied fields execute once in written order. Optional and recursive child slots retain their prepared capacity.

Acceptance: replacement of a populated optional field with an omitted field clears its presence, followed by present empty and present nonempty replacements.

The field walker accepts a storage-scope callback for optional presence updates.
Ordered private staging can release storage borrows between field evaluations
without moving those updates outside their corresponding field's retention.
