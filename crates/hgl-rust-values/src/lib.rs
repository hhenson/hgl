//! Checked hook expression and statement emission.
use hgl_rust_ir::{Kind, Plan, Statement, Value};
pub use hgl_rust_layouts::{
    global_markers, global_schema, global_type, owned_type, rust_type, scalar_type,
};
use hgl_source::{Literal, Ty};

/// Emit the checked literal form.
pub fn literal(value: &Literal) -> String {
    match value {
        Literal::Int(i) => format!("{i}_i64"),
        Literal::Duration(i) => format!("hgl_types::EngineDelta::from_micros({i})"),
        Literal::Date(i) => format!("hgl_types::Date({i})"),
        Literal::Time(i) => format!("hgl_types::Time({i})"),
        Literal::DateTime(i) => format!("hgl_types::EngineTime::from_micros({i})"),
        Literal::Bool(b) => b.to_string(),
        Literal::Float(f) => format!("{f:?}_f64"),
        Literal::Str(s) => format!("{s:?}.to_owned()"),
    }
}
fn value(plan: &Plan, v: &Value) -> String {
    match &v.kind {
        Kind::WiringFailure(message) => {
            format!("return Err(hgl_types::NodeError::new({message:?}))")
        }
        Kind::List(values) => list_value(plan, &v.ty, values),
        Kind::Length(parent) => length(plan, parent),
        Kind::Push(parent, item) => push(plan, parent, item),
        Kind::ValueCall(args, body) => direct_call(plan, &v.ty, args, body),
        Kind::Configuration(id) => retained(&format!("self.configuration{id}"), &v.ty),
        Kind::Construct(fields) => construct(plan, fields),
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
        Kind::Input(i, _) => {
            if matches!(v.ty, Ty::Ref(_)) {
                format!("_ctx.store().bindings().input_reference(self.input{i})")
            } else {
                format!("_ctx.get(self.input{i})")
            }
        }
        Kind::Cache(i) => {
            if v.ty == Ty::Str {
                format!("self.cache{i}.clone()")
            } else {
                format!("self.cache{i}")
            }
        }
        Kind::GlobalGet(i) => format!("_ctx.global_state().get(self.global{i})?"),
        Kind::GlobalSet(i, v) => format!(
            "{{ let value = {}; _ctx.global_state().set(self.global{i}, &value)?; }}",
            value(plan, v)
        ),
        Kind::GeneratorLocal(_) => retained(&place(plan, v), &v.ty),
        Kind::Local(i) | Kind::MutableLocal(i) => retained(&format!("local{i}"), &v.ty),
        Kind::Native(i, args) => format!(
            "<crate::Provider as Native>::{}({}){}",
            plan.natives[*i].method,
            args.iter()
                .map(|v| native_argument(plan, v))
                .collect::<Vec<_>>()
                .join(", "),
            if plan.natives[*i].throws { "?" } else { "" }
        ),
        Kind::Binary(op, a, b) => binary(plan, &v.ty, op, a, b),
        Kind::Unary(op, v) => unary(plan, op, v),
        Kind::Query(op, args) => {
            if op.contains('.') {
                capability_call(plan, op, args)
            } else if op.starts_with("set_") {
                set_query(plan, op, args)
            } else {
                query(op, args)
            }
        }
        Kind::Output => "_ctx.output_value(self._output).expect(\"valid output\")".into(),
        Kind::Wire(_) | Kind::Void | Kind::Capability => unreachable!("checked runtime value"),
    }
}
fn unary(plan: &Plan, op: &str, operand: &Value) -> String {
    let value = value(plan, operand);
    if op == "-" && operand.ty == Ty::I64 {
        format!("(({value}).wrapping_neg())")
    } else if op == "-" && operand.ty == Ty::Duration {
        format!(
            "hgl_types::EngineDelta::from_micros(({value}).micros().checked_neg().ok_or_else(|| hgl_types::NodeError::new(\"time arithmetic overflow\"))?)"
        )
    } else if op == "float" {
        format!("({value} as f64)")
    } else {
        format!("({op}{value})")
    }
}
fn construct(plan: &Plan, fields: &[(usize, Value)]) -> String {
    if fields.is_empty() {
        return "()".into();
    }
    let mut code = vec!["{ ".to_owned()];
    for (index, argument) in fields {
        code.push(format!("let field{index} = {}; ", value(plan, argument)));
    }
    let fields = (0..fields.len())
        .map(|index| format!("field{index}"))
        .collect::<Vec<_>>()
        .join(",");
    code.push(format!("({fields},) }}"));
    code.concat()
}
fn place(plan: &Plan, v: &Value) -> String {
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
        return format!("({}).{index}", place(plan, parent));
    }
    value(plan, v)
}
fn retained(source: &str, ty: &Ty) -> String {
    if matches!(ty, Ty::Nullable(_)) {
        return format!("({source}).as_ref().map(hgl_store::Scalar::try_clone).transpose()?");
    }
    if let Ty::List(..) = ty {
        return format!(
            "<{} as hgl_store::GlobalValue>::retain(&({source}))?",
            global_type(ty)
        );
    }
    if let Ty::Struct(_, fields) = ty {
        let fields = fields
            .iter()
            .enumerate()
            .map(|(i, (_, ty))| retained(&format!("source.{i}"), ty))
            .collect::<Vec<_>>()
            .join(",");
        return if fields.is_empty() {
            format!("{{ let _ = &({source}); () }}")
        } else {
            format!("{{ let source = &({source}); ({fields},) }}")
        };
    }
    if *ty == Ty::Str {
        format!("hgl_store::Scalar::try_clone(&({source}))?")
    } else {
        format!("({source})")
    }
}

