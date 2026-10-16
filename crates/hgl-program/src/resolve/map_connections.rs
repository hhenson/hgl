//! Graph map indexing selects a live child endpoint without reading its payload.
use super::{Checker, Issue, Kind, Literal, Node, Statement, Ty, Value, require_payload};

fn reference_leaf(ty: &Ty) -> bool {
    match ty {
        Ty::Ref(_) => true,
        Ty::Atomic(child)
        | Ty::List(child, _)
        | Ty::Set(child)
        | Ty::Nullable(child)
        | Ty::Delta(child)
        | Ty::Rolling(child, _) => reference_leaf(child),
        Ty::Map(key, child) | Ty::Result(key, child) => {
            reference_leaf(key) || reference_leaf(child)
        }
        Ty::Tuple(children) => children.iter().any(reference_leaf),
        Ty::Struct(_, fields, _) => fields.iter().any(|(_, child)| reference_leaf(child)),
        Ty::Error
        | Ty::Family(_)
        | Ty::Recursive(_)
        | Ty::Enum(_)
        | Ty::Native(_)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Bytes
        | Ty::Any
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Void => false,
    }
}

impl Checker {
    pub(super) fn map_connection(&mut self, parent: Value, mut key: Value) -> Result<Value, Issue> {
        let Kind::Wire(source) = parent.kind else {
            unreachable!("checked graph map")
        };
        let map = if let Ty::Ref(child) = parent.ty {
            *child
        } else {
            parent.ty
        };
        let Ty::Map(expected, child) = &map else {
            unreachable!("checked graph map")
        };
        if key.ty != **expected {
            return Err("map connection key type mismatch".into());
        }
        require_payload(&key)?;
        if reference_leaf(child) {
            return Err("map connection selection of REF-containing children is outside the initial profile".into());
        }
        let result = Ty::Ref(child.clone());
        let mut inputs = vec![("book".into(), source, map.clone())];
        let mut configuration = Vec::new();
        let guard = if let Kind::Wire(source) = key.kind {
            inputs.push(("key".into(), source, key.ty.clone()));
            key.kind = Kind::Input(1, false);
            Value::new(Ty::Bool, Kind::Query("valid".into(), vec![key.clone()]))
        } else {
            configuration.push(key.clone());
            key.kind = Kind::Configuration(0);
            Value::new(Ty::Bool, Kind::Literal(Literal::Bool(true)))
        };
        let selected = Value::new(
            result.clone(),
            Kind::Query(
                "\0map_connection".into(),
                vec![Value::new(map, Kind::Input(0, false)), key],
            ),
        );
        let wire = self.plan.nodes.len();
        self.plan.nodes.push(Node {
            name: "hgl.map_connection".into(),
            inputs,
            result: result.clone(),
            alarm: false,
            generator: None,
            start: Vec::new(),
            stop: Vec::new(),
            global_state: false,
            globals: Vec::new(),
            configuration,
            caches: Vec::new(),
            handlers: vec![(Some(guard), vec![Statement::Return(selected)])],
        });
        Ok(Value::new(result, Kind::Wire(wire)))
    }
}
