//! Prepared lexical box owners preserve borrows through independently reserved slots.
use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::Ty;
fn boxed_locals(body: &mut [Statement]) {
    let owners = hgl_semantics::boxed_uses::exclusive(body, |v| {
        v.ty != Ty::Any
            && (matches!(v.kind, Kind::Construct(_))
                || matches!(&v.kind,Kind::Unary(op,_) if op=="family"))
    });
    hgl_semantics::ir_visit::nodes_mut(body, &mut |s| {
        if let Statement::Let(id, v) = s
            && owners.contains(id)
        {
            *v = Value::new(Ty::Any, Kind::Unary("any".into(), Box::new(v.clone())));
        }
    });
    hgl_semantics::ir_visit::statements_mut(body, &mut |v| {
        if let Kind::Unary(op, child) = &v.kind
            && op == "any"
            && let Kind::Local(id) = child.kind
            && owners.contains(&id)
        {
            *v = Value::new(Ty::Any, Kind::Local(id));
        }
        !matches!(v.kind, Kind::ValueCall(..))
    });
}
fn bindings(body: &mut [Statement]) {
    boxed_locals(body);
    hgl_semantics::ir_visit::nodes_mut(body, &mut |s| {
        if let Statement::Let(_, v)
        | Statement::Var(_, v)
        | Statement::Return(v)
        | Statement::Assign(_, v) = s
            && v.ty == Ty::Any
        {
            v.snapshot = true;
        }
    });
}
fn mark(value: &mut Value) -> bool {
    if let Kind::ValueCall(args, body) = &mut value.kind {
        bindings(body);
        hgl_semantics::ir_visit::statements_mut(body, &mut |v| mark(v));
        for value in args {
            hgl_semantics::ir_visit::value_mut(value, &mut |v| mark(v));
        }
        return false;
    }
    if value.ty == Ty::Any
        && let Kind::ObservedLocal(id) = value.kind
    {
        value.kind = Kind::Local(id);
    }
    if value.ty == Ty::Any && matches!(value.kind, Kind::Local(_) | Kind::MutableLocal(_)) {
        value.snapshot = true;
    }
    true
}
/// Reserve owning Any locals separately from borrowed entry handles.
pub fn prepare(plan: &Plan) -> Plan {
    if !crate::execution_proof::prepared(plan) {
        return plan.clone();
    }
    let mut plan = plan.clone();
    hgl_semantics::call_locals::prepare(&mut plan);
    for node in &mut plan.nodes {
        let mut globals = std::mem::take(&mut node.globals);
        let mut prepare_payload = |v: &mut Value| {
            if let Kind::Unary(op, arg) = &mut v.kind
                && op == "any"
                && matches!(arg.ty, Ty::Delta(_))
                && matches!(arg.kind,Kind::Query(ref op,_) if op=="delta_value")
            {
                let key = format!("\0any-payload:{}", arg.ty.source_name());
                let id = globals
                    .iter()
                    .position(|(name, _)| *name == key)
                    .unwrap_or_else(|| {
                        let id = globals.len();
                        globals.push((key, arg.ty.clone()));
                        id
                    });
                **arg = Value::new(
                    arg.ty.clone(),
                    Kind::Query(format!("\0any-capture:{id}"), vec![*arg.clone()]),
                );
            }
            true
        };
        hgl_semantics::ir_visit::statements_mut(&mut node.start, &mut prepare_payload);
        for (_, body) in &mut node.handlers {
            hgl_semantics::ir_visit::statements_mut(body, &mut prepare_payload);
        }
        node.globals = globals;
        node.global_state |= !node.globals.is_empty();
        bindings(&mut node.start);
        bindings(&mut node.stop);
        for (_, body) in &mut node.handlers {
            bindings(body);
        }
        hgl_semantics::ir_visit::hooks_mut(node, &mut |v| mark(v));
    }
    plan
}
fn copied(plan: &Plan, value: &Value, destination: &str) -> Option<String> {
    let (columns, source) = crate::source_slots::source(plan, value)?;
    let marker = crate::layouts::global_type(&value.ty);
    let source_columns = if columns == "global_values" {
        "columns"
    } else {
        crate::source_slots::columns(columns)
    };
    let copy = if columns == "global_values" {
        format!("copy_within(columns,source,{destination})")
    } else {
        format!("copy_between({source_columns},source,columns,{destination})")
    };
    let copied = format!(
        "<{marker} as hgl_store::PreparedValue>::check_slots({source_columns},source,columns,{destination})?;<{marker} as hgl_store::PreparedValue>::{copy};"
    );
    Some(format!(
        "let source={source};let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();{copied}"
    ))
}
/// Retain complete boxes before any borrowed source can be replaced.
pub fn write(
    plan: &Plan,
    value: &Value,
    destination: &str,
    emit: impl Fn(&Value) -> String,
) -> String {
    if let Kind::Query(op, args) = &value.kind
        && op == "delta_value"
    {
        let input = &args[0];
        let capture =
            crate::observed::capture(&input.ty, &crate::observed::input(input), destination);
        return format!(
            "let now=_ctx.evaluation_time();let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();{capture}"
        );
    }
    if let Some(code) = copied(plan, value, destination) {
        return code;
    }
    if let Kind::Unary(op, argument) = &value.kind
        && op == "any"
    {
        if argument.ty == Ty::Any {
            return write(plan, argument, destination, emit);
        }
        let n = crate::any_values::name(&argument.ty);
        if let Kind::Query(op, args) = &argument.kind
            && let Some(id) = op.strip_prefix("\0any-capture:")
        {
            let Kind::Query(_, inputs) = &args[0].kind else {
                unreachable!("prepared observation")
            };
            let Kind::Input(input, _) = inputs[0].kind else {
                unreachable!("checked observation input")
            };
            let Ty::Delta(origin) = &argument.ty else {
                unreachable!("checked structural delta")
            };
            let capture = crate::observed::capture(origin, &format!("self.input{input}"), "source");
            return format!(
                "let source=_ctx.global_state().destination(self.global{id});let now=_ctx.evaluation_time();let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();{capture}columns.compose_any({destination},|columns|{{let mut width=hgl_types::any_values::AnyWidth::default();let plan=any_box_slots_{n}(columns,source,&mut width);Ok((plan,width.0))}},|columns,out|{{any_box_slots_{n}(columns,source,out);}})?;"
            );
        }
        if let Some((columns, source)) = crate::source_slots::source(plan, argument) {
            let source_columns = crate::source_slots::columns(columns);
            let boxed = crate::any_values::prepared_source(
                &argument.ty,
                source_columns,
                "source",
                destination,
            );
            return format!(
                "let source={source};let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();{boxed}"
            );
        }
        let mut prelude = Vec::new();
        if let Some(code) = crate::any_expression::encoding(plan, argument, &emit, &mut prelude) {
            return format!(
                "{}let mut prepared=_ctx.prepared();let(observation,globals)=prepared.storage.observations();let columns=globals.values_mut();{}",
                prelude.concat(),
                crate::any_expression::compose(&code, destination)
            );
        }
        let value = emit(argument);
        let boxed = crate::any_values::prepared_native(&argument.ty, "value", destination);
        return format!("let value={value};let columns=_ctx.global_state().values_mut();{boxed}");
    }
    format!(
        "let value={};let columns=_ctx.global_state().values_mut();<hgl_store::Any as hgl_store::PreparedValue>::check_native(columns,{destination},&value)?;<hgl_store::Any as hgl_store::PreparedValue>::copy_native(columns,{destination},&value);",
        emit(value)
    )
}
/// Initialize one independently reserved box local without changing lexical authority.
pub fn local(plan: &Plan, id: usize, value: &Value, emit: impl Fn(&Value) -> String) -> String {
    format!(
        "let local{id}={{let destination=_ctx.global_state().destination(self.snapshot{id});{}(destination,true)}};",
        write(plan, value, "destination", emit)
    )
}
/// Borrow a complete box without retaining an intermediate owner.
pub(crate) fn view(plan: &Plan, value: &Value) -> String {
    if let Some((columns, slot)) = crate::source_slots::source(plan, value) {
        let columns = if columns == "configuration_columns" {
            "self.configuration_columns".into()
        } else {
            format!("_ctx.store().{columns}()")
        };
        return format!("{{let source={slot};{columns}.any(source.fields())}}");
    }
    format!("&({})", crate::values::place(plan, value))
}
