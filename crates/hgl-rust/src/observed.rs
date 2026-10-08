//! Typed structural publication transport between independently prepared destinations.
use std::fmt::Write as _;
fn append(out: &mut String, args: std::fmt::Arguments<'_>) {
    out.write_fmt(args)
        .unwrap_or_else(|_| unreachable!("String formatting"));
}
use crate::layouts::{delta_type, global_type, rust_type, whole_payload};
use hgl_semantics::ir::{Kind, Value};
use hgl_source::Ty;
fn structural(ty: &Ty) -> bool {
    matches!(
        ty,
        Ty::List(..) | Ty::Map(..) | Ty::Set(_) | Ty::Tuple(_) | Ty::Struct(..)
    )
}
fn marker(ty: &Ty) -> String {
    global_type(&delta_type(ty))
}
fn fields(ty: &Ty) -> Vec<&Ty> {
    if let Ty::Struct(_, fields, _) = ty {
        return fields.iter().map(|(_, ty)| ty).collect();
    }
    if let Ty::Tuple(fields) = ty {
        return fields.iter().collect();
    }
    Vec::new()
}

fn element(ty: &Ty, list: &str, index: &str, columns: &str) -> String {
    format!(
        "hgl_store::ValueSlot::<{}>::bind(&mut {columns}.list(({list}).fields())[{index}].as_slice())",
        global_type(ty)
    )
}
/// Publish a typed payload slot without constructing an owning intermediary.
pub fn apply(ty: &Ty, output: &str, source: &str, slot: &str) -> String {
    if matches!(ty, Ty::Rolling(..)) {
        return crate::windows::from(ty, output, source, slot);
    }
    if let Some(payload) = whole_payload(ty) {
        return format!(
            "_ctx.prepared().atomic_from::<{}>({source},{slot},{output})?;",
            global_type(payload)
        );
    }
    if structural(ty) {
        return format!(
            "{}::apply_slot({output},{source},{slot},_ctx)?;",
            marker(ty)
        );
    }
    format!(
        "_ctx.prepared().scalar::<{}>({output}.id(),{output}.generation(),{source}.scalar::<{}>(({slot}).fields()))?;",
        rust_type(ty),
        rust_type(ty)
    )
}
/// Copy a sparse temporal observation into an independently owned prepared ordinary slot.
pub fn capture(ty: &Ty, input: &str, slot: &str) -> String {
    if matches!(ty, Ty::Rolling(..)) {
        return crate::windows::capture(ty, input, slot);
    }
    if let Some(payload) = whole_payload(ty) {
        let marker = global_type(payload);
        return format!(
            "{{let source=observation.atomic.borrow(observation.bindings,{input})?;<{marker} as hgl_store::PreparedValue>::check_slots(observation.atomic.values(),source,columns,{slot})?;<{marker} as hgl_store::PreparedValue>::copy_between(observation.atomic.values(),source,columns,{slot});}}"
        );
    }
    if structural(ty) {
        return format!(
            "{}::observe_slot({input},observation,now,columns,{slot})?;",
            marker(ty)
        );
    }
    let marker = global_type(ty);
    format!(
        "{{let value=observation.scalar::<{marker}>({input}.id())?;<{marker} as hgl_store::PreparedValue>::check_native(columns,{slot},value)?;<{marker} as hgl_store::PreparedValue>::copy_native(columns,{slot},value);}}"
    )
}
/// Forward exact changed publications directly between prepared temporal endpoints.
pub fn pass(ty: &Ty, input: &str, output: &str) -> String {
    if matches!(ty, Ty::Rolling(..)) {
        return crate::windows::pass(ty, input, output);
    }
    if let Some(payload) = whole_payload(ty) {
        return format!(
            "_ctx.prepared().pass_atomic::<{}>({input},{output})?;",
            global_type(payload)
        );
    }
    if structural(ty) {
        return format!("{}::pass({input},{output},_ctx)?;", marker(ty));
    }
    format!(
        "_ctx.prepared().pass_scalar::<{}>({input}.id(),{output}.id(),{output}.generation())?;",
        rust_type(ty)
    )
}
fn key(ty: &Ty, list: &str) -> String {
    let slot = element(ty, list, "index", "source");
    format!(
        "{{let keys=&_ctx.store().keys;{}}}",
        crate::composite_keys::slot_id(ty, &slot, global_type)
    )
}
fn append_native(ty: &Ty, list: &str, value: &str) -> String {
    let marker = global_type(ty);
    format!(
        "{{let destination=hgl_store::append_slot::<{marker}>(columns,{list})?;<{marker} as hgl_store::PreparedValue>::check_native(columns,destination,{value})?;<{marker} as hgl_store::PreparedValue>::copy_native(columns,destination,{value});hgl_store::commit_append::<{marker}>(columns,{list});}}"
    )
}
fn append_key(ty: &Ty, list: &str) -> String {
    let marker = global_type(ty);
    format!(
        "{{let keys=observation.keys;let destination=hgl_store::append_slot::<{marker}>(columns,{list})?;{}hgl_store::commit_append::<{marker}>(columns,{list});}}",
        crate::composite_keys::copy(ty, "key", "destination", global_type)
    )
}
fn append_child(ty: &Ty, list: &str) -> String {
    let marker = marker(ty);
    format!(
        "{{let destination=hgl_store::append_slot::<{marker}>(columns,{list})?;{}hgl_store::commit_append::<{marker}>(columns,{list});}}",
        capture(ty, "child", "destination")
    )
}
/// Emit methods on one exact ordinary structural delta marker.
pub fn methods(ty: &Ty, shape: impl Fn(&Ty) -> String) -> String {
    let name = marker(ty);
    let shape_name = shape(ty);
    format!(
        "impl {name} {{fn validate_slot(source:&hgl_store::ValueColumns,slot:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult {{{}Ok(())}}fn apply_slot(output:hgl_store::shapes::Output<{shape_name}>,source:&hgl_store::ValueColumns,slot:hgl_store::ValueSlot<Self>,_ctx:&mut hgl_kernel::Ctx<'_>)->hgl_types::NodeResult {{Self::validate_slot(source,slot)?;{}Ok(())}} fn observe_slot(input:hgl_store::shapes::Input<{shape_name}>,observation:hgl_store::Observation<'_>,now:hgl_types::EngineTime,columns:&mut hgl_store::ValueColumns,slot:hgl_store::ValueSlot<Self>)->hgl_types::NodeResult {{{}Ok(())}} fn pass(input:hgl_store::shapes::Input<{shape_name}>,output:hgl_store::shapes::Output<{shape_name}>,_ctx:&mut hgl_kernel::Ctx<'_>)->hgl_types::NodeResult {{{}Ok(())}}}}",
        validation(ty),
        application(ty),
        observation(ty),
        passing(ty)
    )
}
fn ensure() -> String {
    "_ctx.get_or_create_with(output.id(),key,|_,_|unreachable!(\"finite child prepared before start\"));let child=output.member(_ctx.store().bindings(),key).ok_or_else(||hgl_types::NodeError::new(\"missing prepared member\"))?;".into()
}

