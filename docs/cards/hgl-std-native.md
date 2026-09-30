# Card: hgl-std-native

Rust implementations of the selected shared native scalar interfaces. Uses
`hgl-types`, `hgl-calendar`, `hgl-kernel`; budget 700 lines. No third-party
crates. Selection stays beside Rust binding code in `native/stdlib/rust.hgl`;
portable interfaces and operator bodies remain in hgraph_std.

Public free functions use the emitter's `name_argtype...` spelling. Surface:
text `len`, `is_empty`, `contains`, `starts_with`, `ends_with`, `slice`;
`truthy` (f64/str), `bit_and` (i64), `power` (i64/f64), integer shifts,
`round_decimal`; `as_str` for the eight scalar types; date year/month/day,
time hour, datetime year/hour/timestamp, duration days/seconds/microseconds;
`as_float_i64`, `as_int_f64`; numeric/text `add`, integral `div`, `floordiv`,
`mod`; `print_line_str`, `log_info_str`, `raise_error_str`.
`rust.hgl` lists the exact selected overloads. These are scalar functions,
not replacements for HGL stream nodes. Unselected native signatures fail
compilation rather than acquiring an implicit implementation.

Text inputs borrow `&str`; results own their payload. Declared `throws`
functions return `Result<T, Box<NodeError>>`. Logging uses the test host's
stderr service. Formatting and collection growth may allocate; this slice
makes no new performance-parity claim.

Acceptance: embedded shared native tests plus overflow/error paths, signed
floor/modulo, Unicode slicing, exponent spelling and negative durations.
