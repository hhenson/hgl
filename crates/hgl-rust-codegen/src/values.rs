//! Checked hook expression and statement emission.
pub use crate::layouts::{
    global_markers, global_schema, global_type, owned_type, rust_type, scalar_type, whole_payload,
};
use crate::prepared_hooks as prepared;
use hgl_semantics::ir::{Kind, Plan, Statement, Value};
use hgl_source::{Literal, Ty};

pub use crate::scalars::literal;
fn value(plan: &Plan, v: &Value) -> String {
    if let Some(read) = crate::source_slots::read(plan, v, |v| value(plan, v)) {
        return read;
    }
    if v.snapshot {
        return crate::snapshot_slots::read(v, |v| value(plan, v));
    }
    match &v.kind {
        Kind::Prepared(id) => retained(&format!("prepared{id}"), &v.ty),
        Kind::Opaque(_) | Kind::TemporalLiteral(_) => {
            unreachable!("preparation expressions cannot reach node hooks")
        }
        Kind::WiringFailure(message) => {
            format!("return Err(hgl_types::NodeError::new({message:?}))")
        }
        Kind::Delta(_) => crate::deltas::construct(v, |v| value(plan, v)),
        Kind::ObservedLocal(id) => crate::deltas::observe(&v.ty, &format!("local{id}")),
        Kind::List(values) if matches!(v.ty, Ty::Set(_) | Ty::Map(..)) => {
            crate::collections::construct(&v.ty, values, |v| value(plan, v), owned_type)
        }
        Kind::List(values) => list_value(plan, &v.ty, values),
        Kind::Length(parent) => length(plan, parent),
        Kind::Push(parent, item) => push(plan, parent, item),
        Kind::ValueCall(args, body) => direct_call(plan, &v.ty, args, body),
        Kind::Configuration(id) => retained(&format!("self.configuration{id}"), &v.ty),
        Kind::Construct(fields) if v.ty == Ty::Any => {
            crate::any_values::constructor(fields, |v| value(plan, v))
        }
        Kind::Construct(fields) => crate::snapshots::ordinary(&v.ty, fields, |v| value(plan, v)),
        Kind::Field(..) | Kind::Index(..)
            if matches!(v.ty, Ty::Ref(_)) && hgl_semantics::tuple_values::endpoint(v) =>
        {
            reference_value(plan, v)
        }
        Kind::Field(..) if hgl_semantics::tuple_values::endpoint(v) => {
            crate::snapshot_slots::endpoint_read(v)
        }
        Kind::Index(..) | Kind::Field(..) | Kind::BorrowedLocal(..) => {
            if let Some(slot) = borrowed_place(plan, v) {
                format!("{{ let slot = {slot}; _ctx.global_state().read(slot)? }}")
            } else {
                retained(&place(plan, v), &v.ty)
            }
        }
        Kind::IsPresent(v) => presence(plan, v),
        Kind::Present(v) => format!(
            "({}).expect(\"checked present replay slot\")",
            value(plan, v)
        ),
        Kind::Literal(l) => literal(l),
        Kind::IterationInput(_) => crate::snapshot_slots::endpoint_read(v),
        Kind::Input(i, _) => input_value(v, *i),
        Kind::Cache(i) => retained(&format!("self.cache{i}"), &v.ty),
        Kind::GlobalGet(i) => format!("_ctx.global_state().get(self.global{i})?"),
        Kind::GlobalSet(i, v) => prepared::set(plan, *i, v),
        Kind::GeneratorLocal(_) => retained(&place(plan, v), &v.ty),
        Kind::Local(i) | Kind::MutableLocal(i) => retained(&format!("local{i}"), &v.ty),
        Kind::Native(binding, args) => native_call(plan, binding, args),
        Kind::Binary(op, a, b) => binary(plan, &v.ty, op, a, b),
        Kind::Unary(op, operand) if op == "family" => {
            crate::layouts::family_coerce(&v.ty, &operand.ty, &value(plan, operand))
        }
        Kind::Unary(op, v) => unary(plan, op, v),
        Kind::Query(op, args) => query_value(plan, op, args),
        Kind::Output if matches!(v.ty, Ty::Ref(_)) => {
            "_ctx.store().bindings().reference_value(self._output)".into()
        }
        Kind::Output => "_ctx.output_value(self._output).expect(\"valid output\")".into(),
        Kind::Captured(..) | Kind::Wire(_) | Kind::Void | Kind::Capability => {
            unreachable!("checked runtime value")
        }
    }
}
fn input_value(v: &Value, id: usize) -> String {
    if matches!(v.ty, Ty::Enum(_) | Ty::Any) {
        format!(
            "_ctx.store().atomic_get::<{}>(self.input{id})?",
            global_type(&v.ty)
        )
    } else if matches!(v.ty, Ty::Ref(_)) {
        format!("_ctx.store().bindings().input_reference(self.input{id})")
    } else {
        format!("_ctx.get(self.input{id})")
    }
}
fn reference_value(plan: &Plan, v: &Value) -> String {
    let input = crate::observed::input_with(v, &|index| value(plan, index));
    let input = if matches!(v.ty, Ty::Ref(_)) {
        input
    } else {
        format!("({input}).id()")
    };
    format!("_ctx.store().bindings().input_reference({input})")
}
fn query_value(plan: &Plan, op: &str, args: &[Value]) -> String {
    if op.starts_with("\0lookup_") {
        map_lookup(plan, op, args)
    } else if op == "\0reference" {
        reference_value(plan, &args[0])
    } else if op.starts_with("\0family-eq:") {
        crate::owner_comparisons::comparison(plan, op, args)
    } else if op.starts_with("collection_") {
        crate::deltas::collection_operation(op, args, |v| value(plan, v))
    } else if op.contains('.') {
        capability_call(plan, op, args)
    } else if op.starts_with("set_") {
        set_query(plan, op, args)
    } else {
        query_with(op, args, |v| {
            crate::observed::input_with(v, &|index| value(plan, index))
        })
    }
}
fn unary(plan: &Plan, op: &str, operand: &Value) -> String {
    if op == "any" {
        return crate::any_values::construct(&operand.ty, &value(plan, operand));
    }
    let value = value(plan, operand);
    crate::scalars::unary(&operand.ty, op, &value)
}

