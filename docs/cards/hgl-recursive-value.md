# Card: hgl-recursive-value

Status: since [decision 0004](../decisions/0004-crates-are-programs-policies-or-walls.md) this card describes the `recursive_value` module of `hgl-store` (`crates/hgl-store/src/recursive_value.rs`); its budget and "may use" list are held per module by `cargo xtask ci`.

Typed owning indirection for internal recursive atomic fields. Uses types,
global-value and prepared-value; budget220. No source type, optional
access, cycle construction or shared writable payload is introduced.

`RecursiveTarget:GlobalValue` supplies the exact static `IDENTITY` of a generated
concrete member. `Recursive<T>` implements GlobalValue with Box<T::Value>, one
independently owned descriptor slot and schema RecursiveReference(T::IDENTITY).
It occurs inside Optional for recursive source edges. Ordinary retention and cold
extraction own every descendant. Schema construction terminates at nominal edges.

For T:PreparedValue, Recursive implements PreparedValue with Box<T::Bounds>.
Bounds merge and allocation follow actual finite payload depth; Optional bounds
terminate unset edges. Checks traverse the complete finite active value before
infallible copies reuse reserved independent descendants. No Box or descriptor
allocation occurs during publication, pass-through or prepared capture.