fn application(ty: &Ty) -> String {
    if let Ty::List(child, None) = ty {
        let indices = |field| {
            format!(
                "(0..source.list(slot.fields().{field}.fields()).len()).map(|index|*source.scalar::<i64>(({}).fields()))",
                element(
                    &Ty::I64,
                    &format!("slot.fields().{field}"),
                    "index",
                    "source"
                )
            )
        };
        return format!(
            "hgl_types::validate_growing_distinct(_ctx.store().bindings().output(output.id()).members.live.len(),{},{}).map_err(hgl_types::NodeError::new)?;{}",
            indices(0),
            indices(2),
            application(&Ty::Map(Box::new(Ty::I64), child.clone()))
        );
    }
    if let Ty::Map(key_ty, child) = ty {
        return format!(
            "for index in 0..source.list(slot.fields().2.fields()).len() {{let key={};if output.member(_ctx.store().bindings(),key).is_none() {{return Err(hgl_types::NodeError::new(\"noncanonical map removal\"));}}_ctx.remove_shaped(output.id(),key);}}for index in 0..source.list(slot.fields().0.fields()).len() {{let key={};{}let value={};{}}}",
            key(key_ty, "slot.fields().2"),
            key(key_ty, "slot.fields().0"),
            ensure(),
            element(&delta_type(child), "slot.fields().1", "index", "source"),
            apply(child, "child", "source", "value")
        );
    }
    if let Ty::Set(key_ty) = ty {
        let mut code = String::new();
        for (field, addition) in [(1, false), (0, true)] {
            append(
                &mut code,
                format_args!(
                    "for index in 0..source.list(slot.fields().{field}.fields()).len() {{let key={};",
                    key(key_ty, &format!("slot.fields().{field}"))
                ),
            );
            if addition {
                code += "if output.member(_ctx.store().bindings(),key).is_some() {return Err(hgl_types::NodeError::new(\"noncanonical set addition\"));}";
                code += &ensure();
                code += "_ctx.prepared().scalar::<bool>(child.id(),child.generation(),&true)?;";
            } else {
                code += "if output.member(_ctx.store().bindings(),key).is_none() {return Err(hgl_types::NodeError::new(\"noncanonical set removal\"));}_ctx.remove_shaped(output.id(),key);";
            }
            code += "}";
        }
        return code;
    }
    if let Ty::List(child, Some(n)) = ty {
        return format!(
            "for index in 0..source.list(slot.fields().0.fields()).len() {{let key=*source.scalar::<i64>(({}).fields());let index_key=usize::try_from(key).ok().filter(|&n|n<{n}).ok_or_else(||hgl_types::NodeError::new(\"sparse list index out of bounds\"))?;let child=output.index(_ctx.store().bindings(),index_key);let value={};{}}}",
            element(&Ty::I64, "slot.fields().0", "index", "source"),
            element(&delta_type(child), "slot.fields().1", "index", "source"),
            apply(child, "child", "source", "value")
        );
    }
    fields(ty).iter().enumerate().fold(String::new(),|mut code,(i,child)|{append(&mut code,format_args!("for index in 0..source.list(slot.fields().{i}.fields()).len() {{let child=output.field::<{i}>(_ctx.store().bindings());let value={};{}}}",element(&delta_type(child),&format!("slot.fields().{i}"),"index","source"),apply(child,"child","source","value")));code})
}
fn observation(ty: &Ty) -> String {
    if let Ty::List(child, None) = ty {
        return observation(&Ty::Map(Box::new(Ty::I64), child.clone()));
    }
    let count = if matches!(ty, Ty::Map(..)) {
        3
    } else if matches!(ty, Ty::Set(_) | Ty::List(..)) {
        2
    } else {
        fields(ty).len()
    };
    let mut code = (0..count).fold(String::new(), |mut code, i| {
        append(
            &mut code,
            format_args!("columns.set_list_len(slot.fields().{i}.fields(),0);"),
        );
        code
    });
    if let Ty::Set(key_ty) = ty {
        for (field, method) in [(0, "added_keys"), (1, "removed_keys")] {
            append(
                &mut code,
                format_args!(
                    "for key in observation.bindings.{method}(input.id()) {{{}}}",
                    append_key(key_ty, &format!("slot.fields().{field}"))
                ),
            );
        }
        return code;
    }
    if let Ty::Map(key_ty, child) = ty {
        return code
            + &format!(
                "for &key in observation.bindings.changed_keys(input.id()) {{if let Some(child)=input.member(observation.bindings,key) {{{}{}}}}}for key in observation.bindings.removed_keys(input.id()) {{{}}}",
                append_key(key_ty, "slot.fields().0"),
                append_child(child, "slot.fields().1"),
                append_key(key_ty, "slot.fields().2")
            );
    }
    if let Ty::List(child, Some(n)) = ty {
        return code
            + &format!(
                "for index in 0..{n} {{let child=input.index(observation.bindings,index);if observation.bindings.modified(child.id(),now) {{{}{}}}}}",
                append_native(&Ty::I64, "slot.fields().0", "&(index as i64)"),
                append_child(child, "slot.fields().1")
            );
    }
    for (i, child) in fields(ty).iter().enumerate() {
        append(
            &mut code,
            format_args!(
                "{{let child=input.field::<{i}>(observation.bindings);if observation.bindings.modified(child.id(),now) {{{}}}}}",
                append_child(child, &format!("slot.fields().{i}"))
            ),
        );
    }
    code
}
fn passing(ty: &Ty) -> String {
    if let Ty::List(child, None) = ty {
        return format!(
            "hgl_types::validate_growing_distinct(_ctx.store().bindings().output(output.id()).members.live.len(),_ctx.store().bindings().changed_keys(input.id()).iter().copied().filter(|key|input.member(_ctx.store().bindings(),*key).is_some()),(0.._ctx.store().bindings().input(input.id()).members.initial.len()).filter_map(|index|{{let (key,was)=_ctx.store().bindings().input(input.id()).members.initial.item(index)?;(*was && input.member(_ctx.store().bindings(),key).is_none()).then_some(key)}})).map_err(hgl_types::NodeError::new)?;{}",
            passing(&Ty::Map(Box::new(Ty::I64), child.clone()))
        );
    }
    if let Ty::Map(_, child) | Ty::Set(child) = ty {
        let is_set = matches!(ty, Ty::Set(_));
        let child = if is_set { &Ty::Bool } else { child.as_ref() };
        let publish = if is_set {
            "_ctx.prepared().scalar::<bool>(child.id(),child.generation(),&true)?;".into()
        } else {
            pass(child, "source_child", "child")
        };
        let added = if is_set {
            "if _ctx.store().bindings().input(input.id()).members.initial.get(key)==Some(&false)"
        } else {
            ""
        };
        return format!(
            "let mut removed=0;while let Some((key,was))=_ctx.store().bindings().input(input.id()).members.initial.item(removed).map(|(key,was)|(key,*was)) {{removed+=1;if was && input.member(_ctx.store().bindings(),key).is_none() {{_ctx.remove_shaped(output.id(),key);}}}}let mut index=0;while let Some(key)=_ctx.store().bindings().changed_keys(input.id()).get(index).copied() {{index+=1;if let Some(source_child)=input.member(_ctx.store().bindings(),key) {{{added} {{{}{publish}}}}}else {{_ctx.remove_shaped(output.id(),key);}}}}",
            ensure()
        );
    }
    if let Ty::List(child, Some(n)) = ty {
        return format!(
            "for index in 0..{n} {{let source_child=input.index(_ctx.store().bindings(),index);if _ctx.store().bindings().modified(source_child.id(),_ctx.evaluation_time()) {{let child=output.index(_ctx.store().bindings(),index);{}}}}}",
            pass(child, "source_child", "child")
        );
    }
    fields(ty).iter().enumerate().fold(String::new(),|mut code,(i,child)|{append(&mut code,format_args!("{{let source_child=input.field::<{i}>(_ctx.store().bindings());if _ctx.store().bindings().modified(source_child.id(),_ctx.evaluation_time()) {{let child=output.field::<{i}>(_ctx.store().bindings());{}}}}}",pass(child,"source_child","child")));code})
}