pub(crate) fn place(plan: &Plan, v: &Value) -> String {
    if let Kind::GeneratorLocal(id) = v.kind {
        return format!(
            "(*self.generator_local{id}.as_ref().expect(\"initialized generator local\"))"
        );
    }
    if let Kind::Local(i) | Kind::MutableLocal(i) = &v.kind {
        return format!("local{i}");
    }
    if let Kind::Configuration(id) = v.kind {
        return format!("self.configuration{id}");
    }
    if let Kind::Index(parent, index) = &v.kind {
        return format!(
            "(*hgl_store::list_index(&({}), {})?)",
            place(plan, parent),
            value(plan, index)
        );
    }
    if let Kind::Field(parent, index) = &v.kind {
        if let Ty::Family(family) = &parent.ty {
            return crate::collections::family_field(family, *index, &place(plan, parent), false);
        }
        if parent.snapshot {
            return format!(
                "(*({}).{index}.as_ref().ok_or_else(||hgl_types::NodeError::new(\"absent ordinary tuple child\"))?)",
                place(plan, parent)
            );
        }
        return format!("({}).{index}", place(plan, parent));
    }
    value(plan, v)
}

fn binary(plan: &Plan, result: &Ty, op: &str, a: &Value, b: &Value) -> String {
    if a.ty == Ty::Any {
        return crate::any_values::comparison(
            op,
            &crate::any_prepared::view(plan, a),
            &crate::any_prepared::view(plan, b),
        );
    }
    if matches!(a.ty, Ty::Native(_)) {
        if !crate::execution_proof::prepared(plan) {
            return format!(
                "{{let left_owner={};let right_owner={};{}}}",
                value(plan, a),
                value(plan, b),
                crate::any_values::comparison(op, "left_owner.view()", "right_owner.view()")
            );
        }
        return crate::any_values::comparison(
            op,
            &native_argument(plan, a),
            &native_argument(plan, b),
        );
    }
    if matches!(
        a.ty,
        Ty::Set(_) | Ty::Map(..) | Ty::Family(_) | Ty::Tuple(_)
    ) && matches!(op, "==" | "!=")
    {
        return format!(
            "{{let left={};let right={};({})=={}}}",
            value(plan, a),
            value(plan, b),
            crate::collections::equal(&a.ty, "&left", "&right", global_type),
            op == "=="
        );
    }
    let a = if a.ty == Ty::Bytes {
        byte_view(plan, a)
    } else if a.ty == Ty::Str {
        native_argument(plan, a)
    } else {
        value(plan, a)
    };
    let b = if b.ty == Ty::Bytes {
        byte_view(plan, b)
    } else if b.ty == Ty::Str {
        native_argument(plan, b)
    } else {
        value(plan, b)
    };
    if op == "+" && *result == Ty::Str {
        format!("format!(\"{{}}{{}}\", {a}, {b})")
    } else {
        crate::scalars::binary(result, op, &a, &b)
    }
}
fn presence(plan: &Plan, value: &Value) -> String {
    let option = if let Kind::Local(id) = value.kind {
        format!("local{id}")
    } else {
        self::value(plan, value)
    };
    format!("({option}).is_some()")
}
/// Emit the checked statements form.
pub fn statements(plan: &Plan, body: &[Statement], out: &mut Vec<String>, result: Option<&Ty>) {
    for statement in body {
        out.push(match statement {
            Statement::TimedYield(..) => unreachable!("generator yields use resume lowering"),
            Statement::While(condition, body) => {
                let mut code = vec![format!("while {} {{\n", condition_code(plan, condition))];
                statements(plan, body, &mut code, result);
                code.push("}\n".into());
                code.concat()
            }
            Statement::Yield(v) => format!("return Ok({});\n", value(plan, v)),
            Statement::Exit => "return Ok(());\n".into(),
            Statement::Let(i,Value {kind:Kind::Query(op,args),..}) | Statement::Var(i,Value {kind:Kind::Query(op,args),..}) if op.starts_with("\0constructor:") && args.len()==2 => crate::constructor_owners::local(plan,*i,&args[0],op),
            Statement::Let(i,v) | Statement::Var(i,v) if v.snapshot && matches!(v.ty,Ty::Any|Ty::Native(_)) => crate::any_prepared::local(plan,*i,v,|v|value(plan,v)),
            Statement::Let(i,v) if v.snapshot => crate::snapshot_slots::local(*i,v, |v| value(plan,v)),
            Statement::Let(i,v) => {
                let slot = if matches!(v.ty, Ty::Native(_)) && !crate::execution_proof::prepared(plan) {
                    None
                } else {
                    atomic_place(plan,v).or_else(||observation(v))
                };
                let annotation=if slot.is_none()&&matches!(v.ty,Ty::Family(_)) {format!(":{}",owned_type(&v.ty))} else {String::new()};
                let init = slot.unwrap_or_else(||condition_code(plan,v));
                format!("let local{i}{annotation} = {init};\n")
            },
            Statement::Borrow(i, v, _) => {
                let source = if let Kind::GlobalGet(entry) = v.kind {
                    format!("_ctx.global_state().borrow(self.global{entry})?")
                } else { borrowed_place(plan, v).unwrap_or_else(|| unreachable!("checked borrowed initializer")) };
                format!("let local{i} = {source};\n")
            },
            Statement::Var(i, v) => format!("let mut local{i} = {};\n", condition_code(plan, v)),
            Statement::Return(v) => {
                if let Some(code)=prepared::publication(plan,v,result) {out.push(format!("{code}return Ok(());"));continue;}
                let publication=if v.snapshot {crate::snapshot_views::publication(v,|v|value(plan,v))}else{condition_code(plan,v)};
                if let Some(code)=crate::snapshot_slots::returned(v,&publication,result) {code} else {
                    let publish=if matches!(v.ty,Ty::Ref(_)) {"_ctx.set_reference(self._output, publication)?;".into()} else {result.filter(|ty|matches!(ty,Ty::Rolling(..))).map_or_else(||crate::deltas::publish(&v.ty,"publication"),|ty|crate::windows::apply(ty,"self._output","publication"))};
                    format!("let publication={publication};{publish}return Ok(());\n")
                }
            },
            Statement::Call(v) => format!("{};\n", condition_code(plan, v)),
            Statement::ForItems(key_id, child_id, modified, collection, body) => {
                let Kind::Input(input, _) = collection.kind else { unreachable!("checked map input"); };
                let mut code = vec![crate::collections::iteration_start(&collection.ty, &format!("self.input{input}"), (*key_id,*child_id), *modified, global_type)];
                statements(plan,body,&mut code,result);
                code.push("}}".into());
                code.concat()
            }
            Statement::For(id,collection,body)=> {
                let Kind::Input(input, _)=collection.kind else {unreachable!("checked collection")};
                let Ty::Set(element)=&collection.ty else {unreachable!("checked collection")};
                let mut code=vec![format!("let mut index{id}=0;\nwhile let Some(key{id})=_ctx.store().bindings().changed_keys(self.input{input}.id()).get(index{id}).copied() {{\nindex{id}+=1;\nif _ctx.store().bindings().input(self.input{input}.id()).members.initial.get(key{id})==Some(&false) && _ctx.store().bindings().child_input(self.input{input}.id(),key{id}).is_some() {{\nlet local{id}={};\n",if **element==Ty::Bool {format!("key{id}!=0")} else {format!("key{id}")})];
                statements(plan,body,&mut code,result);code.push("}\n}\n".into());code.concat()
            }
            Statement::Assign(target,v) => assignment(plan, target, v),
            Statement::If(condition, yes, no) => {
                let mut code = vec![format!("if {} {{\n", condition_code(plan, condition))];
                statements(plan, yes, &mut code, result);
                code.push("} else {\n".to_owned());
                statements(plan, no, &mut code, result);
                code.push("}\n".to_owned());
                code.concat()
            }
        });
    }
}
fn assignment(plan: &Plan, target: &Value, v: &Value) -> String {
    if target.ty == Ty::Any
        && crate::finite_domains::prepared(plan)
        && let Some(slot) = borrowed_place(plan, target)
    {
        return format!(
            "{{let destination={slot};{}}}",
            crate::any_prepared::write(plan, v, "destination", |v| value(plan, v))
        );
    }
    if matches!(target.ty, Ty::Any | Ty::Native(_)) && target.snapshot {
        let destination = crate::snapshot_slots::projection(target, |v| value(plan, v));
        return format!(
            "{{let destination={destination};{}}}",
            crate::any_prepared::write(plan, v, "destination", |v| value(plan, v))
        );
    }
    if matches!(target.kind, Kind::Output)
        && let Some(code) =
            crate::structural_publication::assignment(v, &target.ty, |v| value(plan, v))
    {
        return code;
    }
    if matches!(target.kind, Kind::Output)
        && let Some(code) = prepared::publication(plan, v, Some(&target.ty))
    {
        return format!("{{{code}}}");
    }
    let publish = crate::deltas::publish(&v.ty, "publication");
    if crate::finite_domains::prepared(plan)
        && let Some(slot) = borrowed_place(plan, target)
    {
        return prepared::replace(plan, v, &slot);
    }
    let v = condition_code(plan, v);
    if let Some(slot) = borrowed_place(plan, target) {
        return format!(
            "{{ let replacement = {v}; let slot = {slot}; _ctx.global_state().write(slot, &replacement)?; }}\n"
        );
    }
    if matches!(target.kind, Kind::Output) {
        format!("{{ let publication = {v}; {publish} }}\n")
    } else if let Kind::Cache(i) = target.kind {
        format!("self.cache{i} = {v};\n")
    } else if matches!(
        target.kind,
        Kind::MutableLocal(_) | Kind::GeneratorLocal(_) | Kind::Field(..) | Kind::Index(..)
    ) {
        format!(
            "{{ let replacement = {v}; {} = replacement; }}\n",
            mutable_place(plan, target)
        )
    } else {
        unreachable!("checked assignment")
    }
}
/// Emit the checked condition code form.
pub fn condition_code(plan: &Plan, condition: &Value) -> String {
    let code = value(plan, condition);
    if (matches!(condition.kind, Kind::Binary(..) | Kind::Query(..))
        || matches!(&condition.kind,Kind::Unary(op,_) if op!="family"))
        && code.starts_with('(')
        && code.ends_with(')')
    {
        code[1..code.len() - 1].into()
    } else {
        code
    }
}

