//! Cold typed conversion and checked lexical test emission.
use hgl_harness_ir::{Argument, Step, Test};
use hgl_rust_ir::{DeltaEntry, Kind, Statement, Value};
use hgl_source::{Literal, TemporalLiteral, Ty};
fn list<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
    format!(
        "vec![{}]",
        items.iter().map(f).collect::<Vec<_>>().join(",")
    )
}
/// Emit exact checked source type metadata.
pub fn ty(t: &Ty) -> String {
    let inner = match t {
        Ty::Atomic(t) => format!("Atomic(Box::new({}))", ty(t)),
        Ty::Ref(t) => format!("Ref(Box::new({}))", ty(t)),
        Ty::Nullable(t) => format!("Nullable(Box::new({}))", ty(t)),
        Ty::Set(t) => format!("Set(Box::new({}))", ty(t)),
        Ty::Delta(t) => format!("Delta(Box::new({}))", ty(t)),
        Ty::Map(k, v) => format!("Map(Box::new({}),Box::new({}))", ty(k), ty(v)),
        Ty::List(t, n) => format!("List(Box::new({}),{n:?})", ty(t)),
        Ty::Tuple(ts) => format!("Tuple({})", list(ts, ty)),
        Ty::Struct(n, fields) => format!(
            "Struct(hgl_source::Nominal {{origin:{:?}.into(),arguments:{}}},{})",
            n.origin,
            list(&n.arguments, ty),
            list(fields, |(name, t)| format!("({name:?}.into(),{})", ty(t)))
        ),
        Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::Duration
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Void => format!("{t:?}"),
    };
    format!("hgl_source::Ty::{inner}")
}
fn literal(l: &Literal) -> String {
    let inner = match l {
        Literal::Str(s) => format!("Str({s:?}.into())"),
        Literal::Float(v) => format!("Float(f64::from_bits({}))", v.to_bits()),
        Literal::TimeZone(zone) => format!(
            "TimeZone(hgl_types::ZoneId::from_validated_name({:?}.into()))",
            zone.as_str()
        ),
        Literal::ZonedTime(zoned) => format!(
            "ZonedTime(hgl_types::ZonedTime::from_validated_parts(hgl_types::Time({}), hgl_types::ZoneId::from_validated_name({:?}.into())))",
            zoned.time().0,
            zoned.zone().as_str()
        ),
        Literal::ZonedDateTime(zoned) => format!(
            "ZonedDateTime(hgl_types::ZonedDateTime::from_validated_parts(hgl_types::EngineTime::from_micros({}),hgl_types::ZoneId::from_validated_name({:?}.into()),{}))",
            zoned.instant().micros(),
            zoned.zone().as_str(),
            zoned.offset_seconds()
        ),
        Literal::Int(_)
        | Literal::Bool(_)
        | Literal::Duration(_)
        | Literal::Date(_)
        | Literal::Time(_)
        | Literal::DateTime(_)
        | Literal::CivilDateTime(_) => format!("{l:?}"),
    };
    format!("hgl_source::Literal::{inner}")
}
fn recipe(r: &TemporalLiteral) -> String {
    match r {
        TemporalLiteral::TimeZone(s) => {
            format!("hgl_source::TemporalLiteral::TimeZone({s:?}.into())")
        }
        TemporalLiteral::ZonedTime { time_micros, zone } => format!(
            "hgl_source::TemporalLiteral::ZonedTime {{time_micros:{time_micros},zone:{zone:?}.into()}}"
        ),
        TemporalLiteral::ZonedDateTime {
            instant_micros,
            zone,
            offset_seconds,
        } => format!(
            "hgl_source::TemporalLiteral::ZonedDateTime {{instant_micros:{instant_micros},zone:{zone:?}.into(),offset_seconds:{offset_seconds}}}"
        ),
    }
}
fn part(p: &DeltaEntry) -> String {
    let p = match p {
        DeltaEntry::Add(l) => format!("Add({})", literal(l)),
        DeltaEntry::Remove(l) => format!("Remove({})", literal(l)),
        DeltaEntry::Child(i, v) => format!("Child({i},{})", value(v)),
    };
    format!("hgl_rust_ir::DeltaEntry::{p}")
}
fn value(v: &Value) -> String {
    let kind = match &v.kind {
        Kind::Literal(l) => format!("Literal({})", literal(l)),
        Kind::TemporalLiteral(r) => format!("TemporalLiteral({})", recipe(r)),
        Kind::Delta(ps) => format!("Delta({})", list(ps, part)),
        Kind::List(vs) => format!("List({})", list(vs, value)),
        Kind::Construct(fs) => format!(
            "Construct({})",
            list(fs, |(i, v)| format!("({i},{})", value(v)))
        ),
        Kind::ValueCall(args, body) => {
            format!("ValueCall({},{})", list(args, value), list(body, statement))
        }
        Kind::Field(v, i) => format!("Field(Box::new({}),{i})", value(v)),
        Kind::Index(a, b) => format!("Index(Box::new({}),Box::new({}))", value(a), value(b)),
        Kind::Push(a, b) => format!("Push(Box::new({}),Box::new({}))", value(a), value(b)),
        Kind::Length(v) => format!("Length(Box::new({}))", value(v)),
        Kind::Unary(op, v) => format!("Unary({op:?}.into(),Box::new({}))", value(v)),
        Kind::Binary(op, a, b) => format!(
            "Binary({op:?}.into(),Box::new({}),Box::new({}))",
            value(a),
            value(b)
        ),
        Kind::Local(i) => format!("Local({i})"),
        Kind::MutableLocal(i) => format!("MutableLocal({i})"),
        Kind::Void => "Void".into(),
        Kind::WiringFailure(s) => format!("WiringFailure({s:?}.into())"),
        Kind::Prepared(_)
        | Kind::ObservedLocal(_)
        | Kind::Configuration(_)
        | Kind::GlobalGet(_)
        | Kind::BorrowedLocal(..)
        | Kind::GlobalSet(..)
        | Kind::IsPresent(_)
        | Kind::Present(_)
        | Kind::GeneratorLocal(_)
        | Kind::Wire(_)
        | Kind::Input(..)
        | Kind::Cache(_)
        | Kind::Native(..)
        | Kind::Query(..)
        | Kind::Output
        | Kind::Capability => unreachable!("checked ordinary harness expression"),
    };
    format!(
        "hgl_rust_ir::Value::new({},hgl_rust_ir::Kind::{kind})",
        ty(&v.ty)
    )
}
fn statement(s: &Statement) -> String {
    let s = match s {
        Statement::Let(i, v) => format!("Let({i},{})", value(v)),
        Statement::Var(i, v) => format!("Var({i},{})", value(v)),
        Statement::Assign(a, b) => format!("Assign({},{})", value(a), value(b)),
        Statement::Call(v) => format!("Call({})", value(v)),
        Statement::Yield(v) => format!("Yield({})", value(v)),
        Statement::Exit => "Exit".into(),
        Statement::If(v, a, b) => format!(
            "If({},{},{})",
            value(v),
            list(a, statement),
            list(b, statement)
        ),
        Statement::Borrow(..)
        | Statement::Return(_)
        | Statement::For(..)
        | Statement::While(..)
        | Statement::TimedYield(..) => unreachable!("checked ordinary harness statement"),
    };
    format!("hgl_rust_ir::Statement::{s}")
}
fn slots(vs: &[Option<Value>]) -> String {
    list(vs, |v| {
        v.as_ref()
            .map_or_else(|| "None".into(), |v| format!("Some({})", value(v)))
    })
}
fn argument(a: &Argument) -> String {
    let a = match a {
        Argument::Constant { binding, value: v } => {
            format!("Constant {{binding:{binding},value:{}}}", value(v))
        }
        Argument::Dense {
            parameter,
            binding,
            shape,
            entry_type,
            slots: vs,
        } => format!(
            "Dense {{parameter:{parameter:?}.into(),binding:{binding},shape:{},entry_type:{},slots:{}}}",
            ty(shape),
            ty(entry_type),
            slots(vs)
        ),
    };
    format!("hgl_harness_ir::Argument::{a}")
}
/// Emit checked lexical test data without evaluating any expression.
pub fn test(test: &Test) -> String {
    let steps = list(&test.steps, |s| {
        let s = match s {
            Step::Ordinary(s) => format!("Ordinary({})", statement(s)),
            Step::Assert(v) => format!("Assert({})", value(v)),
            Step::Eval(e) => format!(
                "Eval(hgl_harness_ir::Evaluation {{case:{},arguments:{},expected:{}}})",
                e.case,
                list(&e.arguments, argument),
                e.expected
                    .as_ref()
                    .map_or_else(|| "None".into(), |vs| format!("Some({})", slots(vs)))
            ),
        };
        format!("hgl_harness_ir::Step::{s}")
    });
    format!(
        "hgl_harness_ir::Test {{name:{:?}.into(),steps:{steps}}}",
        test.name
    )
}
