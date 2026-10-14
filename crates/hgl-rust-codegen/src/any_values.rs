//! Wiring-selected owning box codecs preserve exact arbitrary ordinary layouts.
use crate::layouts::{global_type, ordinary_storage, owned_type};
use hgl_semantics::ir::Plan;
use hgl_source::Ty;
pub(crate) fn name(ty: &Ty) -> String {
    crate::layouts::identifier(&ty.source_name())
}
pub(crate) fn fields(ty: &Ty) -> Option<(Vec<Ty>, Vec<usize>)> {
    crate::any_outcomes::fields(ty).or_else(|| hgl_semantics::any_values::fields(ty))
}
fn operation(ty: &Ty) -> String {
    if let Some(operation) = crate::any_outcomes::operation(ty) {
        return operation;
    }
    if let Ty::Native(scalar) = ty {
        let marker = crate::native_values::marker(scalar);
        return format!(
            "hgl_types::AnyOperation::Native {{equal:<{marker} as hgl_store::native_values::NativeType>::equal,hash:<{marker} as hgl_store::native_values::NativeType>::hash,compare:<{marker} as hgl_store::native_values::NativeType>::compare}}"
        );
    }
    let operation = if *ty == Ty::Any {
        "Boxed".into()
    } else if matches!(ty, Ty::Delta(_)) {
        "Data".into()
    } else if let Ty::Family(family) = ty {
        format!(
            "Family(&[{}])",
            family
                .members()
                .iter()
                .map(|(_, ty)| format!("&ANY_{}", name(ty)))
                .collect::<Vec<_>>()
                .join(",")
        )
    } else if let Some((fields, optional)) = fields(ty) {
        format!(
            "Fields(&[{}])",
            fields
                .iter()
                .enumerate()
                .map(|(i, ty)| format!(
                    "hgl_types::AnyField {{plan:&ANY_{},optional:{}}}",
                    name(ty),
                    optional.contains(&i)
                ))
                .collect::<Vec<_>>()
                .join(",")
        )
    } else if let Some(child) = crate::collections::element(ty) {
        format!(
            "Sequence(&ANY_{},{})",
            name(&child),
            !matches!(ty, Ty::List(..))
        )
    } else if *ty == Ty::F64 {
        "Float".into()
    } else if matches!(
        ty,
        Ty::Str | Ty::Bytes | Ty::Bool | Ty::TimeZone | Ty::ZonedTime | Ty::ZonedDateTime
    ) {
        "Octets".into()
    } else {
        "Integer".into()
    };
    format!("hgl_types::AnyOperation::{operation}")
}
fn scalar(ty: &Ty) -> Option<(&'static str, &'static str)> {
    Some(
        match (if matches!(ty, Ty::Enum(_)) {
            Some(hgl_types::ScalarType::I64)
        } else {
            ty.scalar_kind()
        })? {
            hgl_types::ScalarType::Bool => ("out.write(&[u8::from(*v)]);", "data[0]!=0"),
            hgl_types::ScalarType::I64 => {
                ("out.write(&v.to_le_bytes());", "any_number(&data) as i64")
            }
            hgl_types::ScalarType::F64 => (
                "out.write(&v.to_bits().to_le_bytes());",
                "f64::from_bits(any_number(&data))",
            ),
            hgl_types::ScalarType::Text => (
                "out.write(v.as_bytes());",
                "String::from_utf8(data.to_vec()).expect(\"encoded text\")",
            ),
            hgl_types::ScalarType::Bytes => ("out.write(v);", "data.to_vec()"),
            hgl_types::ScalarType::Date => (
                "out.write(&v.0.to_le_bytes());",
                "hgl_types::Date(any_number(&data) as i64)",
            ),
            hgl_types::ScalarType::Time => (
                "out.write(&v.0.to_le_bytes());",
                "hgl_types::Time(any_number(&data) as i64)",
            ),
            hgl_types::ScalarType::DateTime => (
                "out.write(&v.micros().to_le_bytes());",
                "hgl_types::EngineTime::from_micros(any_number(&data) as i64)",
            ),
            hgl_types::ScalarType::Duration => (
                "out.write(&v.micros().to_le_bytes());",
                "hgl_types::EngineDelta::from_micros(any_number(&data) as i64)",
            ),
            hgl_types::ScalarType::CivilDateTime => (
                "out.write(&v.micros().to_le_bytes());",
                "hgl_types::CivilDateTime::from_micros(any_number(&data) as i64)",
            ),
            hgl_types::ScalarType::TimeZone => (
                "out.write(v.as_str().as_bytes());",
                "hgl_types::ZoneId::from_validated_name(String::from_utf8(data.to_vec()).expect(\"encoded zone\"))",
            ),
            hgl_types::ScalarType::ZonedTime => (
                "out.write(&v.time().0.to_le_bytes());out.write(v.zone().as_str().as_bytes());",
                "hgl_types::ZonedTime::from_validated_parts(hgl_types::Time(any_number(&data[..8]) as i64),hgl_types::ZoneId::from_validated_name(String::from_utf8(data[8..].to_vec()).expect(\"encoded zone\")))",
            ),
            hgl_types::ScalarType::ZonedDateTime => (
                "out.write(&v.instant().micros().to_le_bytes());out.write(&i64::from(v.offset_seconds()).to_le_bytes());out.write(v.zone().as_str().as_bytes());",
                "hgl_types::ZonedDateTime::from_validated_parts(hgl_types::EngineTime::from_micros(any_number(&data[..8]) as i64),hgl_types::ZoneId::from_validated_name(String::from_utf8(data[16..].to_vec()).expect(\"encoded zone\")),i32::try_from(any_number(&data[8..16]) as i64).expect(\"encoded offset\"))",
            ),
        },
    )
}
fn codecs(ty: &Ty) -> (String, String) {
    if let Some(codecs) = crate::any_outcomes::codecs(ty) {
        return codecs;
    }
    if let Ty::Native(scalar) = ty {
        let marker = crate::native_values::marker(scalar);
        return (
            crate::native_values::append(scalar, "v.view().data()"),
            format!(
                "hgl_store::native_values::NativeView::<{marker}>::from_provider(&data).retain()?"
            ),
        );
    }
    if let Some((write, read)) = scalar(ty) {
        return (write.into(), read.into());
    }
    if *ty == Ty::Any {
        return ("out.descriptor(v.plan());if let Some(plan)=v.plan() {(plan.copy)(v.data(),out);}".into(),"{let value=data.boxed();if let Some(plan)=value.plan {any_retain(plan,value.data)?} else {hgl_types::AnyValue::default()}}".into());
    }
    if let Ty::Family(family) = ty {
        let write = members(family, |i, ty| {
            format!(
                "{i}=>any_write_{}(v.{}.as_ref().expect(\"family member\"),out),",
                name(ty),
                i + 1
            )
        });
        let read = members(family, |i, ty| {
            format!(
                "{i}=>{},",
                crate::families::family_member(
                    family,
                    i,
                    &format!("any_read_{}(data.slice(8..data.len()))?", name(ty))
                )
            )
        });
        return (
            format!(
                "out.write(&v.0.to_le_bytes());match v.0 {{{write}_=>unreachable!(\"family tag\")}}"
            ),
            format!("match any_number(&data[..8]) {{{read}_=>unreachable!(\"family tag\")}}"),
        );
    }
    if let Ty::Delta(origin) = ty {
        let storage = crate::layouts::delta_storage(origin);
        return (
            format!("any_write_{}(v,out);", name(&storage)),
            format!("any_read_{}(data)?", name(&storage)),
        );
    }
    if let Some(child) = crate::collections::element(ty) {
        let child = name(&child);
        let read = format!(
            "hgl_types::any_values::Frames(data).map(any_read_{child}).collect::<hgl_types::NodeResult<Vec<_>>>()?"
        );
        let read = if let Ty::Map(key, _) = ty {
            format!(
                "hgl_store::map_values::MapValue::new({read},|entry|Ok({}))?",
                crate::collections::key_hash(key, "entry.0")
            )
        } else {
            read
        };
        return (
            format!("for v in v {{out.frame(|out|any_write_{child}(v,out));}}"),
            read,
        );
    }
    field_codecs(ty)
}
fn field_codecs(ty: &Ty) -> (String, String) {
    let (fields, optional) = fields(ty).unwrap_or_else(|| unreachable!("ordinary codec"));
    let mut write = Vec::new();
    let mut read = Vec::new();
    for (i, ty) in fields.iter().enumerate() {
        let n = name(ty);
        if optional.contains(&i) {
            write.push(format!(
                    "out.frame(|out|if let Some(v)=&v.{i} {{out.write(&[1]);any_write_{n}(v,out);}} else {{out.write(&[0]);}});"
                ));
            let value = format!("any_read_{n}(data.slice(1..data.len()))?");
            let value = if matches!(ty,Ty::Recursive(batch) if batch.definitions().is_empty()) {
                format!("Box::new({value})")
            } else {
                value
            };
            read.push(format!(
                    "{{let data=frames.next().expect(\"encoded field\");if data[0]==0 {{None}} else {{Some({value})}}}},"
                ));
        } else {
            write.push(format!("out.frame(|out|any_write_{n}(&v.{i},out));"));
            read.push(format!(
                "any_read_{n}(frames.next().expect(\"encoded field\"))?,"
            ));
        }
    }
    let prefix = if matches!(ty, Ty::Recursive(_)) {
        format!("Owned{}", global_type(ty))
    } else {
        String::new()
    };
    let read = read.concat();
    let value = if fields.is_empty() {
        "()".into()
    } else {
        format!("({read})")
    };
    (
        write.concat(),
        format!("{{let mut frames=hgl_types::any_values::Frames(data);{prefix}{value}}}"),
    )
}
fn native_box(ty: &Ty) -> String {
    let n = name(ty);
    if *ty == Ty::Any {
        "if let Some(plan)=v.plan() {(plan.copy)(v.data(),out);}v.plan()".into()
    } else if let Ty::Family(family) = ty {
        let arms = members(family, |i, ty| {
            format!(
                "{i}=>any_box_native_{}(v.{}.as_ref().expect(\"family member\"),out),",
                name(ty),
                i + 1
            )
        });
        format!("match v.0 {{{arms}_=>unreachable!(\"family member\")}}")
    } else {
        format!("any_write_{n}(v,out);Some(&ANY_{n})")
    }
}
/// Emit exact static payload descriptors and statically typed cold/owning codecs.
pub fn layouts(plan: &Plan) -> String {
    let types = crate::layouts::global_types(plan)
        .into_iter()
        .filter(|(_, ty)| hgl_semantics::value_access::ordinary(ty))
        .collect::<std::collections::BTreeMap<_, _>>();
    if plan.wiring.is_empty() && !types.values().any(|ty| *ty == Ty::Any) {
        return String::new();
    }
    let mut out=vec!["use hgl_types::any_values::number as any_number;fn any_retain(plan:&'static hgl_types::AnyPlan,data:hgl_types::any_values::AnyView<'_>)->hgl_types::NodeResult<hgl_types::AnyValue>{let mut width=hgl_types::any_values::AnyWidth::default();(plan.copy)(data,&mut width);let size=width.finish()?;let mut out=hgl_types::AnyValue::default();out.reserve(size)?;(plan.copy)(data,&mut out);Ok(hgl_types::AnyValue::from_encoded(plan,out))}fn any_copy_frame(out:&mut dyn hgl_types::any_values::AnyBuffer,copy:impl FnOnce(&mut dyn hgl_types::any_values::AnyBuffer)){let start=out.len();out.write(&[0;8]);copy(out);out.width(start,out.len()-start-8);}".into()];
    out.push(crate::cold_validation::support().into());
    let mut decode = Vec::new();
    for ty in types.values() {
        let n = name(ty);
        let (write, read) = codecs(ty);
        let copy = copy_codec(ty);
        let validate = crate::cold_validation::codec(ty);
        let native = owned_type(ty);
        let marker = global_type(ty);
        let operation = operation(ty);
        let identity = crate::layouts::any_identity(ty.source_name().as_bytes());
        let caps = [0, 1, 2].map(|op| hgl_semantics::any_values::capable(ty, op));
        out.push(format!(
                "static ANY_{n}:hgl_types::AnyPlan=hgl_types::AnyPlan {{execution:&hgl_store::any_values::operations::EXECUTION,capture:Some(any_capture_{n}),validate:Some(any_validate_{n}),identity:std::any::TypeId::of::<{identity}>,operation:{operation},copy:any_copy_{n},capabilities:{caps:?}}};fn any_validate_{n}(data:hgl_types::any_values::AnyView<'_>)->hgl_types::NodeResult {{{validate} Ok(())}}fn any_copy_{n}(data:hgl_types::any_values::AnyView<'_>,out:&mut dyn hgl_types::any_values::AnyBuffer) {{{copy}}}fn any_write_{n}<B:hgl_types::any_values::AnyBuffer>(v:&{native},out:&mut B) {{{write}}}fn any_read_{n}(data:hgl_types::any_values::AnyView<'_>)->hgl_types::NodeResult<{native}> {{Ok({read})}}fn any_size_{n}(v:&{native})->hgl_types::NodeResult<usize> {{let mut width=hgl_types::any_values::AnyWidth::default();any_write_{n}(v,&mut width);width.finish()}}\n"
            ));
        let encoded = if let Ty::Native(scalar) = ty {
            format!(
                "hgl_semantics::ir::Value::new({},{})",
                crate::checked_data::ty(ty),
                crate::native_values::captured(scalar, "native.into_owned()")
            )
        } else {
            crate::value_convert::encode(ty, "&native")
        };
        out.push(format!("fn any_capture_{n}(data:hgl_types::any_values::AnyView<'_>)->hgl_types::NodeResult<Box<dyn std::any::Any>> {{let native=any_read_{n}(data)?;let value=(||->Result<hgl_semantics::ir::Value,String>{{Ok({encoded})}})().map_err(hgl_types::NodeError::new)?;Ok(Box::new(value))}}"));
        let slots = slot_writer(ty);
        let bound = bound(ty);
        let native_boxed = native_box(ty);
        let boxed = if *ty == Ty::Any {
            "let value=columns.any(slot.fields());if let Some(plan)=value.plan() {(plan.copy)(value.data(),out);}value.plan()".into()
        } else if let Ty::Family(family) = ty {
            let arms = members(family, |i, ty| {
                format!(
                    "{i}=>any_box_slots_{}(columns,{},out),",
                    name(ty),
                    slot_child(
                        ty,
                        &format!("&columns.list(slot.fields().{}.fields())[0]", i + 1)
                    )
                )
            });
            format!(
                "match *columns.scalar::<i64>(slot.fields().0.fields()) {{{arms}_=>unreachable!(\"family member\")}}"
            )
        } else {
            format!("any_slots_{n}(columns,slot,out);Some(&ANY_{n})")
        };
        out.push(format!(
                "fn any_slots_{n}<B:hgl_types::any_values::AnyBuffer>(columns:&hgl_store::ValueColumns,slot:hgl_store::ValueSlot<{marker}>,out:&mut B) {{{slots}}}fn any_box_slots_{n}<B:hgl_types::any_values::AnyBuffer>(columns:&hgl_store::ValueColumns,slot:hgl_store::ValueSlot<{marker}>,out:&mut B)->Option<&'static hgl_types::AnyPlan> {{{boxed}}}fn any_box_native_{n}<B:hgl_types::any_values::AnyBuffer>(v:&{native},out:&mut B)->Option<&'static hgl_types::AnyPlan> {{{native_boxed}}}fn any_bound_{n}(v:&<{marker} as hgl_store::PreparedValue>::Bounds)->usize {{{bound}}}"
            ));
        if *ty != Ty::Any {
            let native = crate::value_convert::decode(ty, "value");
            decode.push(format!(
                    "{:?}=>{{let native={native};let mut data=hgl_types::AnyValue::default();data.reserve(any_size_{n}(&native).map_err(|e|e.message)?).map_err(|e|e.message)?;any_write_{n}(&native,&mut data);Ok(hgl_types::AnyValue::from_encoded(&ANY_{n},data))}},",
                    ty.source_name()
                ));
        }
    }
    let decode = decode.concat();
    out.push(format!(
        "fn decode_any(value:&hgl_semantics::ir::Value)->Result<hgl_types::AnyValue,String>{{let Some(value)=hgl_semantics::any_values::contents(value) else {{return Ok(hgl_types::AnyValue::default())}};let value=hgl_semantics::family_values::concrete(value);match value.ty.source_name().as_str() {{{decode}_=>Err(\"unprepared any payload layout\".into())}}}}fn encode_any(value:&hgl_types::AnyValue)->Result<hgl_semantics::ir::Value,String> {{if value.plan().is_none() {{return Ok(hgl_semantics::ir::Value::new(hgl_source::Ty::Any,hgl_semantics::ir::Kind::Construct(Vec::new())))}};value.capture::<hgl_semantics::ir::Value>().map(hgl_semantics::any_values::retain).map_err(|e|e.message)}}"
    ));
    out.concat()
}
/// Encode a native ordinary value into a reserved box, retaining concrete family identity.
pub fn prepared_native(ty: &Ty, value: &str, destination: &str) -> String {
    let n = name(ty);
    format!(
        "columns.compose_any({destination},|_|{{let mut width=hgl_types::any_values::AnyWidth::default();let plan=any_box_native_{n}(&{value},&mut width);Ok((plan,width.finish()?))}},|_,out|{{any_box_native_{n}(&{value},out);}})?;"
    )
}
/// Encode a borrowed complete ordinary owner using its compiled source columns.
pub fn prepared_source(ty: &Ty, columns: &str, source: &str, destination: &str) -> String {
    let n = name(ty);
    format!(
        "columns.compose_any({destination},|columns|{{let mut width=hgl_types::any_values::AnyWidth::default();let plan=any_box_slots_{n}({columns},{source},&mut width);Ok((plan,width.finish()?))}},|columns,out|{{any_box_slots_{n}({columns},{source},out);}})?;"
    )
}
/// Retain a checked empty or supplied box constructor through its concrete plan.
pub fn constructor(
    fields: &[(usize, hgl_semantics::ir::Value)],
    emit: impl Fn(&hgl_semantics::ir::Value) -> String,
) -> String {
    fields.first().map_or_else(
        || "hgl_types::AnyValue::default()".into(),
        |(_, child)| construct(&child.ty, &emit(child)),
    )
}
/// Encode an owning constructor using its statically selected payload codec.
pub fn construct(ty: &Ty, value: &str) -> String {
    if *ty == Ty::Any {
        return format!("({value}).retain()?");
    }
    if let Ty::Family(family) = ty {
        let arms = members(family, |i, ty| {
            format!(
                "{i}=>{},",
                construct(ty, &format!("value.{}.expect(\"family member\")", i + 1))
            )
        });
        return format!(
            "{{let value={value};match value.0 {{{arms}_=>unreachable!(\"family member\")}}}}"
        );
    }
    let n = name(ty);
    format!(
        "{{let value={value};let mut data=hgl_types::AnyValue::default();data.reserve(any_size_{n}(&value)?)?;any_write_{n}(&value,&mut data);hgl_types::AnyValue::from_encoded(&ANY_{n},data)}}"
    )
}