fn reference_output_query(op: &str) -> String {
    match op {
        "valid" | "all_valid" => "(_ctx.store().bindings().output(self._output).modified_at != hgl_types::EngineTime::NEVER)".into(),
        "last_modified" => "_ctx.store().bindings().output(self._output).modified_at".into(),
        "modified" => "_ctx.store().output_modified(self._output,_ctx.evaluation_time())".into(),
        _ => unreachable!("checked reference output query"),
    }
}

/// Emit the checked query form.
pub fn query(op: &str, args: &[Value]) -> String {
    query_with(op, args, crate::observed::input)
}
fn query_with(op: &str, args: &[Value], endpoint: impl Fn(&Value) -> String) -> String {
    let values = args
        .iter()
        .map(|v| {
            if matches!(v.kind, Kind::Output) {
                if matches!(v.ty, Ty::Ref(_)) {
                    return reference_output_query(op);
                }
                return match op {
                    "valid" => "_ctx.store().output_ref(self._output).is_some()".into(),
                    "last_modified" => {
                        "_ctx.store().bindings().output(self._output.id()).modified_at".into()
                    }
                    "modified" => {
                        "_ctx.store().output_modified(self._output.id(),_ctx.evaluation_time())"
                            .into()
                    }
                    _ => unreachable!("checked output query"),
                };
            }
            let input = endpoint(v);
            if let Some(code) = crate::windows::query(&v.ty, op, &input) {
                return code;
            }
            if let Some(payload) = whole_payload(&v.ty)
                && op == "delta_value"
            {
                return format!(
                    "_ctx.store().atomic_get::<{}>({input})?",
                    global_type(payload)
                );
            }
            if crate::deltas::shaped(&v.ty) {
                return match op {
                    "delta_value" => crate::deltas::observe(
                        &v.ty
                            .clone()
                            .delta()
                            .unwrap_or_else(|_| unreachable!("checked delta")),
                        &input,
                    ),
                    "valid" => format!("_ctx.store().input_valid({input}.id())"),
                    "all_valid" => format!("_ctx.store().bindings().all_valid({input}.id())"),
                    "modified" => format!(
                        "_ctx.store().bindings().modified({input}.id(),_ctx.evaluation_time())"
                    ),
                    "last_modified" => {
                        format!("_ctx.store().bindings().last_modified({input}.id())")
                    }
                    _ => unreachable!("checked structural query"),
                };
            }
            if matches!(v.ty, Ty::Ref(_)) {
                return match op {
                    "valid" | "all_valid" => format!("_ctx.store().input_valid({input})"),
                    "modified" => {
                        format!("_ctx.store().bindings().modified({input},_ctx.evaluation_time())")
                    }
                    "last_modified" => {
                        format!("_ctx.store().bindings().last_modified({input})")
                    }
                    _ => unreachable!("unsupported structural query"),
                };
            }
            match op {
                "delta_value" => format!("_ctx.get({input})"),
                "last_modified" => format!("_ctx.last_modified({input})"),
                "activate" | "passivate" => {
                    format!("_ctx.set_active({input},{})", op == "activate")
                }
                _ => format!("_ctx.{op}({input})"),
            }
        })
        .collect::<Vec<_>>();
    if values.len() == 1 {
        values[0].clone()
    } else {
        let separator = if op == "modified" { " || " } else { " && " };
        format!("({})", values.join(separator))
    }
}

