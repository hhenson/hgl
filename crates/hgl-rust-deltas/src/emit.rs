use super::append;
use super::{
    Ty, allocation, apply, children, delta_type, empty, operations, owned_type, read, reserve,
    shape_marker, structural,
};

pub(super) fn marker(ty: &Ty) -> String {
    let marker = shape_marker(ty);
    let mut code = String::new();
    let fields = children(ty);
    if matches!(ty, Ty::Struct(..) | Ty::Tuple(_)) {
        let names = if let Ty::Struct(_, fields) = ty {
            fields.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>()
        } else {
            (0..fields.len()).map(|i| i.to_string()).collect()
        };
        let schema = fields
            .iter()
            .zip(names)
            .map(|(t, n)| {
                format!(
                    "({n:?}.into(),<{} as hgl_store::shapes::Shape>::shape())",
                    shape_marker(t)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        append(
            &mut code,
            format_args!(
                "#[derive(Debug)] struct {marker}; impl hgl_store::shapes::Shape for {marker} {{fn shape()->hgl_types::TsType {{hgl_types::TsType::Bundle(vec![{schema}])}}}}\n"
            ),
        );
        for (i, child) in fields.iter().enumerate() {
            append(
                &mut code,
                format_args!(
                    "impl hgl_store::shapes::Field<{i}> for {marker} {{type Child={};}}\n",
                    shape_marker(child)
                ),
            );
        }
    }
    let name = operations(ty);
    let value = owned_type(&delta_type(ty));
    append(
        &mut code,
        format_args!(
            "impl {name} {{ fn allocate(store:&mut hgl_store::Store,owner:hgl_types::NodeId)->hgl_store::OutputId {{ {} }}\n",
            allocate(ty)
        ),
    );
    append(
        &mut code,
        format_args!(
            "fn validate(delta:&{value})->hgl_types::NodeResult {{ {} Ok(()) }}\n",
            validate(ty)
        ),
    );
    append(
        &mut code,
        format_args!(
            "fn observe(input:hgl_store::shapes::Input<{marker}>,_ctx:&hgl_kernel::Ctx<'_>)->hgl_types::NodeResult<{value}> {{ let mut delta:{value}={}; {} Ok(delta) }}\n",
            empty(ty),
            observation(ty)
        ),
    );
    append(
        &mut code,
        format_args!(
            "fn equivalent(a:&{value},b:&{value})->bool {{ {} }}\n",
            equality(ty)
        ),
    );
    append(
        &mut code,
        format_args!(
            "fn apply(output:hgl_store::shapes::Output<{marker}>,delta:{value},_ctx:&mut hgl_kernel::Ctx<'_>)->hgl_types::NodeResult {{ Self::validate(&delta)?; {} Ok(()) }} }}\n",
            application(ty)
        ),
    );
    code
}
fn allocate(ty: &Ty) -> String {
    if let Ty::Set(key) = ty {
        return hgl_rust_keyed::allocation(
            key,
            &shape_marker(ty),
            "store.add_output::<bool>(owner).id()",
        );
    }
    if let Ty::Map(key, child) = ty {
        return hgl_rust_keyed::allocation(key, &shape_marker(ty), &allocation(child));
    }
    let mut code = "let mut children=Vec::new();".to_string();
    if let Ty::List(child, Some(n)) = ty {
        append(
            &mut code,
            format_args!("for _ in 0..{n} {{children.push({});}}", allocation(child)),
        );
    } else {
        for child in children(ty) {
            append(
                &mut code,
                format_args!("children.push({});", allocation(child)),
            );
        }
    }
    append(
        &mut code,
        format_args!(
            "store.add_prepared_output(owner,<{} as hgl_store::shapes::Shape>::shape(),children)",
            shape_marker(ty)
        ),
    );
    code
}
fn nonempty(condition: &str) -> String {
    format!(
        "if {condition} {{return Err(hgl_types::NodeError::new(\"empty structural delta application is outside the supported profile\"));}}"
    )
}
fn child_validation(child: &Ty, source: &str) -> String {
    if structural(child) {
        format!("{}::validate({source})?;", operations(child))
    } else {
        String::new()
    }
}
fn validate(ty: &Ty) -> String {
    match ty {
        Ty::Set(_) => nonempty("delta.0.is_empty() && delta.1.is_empty()"),
        Ty::List(child, _) | Ty::Map(_, child) => {
            let cond = if matches!(ty, Ty::Map(..)) {
                "delta.0.is_empty() && delta.2.is_empty()"
            } else {
                "delta.0.is_empty()"
            };
            format!(
                "{} if delta.0.len()!=delta.1.len() {{ return Err(hgl_types::NodeError::new(\"malformed sparse delta\")); }} for child in &delta.1 {{{}}}",
                nonempty(cond),
                child_validation(child, "child")
            )
        }
        Ty::Struct(..) | Ty::Tuple(_) => {
            let fields = children(ty);
            let condition = if fields.is_empty() {
                "true".into()
            } else {
                (0..fields.len())
                    .map(|i| format!("delta.{i}.is_empty()"))
                    .collect::<Vec<_>>()
                    .join(" && ")
            };
            let mut code = nonempty(&condition);
            for (i, child) in fields.iter().enumerate() {
                append(
                    &mut code,
                    format_args!(
                        "if delta.{i}.len()>1 {{return Err(hgl_types::NodeError::new(\"malformed sparse field\"));}} for child in &delta.{i} {{{}}}",
                        child_validation(child, "child")
                    ),
                );
            }
            code
        }
        Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::Enum(_)
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Ref(_)
        | Ty::Nullable(_)
        | Ty::Void => unreachable!("structural origin"),
    }
}
fn push(target: &str, value: &str) -> String {
    format!("{} {target}.push({value});", reserve(target))
}
fn observation(ty: &Ty) -> String {
    match ty {
        Ty::Set(key) => hgl_rust_keyed::observation(key, None),
        Ty::Map(key, child) => hgl_rust_keyed::observation(key, Some(&read(child, "child"))),
        Ty::List(child, Some(n)) => format!(
            "for n in 0..{n} {{let child=input.index(_ctx.store().bindings(),n); {} }}",
            observe_child(
                child,
                "i64::try_from(n).map_err(|e|hgl_types::NodeError::new(e.to_string()))?",
                "delta.1",
                true
            )
        ),
        Ty::Struct(..) | Ty::Tuple(_) => children(ty)
            .iter()
            .enumerate()
            .map(|(i, child)| {
                format!(
                    "{{let child=input.field::<{i}>(_ctx.store().bindings()); {}}}",
                    observe_child(child, "", &format!("delta.{i}"), false)
                )
            })
            .collect::<Vec<_>>()
            .concat(),
        Ty::Atomic(_)
        | Ty::Delta(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::Enum(_)
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Ref(_)
        | Ty::Nullable(_)
        | Ty::Void
        | Ty::List(_, None) => unreachable!("structural origin"),
    }
}
fn observe_child(child: &Ty, index: &str, target: &str, keys: bool) -> String {
    let key = if keys {
        push("delta.0", index)
    } else {
        String::new()
    };
    format!(
        "if _ctx.store().bindings().modified(child.id(),_ctx.evaluation_time()) {{if !_ctx.store().input_valid(child.id()) {{return Err(hgl_types::NodeError::new(\"invalid structural child delta\"));}} let value={}; {key} {} }}",
        read(child, "child"),
        push(target, "value")
    )
}
fn application(ty: &Ty) -> String {
    match ty {
        Ty::Set(key)=>hgl_rust_keyed::application(key,None),
        Ty::Map(key,child)=>hgl_rust_keyed::application(key,Some((&allocation(child),&apply(child,"child","value")))),
        Ty::List(child,Some(n))=>format!("for (key,value) in delta.0.into_iter().zip(delta.1) {{let n=usize::try_from(key).ok().filter(|&n|n<{n}).ok_or_else(||hgl_types::NodeError::new(\"sparse list index out of bounds\"))?; let child=output.index(_ctx.store().bindings(),n); {} }}",apply(child,"child","value")),
        Ty::Struct(..)|Ty::Tuple(_)=>children(ty).iter().enumerate().map(|(i,child)|format!("for value in delta.{i} {{let child=output.field::<{i}>(_ctx.store().bindings()); {}}}",apply(child,"child","value"))).collect::<Vec<_>>().concat(),
        Ty::Atomic(_) | Ty::Delta(_) | Ty::I64 | Ty::F64 | Ty::Bool | Ty::Str | Ty::CivilDateTime | Ty::TimeZone | Ty::Enum(_) | Ty::ZonedTime | Ty::ZonedDateTime | Ty::Duration | Ty::Date | Ty::Time | Ty::DateTime | Ty::Ref(_) | Ty::Nullable(_) | Ty::Void | Ty::List(_,None)=>unreachable!("structural origin"),
    }
}

fn equality(ty: &Ty) -> String {
    let members = |slot| {
        format!(
            "a.{slot}.len()==b.{slot}.len() && a.{slot}.iter().all(|key|b.{slot}.contains(key))"
        )
    };
    if matches!(ty, Ty::Set(_)) {
        return format!("{} && {}", members(0), members(1));
    }
    if let Ty::List(child, _) | Ty::Map(_, child) = ty {
        let child = super::equivalent(&delta_type(child), "value", "&b.1[index]");
        let removed = if matches!(ty, Ty::Map(..)) {
            format!(" && {}", members(2))
        } else {
            String::new()
        };
        return format!(
            "a.0.len()==b.0.len() && a.0.iter().zip(&a.1).all(|(key,value)|b.0.iter().position(|candidate|candidate==key).is_some_and(|index| {child})){removed}"
        );
    }
    let terms = children(ty)
        .iter()
        .enumerate()
        .map(|(i, child)| {
            format!(
                "a.{i}.len()==b.{i}.len() && a.{i}.iter().zip(&b.{i}).all(|(a,b)| {})",
                super::equivalent(&delta_type(child), "a", "b")
            )
        })
        .collect::<Vec<_>>();
    if terms.is_empty() {
        "true".into()
    } else {
        terms.join(" && ")
    }
}