/// Whether cold capture equality must preserve fallible contained box dispatch.
pub use hgl_semantics::any_values::contains_any as contains;
/// Cold owning capture comparison delegates dynamic failures without suppressing them.
pub fn capture_equal(ty: &Ty, a: &str, b: &str) -> String {
    format!(
        "{{let (a,b)=(||->Result<_,String>{{Ok(({},{}))}})().map_err(hgl_types::NodeError::new)?;hgl_semantics::any_values::equal_data(&a,&b)}}",
        crate::value_convert::encode(ty, a),
        crate::value_convert::encode(ty, b)
    )
}

fn slot_child(ty: &Ty, positions: &str) -> String {
    let marker = global_type(ty);
    let slots = format!("hgl_store::ValueSlot::<{marker}>::bind(&mut ({positions}).as_slice())");
    if matches!(ty,Ty::Recursive(batch) if batch.definitions().is_empty()) {
        format!(
            "hgl_store::ValueSlot::<{marker}>::bind(&mut columns.list(hgl_store::ValueSlot::<hgl_store::Recursive<{marker}>>::bind(&mut ({positions}).as_slice()).fields())[0].as_slice())"
        )
    } else {
        slots
    }
}
fn slot_writer(ty: &Ty) -> String {
    if let Some(slots) = crate::any_outcomes::slots(ty) {
        return slots;
    }
    if let Ty::Native(scalar) = ty {
        return crate::native_values::append(scalar, "columns.native(slot).data()");
    }
    let n = name(ty);
    if *ty == Ty::Any {
        return format!("any_write_{n}(columns.any(slot.fields()),out);");
    }
    if scalar(ty).is_some() {
        let scalar = if matches!(ty, Ty::Enum(_)) {
            "i64".into()
        } else {
            crate::scalars::rust_type(ty)
        };
        return format!("any_write_{n}(columns.scalar::<{scalar}>(slot.fields()),out);");
    }
    if let Some(storage) = ordinary_storage(ty) {
        if let Ty::Family(family) = ty {
            let arms = members(family, |i, ty| {
                format!(
                    "{i}=>{{let child={};any_slots_{}(columns,child,out);}},",
                    slot_child(
                        ty,
                        &format!("&columns.list(slot.fields().{}.fields())[0]", i + 1)
                    ),
                    name(ty)
                )
            });
            return format!(
                "let tag=*columns.scalar::<i64>(slot.fields().0.fields());out.write(&tag.to_le_bytes());match tag {{{arms}_=>unreachable!(\"family member\")}}"
            );
        }
        if matches!(ty, Ty::Delta(_)) {
            return format!("any_slots_{}(columns,slot,out);", name(&storage));
        }
    }
    if let Some(child) = crate::collections::element(ty) {
        return format!(
            "for positions in columns.list(slot.fields()) {{let child={};out.frame(|out|any_slots_{}(columns,child,out));}}",
            slot_child(&child, "positions"),
            name(&child)
        );
    }
    let (fields, optional) = fields(ty).unwrap_or_else(|| unreachable!("compiled slot layout"));
    fields.iter().enumerate().map(|(i,ty)| {
        if optional.contains(&i) {
            format!("out.frame(|out|if let Some(positions)=columns.list(slot.fields().{i}.fields()).first() {{out.write(&[1]);let child={};any_slots_{}(columns,child,out);}} else {{out.write(&[0]);}});",slot_child(ty,"positions"),name(ty))
        } else {format!("out.frame(|out|any_slots_{}(columns,slot.fields().{i},out));",name(ty))}
    }).collect()
}
fn bound(ty: &Ty) -> String {
    if let Some(bound) = crate::any_outcomes::bound(ty) {
        return bound;
    }
    if let Some(child) = crate::collections::element(ty) {
        return format!(
            "v.len.saturating_mul(8usize.saturating_add(any_bound_{}(&v.element)))",
            name(&child)
        );
    }
    if let Some(storage) = ordinary_storage(ty)
        && matches!(ty, Ty::Delta(_) | Ty::Family(_))
    {
        return format!("any_bound_{}(v)", name(&storage));
    }
    if let Some((fields, optional)) = fields(ty) {
        return fields
            .iter()
            .enumerate()
            .fold("0usize".into(), |size, (i, ty)| {
                let n = name(ty);
                let child = if optional.contains(&i) {
                    format!(
                        "1usize.saturating_add(v.field{i}.as_ref().map_or(0,|v|any_bound_{n}(v)))"
                    )
                } else {
                    format!("any_bound_{n}(&v.field{i})")
                };
                format!("({size}).saturating_add(8).saturating_add({child})")
            });
    }
    if matches!(ty, Ty::Any | Ty::TimeZone | Ty::ZonedTime) {
        "8usize.saturating_add(*v)"
    } else if matches!(ty, Ty::Str | Ty::Bytes | Ty::Native(_)) {
        "*v"
    } else if *ty == Ty::ZonedDateTime {
        "16usize.saturating_add(*v)"
    } else if *ty == Ty::Bool {
        "1"
    } else {
        "8"
    }
    .into()
}
/// Widen prepared Any buffers using cold ordinary bounds, without executing a source constructor.
pub fn limits<'a>(
    types: impl Iterator<Item = &'a Ty>,
    bounds: impl Fn(&Ty) -> String,
    any: usize,
) -> String {
    types
        .map(|ty| {
            format!(
                "capacity.limit{any}=capacity.limit{any}.max(any_bound_{}(&({})));",
                name(ty),
                bounds(ty)
            )
        })
        .collect::<Vec<_>>()
        .concat()
}
/// Select statically readable boxing inputs for prepared publication.
pub fn publication(value: &hgl_semantics::ir::Value) -> Option<String> {
    use hgl_semantics::ir::Kind;
    let Kind::Unary(op, argument) = &value.kind else {
        return None;
    };
    if op != "any" {
        return None;
    }
    let mut input = argument.as_ref();
    if let Kind::Unary(op, child) = &input.kind
        && op == "atomic_value"
    {
        input = child;
    }
    if let Kind::Query(op, args) = &input.kind
        && op == "delta_value"
    {
        input = args.first()?;
    }
    let Kind::Input(id, _) = input.kind else {
        return None;
    };
    let ty = crate::layouts::whole_payload(&input.ty).unwrap_or(&input.ty);
    if crate::deltas::structural(&input.ty) {
        return None;
    }
    let n = name(ty);
    let write = if crate::layouts::whole_payload(&input.ty).is_some() {
        format!(
            "let source=observation.atomic.borrow(observation.bindings,self.input{id})?;let plan=any_box_slots_{n}(observation.atomic.values(),source,out);"
        )
    } else {
        format!(
            "let source=observation.scalar::<{}>(self.input{id}.id())?;any_write_{n}(source,out);let plan=Some(&ANY_{n});",
            owned_type(ty)
        )
    };
    Some(format!(
        "_ctx.prepared().any(self._output,|observation|{{let mut width=hgl_types::any_values::AnyWidth::default();let out=&mut width;{write}Ok((plan,width.finish()?))}},|out,observation|{{let result=(||->hgl_types::NodeResult{{{write}Ok(())}})();result.unwrap_or_else(|_|unreachable!(\"preflighted ordinary source\"));}})?;"
    ))
}