fn set_query(plan: &Plan, op: &str, args: &[Value]) -> String {
    let endpoint = if let Kind::Input(i, _) = args[0].kind {
        format!("self.input{i}.id()")
    } else {
        assert!(matches!(args[0].kind, Kind::Output), "checked set endpoint");
        "self._output.id()".to_owned()
    };
    let key = args
        .get(1)
        .map(|value| condition_code(plan, value))
        .unwrap_or_default();
    let key = if args.get(1).is_some_and(|value| value.ty == Ty::Bool) {
        format!("i64::from({key})")
    } else {
        key
    };
    crate::keyed::set_operation(op, &endpoint, &key)
}

fn byte_view(plan: &Plan, value: &Value) -> String {
    if let Kind::Query(op, args) = &value.kind
        && op == "delta_value"
    {
        return byte_view(plan, &args[0]);
    }
    let source = crate::source_slots::source(plan, value)
        .filter(|(columns, _)| *columns != "configuration_columns");
    crate::snapshot_views::byte_slice(value, source, &place(plan, value))
}

fn native_argument(plan: &Plan, v: &Value) -> String {
    if matches!(v.ty, Ty::Native(_)) {
        if let Some((arena, slot)) = crate::source_slots::source(plan, v) {
            let columns = if arena == "configuration_columns" {
                "self.configuration_columns".into()
            } else {
                format!("_ctx.store().{arena}()")
            };
            return format!("{columns}.native({slot})");
        }
        return format!("({}).view()", condition_code(plan, v));
    }
    if v.ty != Ty::Str {
        return condition_code(plan, v);
    }
    if v.snapshot {
        let slot = crate::snapshot_slots::projection(v, |v| value(plan, v));
        return format!(
            "_ctx.store().global_values().scalar::<String>(({slot}).fields()).as_str()"
        );
    }
    if hgl_semantics::tuple_values::endpoint(v) && matches!(v.kind, Kind::Field(..)) {
        return crate::snapshot_slots::native_text(v);
    }
    if let Some(slot) = borrowed_place(plan, v) {
        return format!(
            "_ctx.store().global_values().scalar::<String>(({slot}).fields()).as_str()"
        );
    }
    if matches!(v.kind, Kind::IterationInput(_) | Kind::Input(..)) {
        return crate::snapshot_slots::native_text(v);
    }
    if let Kind::Literal(Literal::Str(s)) = &v.kind {
        return format!("{s:?}");
    }
    if let Kind::Cache(i) = v.kind {
        return format!("self.cache{i}.as_str()");
    }
    if matches!(
        v.kind,
        Kind::Local(_)
            | Kind::MutableLocal(_)
            | Kind::Field(..)
            | Kind::Index(..)
            | Kind::GeneratorLocal(_)
    ) {
        return format!("({}).as_str()", place(plan, v));
    }
    if matches!(v.kind, Kind::Output) {
        return "_ctx.store().output_ref(self._output).expect(\"valid output\").as_str()".into();
    }
    format!("&({})", value(plan, v))
}

