# Card: hgl-value-constructors

The value_constructors policy owns explicit ordinary builtin constructor checking. It may use any_values, ir, value_eval, endpoint_check and value_access; initial budget 100 lines. Public surface: constructor and bytes, reexported from value_operations for compatibility.

bytes supplies exact ordinary i64-list context, while any checks its argument using existing ordinary admission. Neither constructor adds runtime list-literal admission, implicit encoding or value inspection. Constructor failure remains in the phase where the expression executes; required contexts reject during checking.

Acceptance: BYTE-1–6 and ANY-1 constructor matrices and ordinary runtime literal admission regressions.