/// Prepare boxed key contents through the same cold typed registry as other ordinary keys.
pub fn key_preparation(plan: &Plan) -> String {
    if crate::layouts::global_types(plan)
        .values()
        .any(|ty| *ty == Ty::Any)
    {
        "if value.ty==hgl_source::Ty::Any {return <hgl_store::Any as hgl_store::Key>::prepare(keys,&decode_any(value)?).map_err(|e|e.message);}".into()
    } else {
        String::new()
    }
}

/// Compare borrowed boxes without allocating owning intermediates.
pub fn comparison(op: &str, left: &str, right: &str) -> String {
    let method = if matches!(op, "==" | "!=") {
        "equal"
    } else {
        "compare"
    };
    let result = match op {
        "==" => "result",
        "!=" => "!result",
        "<" => "result==Some(std::cmp::Ordering::Less)",
        ">" => "result==Some(std::cmp::Ordering::Greater)",
        "<=" => "matches!(result,Some(std::cmp::Ordering::Less|std::cmp::Ordering::Equal))",
        ">=" => "matches!(result,Some(std::cmp::Ordering::Greater|std::cmp::Ordering::Equal))",
        _ => unreachable!("box comparison"),
    };
    format!("{{let left={left};let right={right};let result=left.{method}(right)?;{result}}}")
}