fn capability_call(plan: &Plan, op: &str, args: &[Value]) -> String {
    let args = args.iter().map(|v| value(plan, v)).collect::<Vec<_>>();
    match op {
        "clock.evaluation_time" => "_ctx.evaluation_time()".into(),
        "clock.next_cycle_evaluation_time" => "_ctx.next_cycle_evaluation_time()".into(),
        "alarm.schedule" => format!("_ctx.alarm_in({})?", args[0]),
        "alarm.schedule_at" => format!(
            "{{ let time={}; let delay=time.micros().checked_sub(_ctx.evaluation_time().micros()).ok_or_else(||hgl_kernel::NodeError::new(\"alarm: time difference overflow\"))?; _ctx.alarm_in(hgl_types::EngineDelta::from_micros(delay))?; }}",
            args[0]
        ),
        _ => unreachable!("checked capability operation or property"),
    }
}

pub(crate) fn borrowed_place(plan: &Plan, value: &Value) -> Option<String> {
    if let Kind::GlobalGet(id) = value.kind {
        return Some(format!("_ctx.global_state().borrow(self.global{id})?"));
    }
    if let Kind::BorrowedLocal(id, _, _) = value.kind {
        return Some(format!("local{id}"));
    }
    if let Kind::Index(parent, index) = &value.kind {
        return borrowed_place(plan, parent).map(|parent| format!("{{ let parent = {parent}; let index = {}; hgl_store::list::global_index(_ctx.store().global_values(), parent, index)? }}", self::value(plan, index)));
    }
    if let Kind::Field(parent, field) = &value.kind {
        return borrowed_place(plan, parent).map(|source| {
            if let Ty::Family(family) = &parent.ty {
                return crate::collections::family_field(family, *field, &source, true);
            }
            format!("({source}).fields().{field}")
        });
    }
    None
}

