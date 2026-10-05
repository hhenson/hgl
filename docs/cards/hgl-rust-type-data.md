# Card: hgl-rust-type-data

Serialize exact checked source type metadata through public ty(&Ty)->String.
Uses hgl-source and hgl-rust-enums; budget 120 source lines. Retain nominal
arguments, optional positions, recursive batches, declared enum identity and
publication origins. No expression evaluation or type inference occurs here.
hgl-rust-checked-data re-exports this function for existing emission callers.
