# Card: hgl-columns

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `columns` module of `hgl-store` (`crates/hgl-store/src/columns.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Typed value columns separated from endpoint/binding policy. Uses `hgl-types`;
budget 350 lines. `hgl-store` owns its private `Columns` instance.

Surface: `Columns: Default + Debug`,
`Columns::value(ScalarType, usize) -> ScalarValue`; sealed
`Scalar: Clone + PartialEq + Debug + Default` with `TYPE`, `into_value`,
`from_value` and inherited column accessors. Implementations: bool, i64, f64,
String, Date, Time, EngineTime, EngineDelta. The private `Column` trait seals
these implementations; external crates cannot add storage types.

Monomorphized reads use one typed vector. Erased reads are for tooling. Owned
strings move into slots; borrowed reads allocate nothing. Scalar handles stay
Copy even when their payload is not. Equal publications still tick (TS-6).

Acceptance: existing allocation tests, scalar round trips, borrowed string
address stability and the compile-fail sealing test.

`Scalar::try_clone(&self) -> Result<Self, Box<hgl_types::NodeError>>` provides
fallible ordinary owned copying. Fixed scalar implementations use their copy;
String reserves fallibly before copying. Existing endpoint reads are unchanged.

CivilDateTime, ZoneId, ZonedDateTime and ZonedTime are concrete typed scalar implementations.
Zone-bearing values retain exact owned names fallibly at retention boundaries;
prepared borrowed projections neither allocate nor consult a provider.

Scalar inherits hgl-scalar-copy::ScalarCopy for statically selected cold byte
reservation and prevalidated capacity-preserving copies. Existing fixed scalars
remain direct copies. The sealed set of scalar storage types is unchanged.
