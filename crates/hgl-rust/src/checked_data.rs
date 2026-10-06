//! Cold typed conversion and checked lexical test emission.
use crate::enums::metadata as enum_data;
use hgl_semantics::harness_ir::{Argument, Step, Test};
use hgl_semantics::ir::{DeltaEntry, Kind, Statement, Value};
use hgl_source::{Literal, TemporalLiteral};
fn list<T>(items: &[T], f: impl Fn(&T) -> String) -> String {
    format!(
        "vec![{}]",
        items.iter().map(f).collect::<Vec<_>>().join(",")
    )
}
pub use crate::type_data::ty;
fn literal(l: &Literal) -> String {
    let inner = match l {
        Literal::Enum(e, number) => format!("Enum({}, {number})", enum_data(e)),
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
        DeltaEntry::Add(l) => format!("Add({})", value(l)),
        DeltaEntry::Remove(l) => format!("Remove({})", value(l)),
        DeltaEntry::Keyed(k, v) => format!("Keyed({},{})", value(k), value(v)),
        DeltaEntry::Child(i, v) => format!("Child({i},{})", value(v)),
    };
    format!("hgl_semantics::ir::DeltaEntry::{p}")
}
/// Emit one exact checked expression for cold preparation.
pub fn value(v: &Value) -> String {
    let kind = match &v.kind {
        Kind::Captured(n, vs) => format!(
            "Captured({n},{})",
            list(vs, |(i, v)| format!("({i},{})", value(v)))
        ),
        Kind::IsPresent(v) => format!("IsPresent(Box::new({}))", value(v)),
        Kind::Present(v) => format!("Present(Box::new({}))", value(v)),
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
        "hgl_semantics::ir::Value::new({},hgl_semantics::ir::Kind::{kind})",
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
    format!("hgl_semantics::ir::Statement::{s}")
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
    format!("hgl_semantics::harness_ir::Argument::{a}")
}
fn evaluation(e: &hgl_semantics::harness_ir::Evaluation) -> String {
    format!(
        "hgl_semantics::harness_ir::Evaluation {{case:{},arguments:{},expected:{}}}",
        e.case,
        list(&e.arguments, argument),
        e.expected
            .as_ref()
            .map_or_else(|| "None".into(), |vs| format!("Some({})", slots(vs)))
    )
}
fn step(s: &Step) -> String {
    let s = match s {
        Step::Raises(code, body) => format!("Raises({code:?}.into(),{})", list(body, step)),
        Step::Ordinary(s) => format!("Ordinary({})", statement(s)),
        Step::Assert(v) => format!("Assert({})", value(v)),
        Step::Eval(e) => format!("Eval({})", evaluation(e)),
        Step::BindEval(id, t, e) => format!("BindEval({id},{},{})", ty(t), evaluation(e)),
        Step::If(v, a, b) => format!("If({},{},{})", value(v), list(a, step), list(b, step)),
    };
    format!("hgl_semantics::harness_ir::Step::{s}")
}
/// Emit checked lexical test data without evaluating any expression.
pub fn test(test: &Test) -> String {
    format!(
        "hgl_semantics::harness_ir::Test {{name:{:?}.into(),steps:{}}}",
        test.name,
        list(&test.steps, step)
    )
}