fn validation(ty: &Ty) -> String {
    if let Ty::List(child, None) = ty {
        return validation(&Ty::Map(Box::new(Ty::I64), child.clone()));
    }
    let list_len = |i| format!("source.list(slot.fields().{i}.fields()).len()");
    let (nonempty, children) = if let Ty::Set(_) = ty {
        (vec![0, 1], Vec::new())
    } else if let Ty::Map(_, child) = ty {
        (vec![0, 2], vec![(1, child.as_ref())])
    } else if let Ty::List(child, _) = ty {
        (vec![0], vec![(1, child.as_ref())])
    } else {
        (
            (0..fields(ty).len()).collect(),
            fields(ty).into_iter().enumerate().collect(),
        )
    };
    let condition = nonempty
        .iter()
        .map(|i| format!("{}==0", list_len(*i)))
        .collect::<Vec<_>>()
        .join(" && ");
    let mut code = format!(
        "if {} {{return Err(hgl_types::NodeError::new(\"empty structural delta application is outside the supported profile\"));}}",
        if condition.is_empty() {
            "true"
        } else {
            &condition
        }
    );
    if matches!(ty, Ty::Map(..) | Ty::List(..)) {
        append(
            &mut code,
            format_args!(
                "if {}!={} {{return Err(hgl_types::NodeError::new(\"malformed sparse delta\"));}}",
                list_len(0),
                list_len(1)
            ),
        );
    }
    for (i, child) in children {
        if matches!(ty, Ty::Struct(..) | Ty::Tuple(_)) {
            append(
                &mut code,
                format_args!(
                    "if {}>1 {{return Err(hgl_types::NodeError::new(\"malformed sparse field\"));}}",
                    list_len(i)
                ),
            );
        }
        if structural(child) {
            append(
                &mut code,
                format_args!(
                    "for index in 0..{} {{{}::validate_slot(source,{})?;}}",
                    list_len(i),
                    marker(child),
                    element(
                        &delta_type(child),
                        &format!("slot.fields().{i}"),
                        "index",
                        "source"
                    )
                ),
            );
        }
    }
    code
}

