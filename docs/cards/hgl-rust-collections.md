# Card: hgl-rust-collections

Checked ordinary collection layout, constructor and equality emission. Uses
source, IR and prepared-value emission; budget 250 lines, no runtime execution.

`element(&Ty)->Option<Ty>` selects a list/set item or exact map pair.
`marker(&Ty, global:fn(&Ty)->String, schema:fn(&Ty)->String)->String` emits
an exact Set/Map schema marker delegating physical owning retention and prepared
copies to List storage of the selected item. No type-erased runtime lookup.

`construct(&Ty,&[Value],emit:impl Fn(&Value)->String,owned:fn(&Ty)->String)` emits
source-ordered native construction with immediate key validation and duplicate
checking before each map value. `equal(&Ty,&str,&str)->String` recursively emits
unordered collection comparison and ordered descendant comparison.
Acceptance: generated debug/release shared snapshots and complete nested
retention, equal empty publications, replay/record and sparse atomic children.

retained(&str,&Ty,fn(&Ty)->String)->String emits independent owning reads through exact composite markers and the existing scalar copy contract. The layout callback selects the concrete marker without runtime type tests.

Constructor key validation walks finite tuple/concrete struct fields, preserving optional presence and rejecting every present NaN leaf before the associated map value executes. Native key equality uses complete exact typed components; keys cannot contain collections, recursive values or abstract families.
