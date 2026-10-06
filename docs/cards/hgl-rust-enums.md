# Card: hgl-rust-enums

Static Rust emission for exact declared enum storage markers. Uses hgl-source;
budget 180 source lines. No runtime lookup or third-party dependencies.

marker_type(&EnumType) -> String names the canonical generated marker;
marker(&EnumType) -> String emits its GlobalValue implementation. Enum values
use i64 physical slots and OrdinaryType::Enum(origin), with PREPARED_SCALAR true.
Retain, prepare, read, commit, install, release and layout operations delegate to
the typed i64 implementation. Member numbers are already checked by the frontend.

Typed storage tests verify exact nominal binding and allocation-free first and
repeated publications; generated shared HGL tests verify recursive enum capture.

Markers also implement the hgl-store Key contract by delegating physical i64
prepare/id/value/domain operations. The collection schema preserves the nominal
enum identity independently of the membership token.

Nominal markers also delegate additive PreparedValue emission to
hgl-rust-prepared-values, retaining exact identity in independently prepared slots.

metadata(&EnumType)->String emits complete cold checked enum metadata, shared by
checked type and enum literal serialization. Numbers and nominal origins are
retained exactly.