fn mutable_place(plan: &Plan, v: &Value) -> String {
    if let Kind::GeneratorLocal(id) = v.kind {
        return format!(
            "(*self.generator_local{id}.as_mut().expect(\"initialized generator local\"))"
        );
    }
    if let Kind::Index(parent, index) = &v.kind {
        return format!(
            "(*hgl_store::list_index_mut(&mut ({}), {})?)",
            mutable_place(plan, parent),
            value(plan, index)
        );
    }
    if let Kind::Field(parent, field) = &v.kind {
        return format!("({}).{field}", mutable_place(plan, parent));
    }
    place(plan, v)
}
fn list_value(plan: &Plan, ty: &Ty, values: &[Value]) -> String {
    let Ty::List(element, _) = ty else {
        unreachable!("checked list")
    };
    let mut code = vec![format!(
        "{{ let mut items: Vec<{}> = Vec::new();",
        owned_type(element)
    )];
    for item in values {
        code.push(format!(
            "hgl_store::list_push::<{}>(&mut items, &({}))?;",
            global_type(element),
            value(plan, item)
        ));
    }
    code.push("items }".into());
    code.concat()
}
fn push(plan: &Plan, parent: &Value, item: &Value) -> String {
    if crate::finite_domains::prepared(plan)
        && let Some(slot) = borrowed_place(plan, parent)
    {
        return prepared::append(plan, item, &slot);
    }
    let item = value(plan, item);
    if let Some(slot) = borrowed_place(plan, parent) {
        return format!(
            "{{ let slot = {slot}; let item = {item}; _ctx.global_state().list_push(slot, &item)?; }}"
        );
    }
    let Ty::List(element, _) = &parent.ty else {
        unreachable!("checked push")
    };
    let mut setup = Vec::new();
    let target = prepare_place(plan, parent, &mut setup);
    format!(
        "{{ {} let item = {item}; hgl_store::list_push::<{}>(&mut ({target}), &item)?; }}",
        setup.concat(),
        global_type(element)
    )
}
fn direct_call(plan: &Plan, result: &Ty, args: &[Value], body: &[Statement]) -> String {
    let mut code = vec!["{ ".into()];
    for (i, arg) in args.iter().enumerate() {
        let annotation = matches!(arg.ty, Ty::Family(_))
            .then(|| format!(":{}", owned_type(&arg.ty)))
            .unwrap_or_default();
        code.push(format!(
            "let argument{i}{annotation} = {};",
            crate::value_calls::argument(arg, i, body, |v| value(plan, v))
        ));
    }
    code.push(format!(
        "(|| -> Result<{}, Box<hgl_types::NodeError>> {{",
        owned_type(result)
    ));
    for i in 0..args.len() {
        code.push(format!("let local{i} = argument{i};"));
    }
    statements(plan, body, &mut code, None);
    if *result == Ty::Void {
        code.push("Ok(())".into());
    }
    code.push("})()? }".into());
    code.concat()
}