fn binary(plan: &Plan, result: &Ty, op: &str, a: &Value, b: &Value) -> String {
    let a = if a.ty == Ty::Str {
        native_argument(plan, a)
    } else {
        value(plan, a)
    };
    let b = if b.ty == Ty::Str {
        native_argument(plan, b)
    } else {
        value(plan, b)
    };
    if matches!(result, Ty::DateTime | Ty::Duration) && matches!(op, "+" | "-") {
        let operation = if op == "+" {
            "checked_add"
        } else {
            "checked_sub"
        };
        return format!(
            "{{ let lhs = ({a}).micros(); let rhs = ({b}).micros(); {}::from_micros(lhs.{operation}(rhs).ok_or_else(|| hgl_types::NodeError::new(\"time arithmetic overflow\"))?) }}",
            rust_type(result)
        );
    }
    if *result == Ty::I64 && matches!(op, "+" | "-" | "*" | "%") {
        integer_binary(op, &a, &b)
    } else if op == "/" {
        format!(
            "{{ let lhs = ({a}) as f64; let rhs = ({b}) as f64; if rhs == 0.0 {{ return Err(hgl_kernel::NodeError::new(\"division by zero\")); }} lhs / rhs }}"
        )
    } else if op == "+" && *result == Ty::Str {
        format!("format!(\"{{}}{{}}\", {a}, {b})")
    } else {
        format!("({a} {op} {b})")
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
fn integer_binary(op: &str, a: &str, b: &str) -> String {
    match op {
        "+" => format!("(({a}).wrapping_add({b}))"),
        "-" => format!("(({a}).wrapping_sub({b}))"),
        "*" => format!("(({a}).wrapping_mul({b}))"),
        "%" => format!(
            "{{ let lhs = {a}; let rhs = {b}; if rhs == -1 {{ 0_i64 }} else {{ let rem = lhs.checked_rem(rhs).ok_or_else(|| hgl_kernel::NodeError::new(\"modulo by zero\"))?; if rem != 0 && (rem < 0) != (rhs < 0) {{ rem + rhs }} else {{ rem }} }} }}"
        ),
        _ => unreachable!("checked integer operation"),
    }
}

/// Emit the checked statements form.
pub fn statements(plan: &Plan, body: &[Statement], out: &mut Vec<String>) {
    for statement in body {
        out.push(match statement {
            Statement::TimedYield(..) => unreachable!("generator yields use resume lowering"),
            Statement::While(condition, body) => {
                let mut code = vec![format!("while {} {{\n", condition_code(plan, condition))];
                statements(plan, body, &mut code);
                code.push("}\n".into());
                code.concat()
            }
            Statement::Yield(v) => format!("return Ok({});\n", value(plan, v)),
            Statement::Exit => "return Ok(());\n".into(),
            Statement::Let(i, v) => format!("let local{i} = {};\n", condition_code(plan, v)),
            Statement::Borrow(i, v, _) => {
                let source = if let Kind::GlobalGet(entry) = v.kind {
                    format!("_ctx.global_state().borrow(self.global{entry})?")
                } else { borrowed_place(plan, v).unwrap_or_else(|| unreachable!("checked borrowed initializer")) };
                format!("let local{i} = {source};\n")
            },
            Statement::Var(i, v) => format!("let mut local{i} = {};\n", condition_code(plan, v)),
            Statement::Return(v) => {
                let publish = if matches!(v.ty, Ty::Ref(_)) { "_ctx.set_reference(self._output, publication)?;" } else { "_ctx.set(self._output, publication);" };
                format!("let publication = {};\n{publish}\nreturn Ok(());\n", condition_code(plan, v))
            },
            Statement::Call(v) => format!("{};\n", condition_code(plan, v)),
            Statement::For(id,collection,body)=> {
                let Kind::Input(input, _)=collection.kind else {unreachable!("checked collection")};
                let Ty::Set(element)=&collection.ty else {unreachable!("checked collection")};
                let mut code=vec![format!("let mut index{id}=0;\nwhile let Some(key{id})=_ctx.store().bindings().changed_keys(self.input{input}).get(index{id}).copied() {{\nindex{id}+=1;\nif _ctx.store().bindings().input(self.input{input}).members.initial.get(&key{id})==Some(&false) && _ctx.store().bindings().child_input(self.input{input},key{id}).is_some() {{\nlet local{id}={};\n",if **element==Ty::Bool {format!("key{id}!=0")} else {format!("key{id}")})];
                statements(plan,body,&mut code);code.push("}\n}\n".into());code.concat()
            }
            Statement::Assign(target,v) => assignment(plan, target, v),
            Statement::If(condition, yes, no) => {
                let mut code = vec![format!("if {} {{\n", condition_code(plan, condition))];
                statements(plan, yes, &mut code);
                code.push("} else {\n".to_owned());
                statements(plan, no, &mut code);
                code.push("}\n".to_owned());
                code.concat()
            }
        });
    }
}
fn assignment(plan: &Plan, target: &Value, v: &Value) -> String {
    let v = condition_code(plan, v);
    if let Some(slot) = borrowed_place(plan, target) {
        return format!(
            "{{ let replacement = {v}; let slot = {slot}; _ctx.global_state().write(slot, &replacement)?; }}\n"
        );
    }
    if matches!(target.kind, Kind::Output) {
        format!("{{ let publication = {v}; _ctx.set(self._output, publication); }}\n")
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
    if matches!(
        condition.kind,
        Kind::Binary(..) | Kind::Unary(..) | Kind::Query(..)
    ) && code.starts_with('(')
        && code.ends_with(')')
    {
        code[1..code.len() - 1].into()
    } else {
        code
    }
}

/// Emit the checked query form.
pub fn query(op: &str, args: &[Value]) -> String {
    let values = args
        .iter()
        .map(|v| {
            if matches!(v.kind, Kind::Output) {
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
            let Kind::Input(i, _) = v.kind else {
                unreachable!("checked endpoint query")
            };
            if matches!(v.ty, Ty::Ref(_) | Ty::Set(_)) {
                return match op {
                    "valid" => format!("_ctx.store().input_valid(self.input{i})"),
                    "modified" => format!(
                        "_ctx.store().bindings().modified(self.input{i},_ctx.evaluation_time())"
                    ),
                    "last_modified" => {
                        format!("_ctx.store().bindings().last_modified(self.input{i})")
                    }
                    _ => unreachable!("unsupported structural query"),
                };
            }
            match op {
                "delta_value" => format!("_ctx.get(self.input{i})"),
                "last_modified" => format!("_ctx.last_modified(self.input{i})"),
                "activate" | "passivate" => {
                    format!("_ctx.set_active(self.input{i},{})", op == "activate")
                }
                _ => format!("_ctx.{op}(self.input{i})"),
            }
        })
        .collect::<Vec<_>>();
    if values.len() == 1 {
        values[0].clone()
    } else {
        format!(
            "({})",
            values.join(if op == "modified" { " || " } else { " && " })
        )
    }
}

fn set_query(plan: &Plan, op: &str, args: &[Value]) -> String {
    let endpoint = if let Kind::Input(i, _) = args[0].kind {
        format!("self.input{i}")
    } else {
        assert!(matches!(args[0].kind, Kind::Output), "checked set endpoint");
        "self._output".to_owned()
    };
    if op == "set_bound" {
        return format!("_ctx.store().bindings().has_peer({endpoint})");
    }
    if op == "set_len" {
        return format!(
            "i64::try_from(_ctx.store().bindings().input({endpoint}).members.live.len()).map_err(|e|hgl_kernel::NodeError::new(e.to_string()))?"
        );
    }
    let key = condition_code(plan, &args[1]);
    let key = if args[1].ty == Ty::Bool {
        format!("i64::from({key})")
    } else {
        key
    };
    match op {
        "set_contains" => {
            format!("_ctx.store().bindings().child_input({endpoint},{key}).is_some()")
        }
        "set_discard" => format!("_ctx.remove_shaped({endpoint},{key})"),
        "set_upsert" => format!(
            "{{let key={key}; if _ctx.store().bindings().child_output({endpoint},key).is_none() {{let child=_ctx.get_or_create_shaped({endpoint},key);let child=_ctx.store().scalar_output::<bool>(child).map_err(|e|hgl_kernel::NodeError::new(format!(\"{{e:?}}\")))?;_ctx.set(child,true);}}}}"
        ),
        _ => unreachable!("checked set operation"),
    }
}

fn native_argument(plan: &Plan, v: &Value) -> String {
    if v.ty != Ty::Str {
        return condition_code(plan, v);
    }
    if borrowed_place(plan, v).is_some() {
        return format!("&({})", value(plan, v));
    }
    match &v.kind {
        Kind::Input(i, _) => {
            format!("_ctx.store().get_ref(self.input{i}).as_str()")
        }
        Kind::Literal(Literal::Str(s)) => format!("{s:?}"),
        Kind::Cache(i) => format!("self.cache{i}.as_str()"),
        Kind::Local(i) | Kind::MutableLocal(i) => format!("local{i}.as_str()"),
        Kind::Field(..) | Kind::Index(..) | Kind::GeneratorLocal(_) => {
            format!("({}).as_str()", place(plan, v))
        }
        Kind::Output => {
            "_ctx.store().output_ref(self._output).expect(\"valid output\").as_str()".into()
        }
        Kind::WiringFailure(_)
        | Kind::List(_)
        | Kind::Length(_)
        | Kind::Push(..)
        | Kind::ValueCall(..)
        | Kind::Configuration(_)
        | Kind::Construct(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalGet(_)
        | Kind::GlobalSet(..)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::Literal(_)
        | Kind::Wire(_)
        | Kind::Native(..)
        | Kind::Binary(..)
        | Kind::Unary(..)
        | Kind::Query(..)
        | Kind::Capability
        | Kind::Void => format!("&({})", value(plan, v)),
    }
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

fn borrowed_place(plan: &Plan, value: &Value) -> Option<String> {
    if let Kind::BorrowedLocal(id, _, _) = value.kind {
        return Some(format!("local{id}"));
    }
    if let Kind::Index(parent, index) = &value.kind {
        return borrowed_place(plan, parent).map(|parent| format!("{{ let parent = {parent}; let index = {}; _ctx.global_state().list_index(parent, index)? }}", self::value(plan, index)));
    }
    if let Kind::Field(parent, field) = &value.kind {
        return borrowed_place(plan, parent).map(|parent| format!("({parent}).fields().{field}"));
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
        code.push(format!("let argument{i} = {};", value(plan, arg)));
    }
    code.push(format!(
        "(|| -> Result<{}, Box<hgl_types::NodeError>> {{",
        owned_type(result)
    ));
    for i in 0..args.len() {
        code.push(format!("let local{i} = argument{i};"));
    }
    statements(plan, body, &mut code);
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
    if let Some(slot) = borrowed_place(plan, parent) {
        format!("{{ let slot = {slot}; _ctx.global_state().list_len(slot)? }}")
    } else {
        format!("hgl_store::list_len(&({}))?", place(plan, parent))
    }
}