mod text;
pub use text::text;
/// Read one checked child endpoint using its statically selected transport.
pub fn read(ty: &Ty, input: &str, operation: Option<String>) -> String {
    if matches!(ty, Ty::Rolling(..)) {
        return crate::windows::read(ty, input);
    }
    if let Some(payload) = whole_payload(ty) {
        return format!(
            "_ctx.store().atomic_get::<{}>({input})?",
            global_type(payload)
        );
    }
    if let Some(operation) = operation {
        format!("{operation}::observe({input},_ctx)?")
    } else {
        format!(
            "hgl_store::Scalar::try_clone(_ctx.store().get_ref(hgl_store::Store::prepared_input({input})))?"
        )
    }
}

/// Render an exact checked input endpoint binding for scalar or shaped queries.
/// # Panics
/// Panics if the supplied IR is not an input or a scoped iteration input.
pub fn input(v: &Value) -> String {
    if let Kind::Input(i, _) = v.kind {
        format!("self.input{i}")
    } else if let Kind::IterationInput(id) = v.kind {
        if structural(&v.ty) || whole_payload(&v.ty).is_some() || matches!(v.ty, Ty::Rolling(..)) {
            format!("local{id}")
        } else {
            format!("hgl_store::Store::prepared_input(local{id})")
        }
    } else {
        unreachable!("checked endpoint query")
    }
}