fn prepare_place(plan: &Plan, target: &Value, setup: &mut Vec<String>) -> String {
    if let Kind::Field(parent, field) = &target.kind {
        return format!("({}).{field}", prepare_place(plan, parent, setup));
    }
    if let Kind::Index(parent, index) = &target.kind {
        let parent = prepare_place(plan, parent, setup);
        let id = setup.len();
        setup.push(format!(
            "let index{id} = {}; hgl_store::list_index(&({parent}), index{id})?;",
            value(plan, index)
        ));
        return format!("(*hgl_store::list_index_mut(&mut ({parent}), index{id})?)");
    }
    mutable_place(plan, target)
}

fn length(plan: &Plan, parent: &Value) -> String {
    if parent.ty == Ty::Bytes {
        return format!("hgl_store::list_len({})?", byte_view(plan, parent));
    }
    if parent.snapshot {
        return crate::snapshot_views::length(parent, |v| value(plan, v));
    }
    if let Some(slot) = atomic_place(plan, parent) {
        return format!(
            "hgl_store::list_len(_ctx.store().atomic_values().list(({slot}).fields()))?"
        );
    }
    if let Some(slot) = borrowed_place(plan, parent) {
        format!("{{ let slot = {slot}; _ctx.global_state().list_len(slot)? }}")
    } else {
        format!("hgl_store::list_len(&({}))?", place(plan, parent))
    }
}

fn observation(v: &Value) -> Option<String> {
    if !matches!(v.ty, Ty::Delta(_)) {
        return None;
    }
    if let Kind::ObservedLocal(id) = v.kind {
        return Some(format!("local{id}"));
    }
    if let Kind::Query(op, args) = &v.kind
        && op == "delta_value"
    {
        let Kind::Input(id, _) = args[0].kind else {
            unreachable!("checked observed input")
        };
        return Some(format!("self.input{id}"));
    }
    None
}

pub(crate) fn atomic_place(plan: &Plan, value: &Value) -> Option<String> {
    if let Kind::Input(id, _) = value.kind
        && let Some(payload) = whole_payload(&value.ty)
    {
        return Some(format!(
            "_ctx.store().atomic_borrow::<{}>(self.input{id})?",
            global_type(payload)
        ));
    }
    if matches!(value.ty, Ty::Delta(_)) {
        return None;
    }
    if let Kind::ObservedLocal(id) = value.kind {
        return Some(format!("local{id}"));
    }
    if let Kind::Query(name, args) = &value.kind
        && name == "delta_value"
    {
        let input = args.first()?;
        let payload = whole_payload(&input.ty)?;
        let Kind::Input(id, _) = input.kind else {
            return None;
        };
        return Some(format!(
            "_ctx.store().atomic_borrow::<{}>(self.input{id})?",
            global_type(payload)
        ));
    }
    if let Kind::Field(parent, index) = &value.kind {
        return atomic_place(plan, parent).map(|slot| format!("({slot}).fields().{index}"));
    }
    if let Kind::Index(parent, index) = &value.kind {
        return atomic_place(plan, parent).map(|slot| format!("hgl_store::ValueSlot::<{}>::bind(&mut hgl_store::list_index(_ctx.store().atomic_values().list(({slot}).fields()), {})?.as_slice())", global_type(&value.ty), self::value(plan, index)));
    }
    None
}

fn retained(source: &str, ty: &Ty) -> String {
    crate::collections::retained(source, ty, global_type)
}