fn members(family: &hgl_source::FamilyType, emit: impl Fn(usize, &Ty) -> String) -> String {
    family
        .members()
        .iter()
        .enumerate()
        .map(|(i, (_, ty))| emit(i, ty))
        .collect::<Vec<_>>()
        .concat()
}

fn copy_codec(ty: &Ty) -> String {
    if let Some(copy) = crate::any_outcomes::copy(ty) {
        return copy;
    }
    if let Ty::Native(scalar) = ty {
        let marker = crate::native_values::marker(scalar);
        return format!(
            "out.native(&data,<{marker} as hgl_store::native_values::NativeType>::copy,<{marker} as hgl_store::native_values::NativeType>::preflight);"
        );
    }
    if *ty == Ty::Any {
        return "let value=data.boxed();out.descriptor(value.plan);if let Some(plan)=value.plan {(plan.copy)(value.data,out);}".into();
    }
    if let Ty::Family(family) = ty {
        let arms = members(family, |i, ty| {
            format!("{i}=>any_copy_{}(data.slice(8..data.len()),out),", name(ty))
        });
        return format!(
            "out.write(&data[..8]);match any_number(&data[..8]) {{{arms}_=>unreachable!(\"family member\")}}"
        );
    }
    if let Ty::Delta(origin) = ty {
        return format!(
            "any_copy_{}(data,out);",
            name(&crate::layouts::delta_storage(origin))
        );
    }
    if let Some(child) = crate::collections::element(ty) {
        return format!(
            "for data in hgl_types::any_values::Frames(data) {{any_copy_frame(out,|out|any_copy_{}(data,out));}}",
            name(&child)
        );
    }
    if let Some((fields, optional)) = fields(ty) {
        let mut code = "let mut frames=hgl_types::any_values::Frames(data);".to_string();
        for (i, child) in fields.iter().enumerate() {
            let child = name(child);
            let copy = if optional.contains(&i) {
                format!(
                    "out.write(&data[..1]);if data[0]!=0 {{any_copy_{child}(data.slice(1..data.len()),out);}}"
                )
            } else {
                format!("any_copy_{child}(data,out);")
            };
            crate::append(
                &mut code,
                format_args!(
                    "{{let data=frames.next().expect(\"encoded field\");any_copy_frame(out,|out|{{{copy}}});}}"
                ),
            );
        }
        return code;
    }
    "out.write(&data);".into()
}