fn native_call(plan: &Plan, binding: &hgl_semantics::ir::Native, args: &[Value]) -> String {
    let call = format!(
        "{}::{}({}){}",
        binding
            .provider
            .as_ref()
            .map_or("<crate::Provider as Native>".into(), |p| {
                crate::native_image::image_module(&p.provider)
            }),
        binding.method,
        args.iter()
            .map(|v| native_argument(plan, v))
            .collect::<Vec<_>>()
            .join(", "),
        if binding.throws { "?" } else { "" }
    );
    if binding.provider.is_some() && !crate::execution_proof::prepared(plan) {
        if matches!(binding.result, Ty::Native(_)) {
            return format!("({call}).retain()?");
        }
        if binding.result == Ty::Str {
            return format!("({call}).to_owned()");
        }
    }
    call
}

fn map_lookup(plan: &Plan, op: &str, args: &[Value]) -> String {
    let Ty::Map(key, child) = &args[0].ty else {
        unreachable!("checked dictionary lookup")
    };
    let (any_borrow, key_value) = lookup_key(plan, &args[1]);
    let default = args
        .get(2)
        .map(|v| format!("let fallback={};", value(plan, v)))
        .unwrap_or_default();
    let hash = if let Ty::Native(scalar) = key.as_ref() {
        format!(
            "<{} as hgl_store::native_values::NativeType>::hash(key.data())?",
            crate::native_values::marker(scalar)
        )
    } else {
        crate::collections::key_hash(key, "key")
    };
    let mut code = String::from("{");
    let found = if let Some((arena, slot)) = crate::source_slots::source(plan, &args[0]) {
        let columns = if arena == "configuration_columns" {
            "&self.configuration_columns".into()
        } else {
            format!("_ctx.store().{arena}()")
        };
        crate::append(
            &mut code,
            format_args!(
                "let parent={slot};let key={key_value};{default}let columns={columns};let hash={hash};let position=columns.map_index(parent.fields()).find(hash,|position|{{let entry=hgl_store::ValueSlot::<{}>::bind(&mut columns.list(parent.fields())[position].as_slice());Ok({})}})?;let found=position.map(|position|{{let entry=hgl_store::ValueSlot::<{}>::bind(&mut columns.list(parent.fields())[position].as_slice());entry.fields().1.read(columns)}}).transpose()?;",
                global_type(&Ty::Tuple(vec![*key.clone(), *child.clone()])),
                if any_borrow {
                    "columns.any(entry.fields().0.fields()).borrowed().equal(&key)?".into()
                } else if matches!(key.as_ref(), Ty::Native(_)) {
                    "columns.native(entry.fields().0).equal(key)?".into()
                } else {
                    crate::collections::slot_key_equal(key, "entry.fields().0", "key", "columns")
                },
                global_type(&Ty::Tuple(vec![*key.clone(), *child.clone()]))
            ),
        );
        "found".into()
    } else {
        let parent = if matches!(
            args[0].kind,
            Kind::Local(_) | Kind::MutableLocal(_) | Kind::GeneratorLocal(_)
        ) {
            place(plan, &args[0])
        } else {
            value(plan, &args[0])
        };
        crate::append(
            &mut code,
            format_args!(
                "let parent=&({parent});let key={key_value};{default}let hash={hash};let position=parent.index().find(hash,|position|{{let entry=&parent[position];Ok({})}})?;let found=position.map(|position|{{let entry=&parent[position];Ok::<_,Box<hgl_types::NodeError>>({})}}).transpose()?;",
                if any_borrow {
                    "entry.0.borrowed().equal(&key)?".into()
                } else if matches!(key.as_ref(), Ty::Native(_)) {
                    "entry.0.view().equal(key)?".into()
                } else {
                    crate::collections::equal(key, "&entry.0", "&key", global_type)
                },
                retained("entry.1", child)
            ),
        );
        String::from("found")
    };
    let result = if args.len() == 3 {
        format!("match {found} {{Some(value)=>value,None=>fallback}}")
    } else if op == "\0lookup_at" {
        format!(
            "{found}.ok_or_else(||hgl_types::NodeError::coded(\"dictionary key is absent\",\"value.missing_key\"))?"
        )
    } else {
        found
    };
    code.push_str(&result);
    code.push('}');
    code
}

fn lookup_key(plan: &Plan, key: &Value) -> (bool, String) {
    if key.ty == Ty::Any
        && let Some((arena, slot)) = crate::source_slots::source(plan, key)
    {
        let columns = if arena == "configuration_columns" {
            "self.configuration_columns".into()
        } else {
            format!("_ctx.store().{arena}()")
        };
        return (
            true,
            format!("{{let slot={slot};{columns}.any(slot.fields()).borrowed()}}"),
        );
    }
    let operand = if key.ty == Ty::Str || matches!(key.ty, Ty::Native(_)) {
        native_argument(plan, key)
    } else {
        value(plan, key)
    };
    (false, operand)
}
