//! Fixed nominal graph construction retains checked child connections.
use crate::ir::{Kind, Node, Plan, Statement, Value};
use hgl_source::{Literal, Ty};

/// Reduce a temporal nominal constructor to a fixed connection assembly.
pub fn constructor(value: Value, plan: &mut Plan) -> Result<Value, String> {
    let Kind::Construct(supplied) = &value.kind else {
        return Ok(value);
    };
    if !supplied
        .iter()
        .any(|(_, child)| matches!(child.kind, Kind::Wire(_)))
    {
        return Ok(value);
    }
    let Ty::Struct(_, fields, optional) = &value.ty else {
        return Err("graph connection assembly currently requires a fixed nominal struct".into());
    };
    if !optional.is_empty() || supplied.len() != fields.len() {
        return Err("optional graph constructor fields are outside this backend profile".into());
    }
    let mut inputs = Vec::new();
    for (index, (name, ty)) in fields.iter().enumerate() {
        let child = supplied
            .iter()
            .find(|(field, _)| *field == index)
            .map(|(_, child)| child)
            .ok_or("missing graph constructor field")?;
        let Kind::Wire(source) = child.kind else {
            return Err("mixed ordinary and temporal graph constructor fields are outside this backend profile".into());
        };
        if child.ty != *ty {
            return Err("graph constructor child connection type mismatch".into());
        }
        if matches!(plan.nodes[source].result, Ty::Ref(_)) && !assembly(&plan.nodes[source]) {
            return Err(
                "REF child graph constructor connections are outside this backend profile".into(),
            );
        }
        inputs.push((name.clone(), source, ty.clone()));
    }
    let result = Ty::Ref(Box::new(value.ty.clone()));
    let index = plan.nodes.len();
    plan.nodes.push(Node {
        name: "hgl.assembly".into(),
        inputs,
        result: result.clone(),
        alarm: false,
        generator: None,
        start: Vec::new(),
        stop: Vec::new(),
        global_state: false,
        globals: Vec::new(),
        configuration: Vec::new(),
        caches: Vec::new(),
        handlers: vec![(
            Some(Value::new(Ty::Bool, Kind::Literal(Literal::Bool(true)))),
            vec![Statement::Return(Value::new(
                result,
                Kind::Query("\0assembly".into(), Vec::new()),
            ))],
        )],
    });
    Ok(Value::new(value.ty, Kind::Wire(index)))
}

/// Assemble a finite list of graph endpoint designations, preserving REF carriers.
pub fn reference_list(values: Vec<Value>, plan: &mut Plan) -> Result<Value, String> {
    let first = values
        .first()
        .ok_or("graph reference list requires entries")?;
    let child = if let Ty::Ref(child) = &first.ty {
        *child.clone()
    } else {
        first.ty.clone()
    };
    let reference = Ty::Ref(Box::new(child));
    let mut inputs = Vec::new();
    for (index, value) in values.into_iter().enumerate() {
        let Kind::Wire(mut source) = value.kind else {
            return Err(
                "mixed ordinary and temporal graph list entries are outside this backend profile"
                    .into(),
            );
        };
        if value.ty != reference && Ty::Ref(Box::new(value.ty.clone())) != reference {
            return Err("graph reference list entry type mismatch".into());
        }
        if !matches!(plan.nodes[source].result, Ty::Ref(_)) {
            let input = Value::new(reference.clone(), Kind::Input(0, false));
            source = append(
                plan,
                vec![("value".into(), source, reference.clone())],
                reference.clone(),
                input,
            );
        }
        inputs.push((index.to_string(), source, reference.clone()));
    }
    let ty = Ty::List(Box::new(reference), Some(inputs.len()));
    let result = Ty::Ref(Box::new(ty.clone()));
    let marker = Value::new(result.clone(), Kind::Query("\0assembly".into(), Vec::new()));
    let index = append(plan, inputs, result, marker);
    Ok(Value::new(ty, Kind::Wire(index)))
}
fn append(plan: &mut Plan, inputs: Vec<(String, usize, Ty)>, result: Ty, value: Value) -> usize {
    let index = plan.nodes.len();
    plan.nodes.push(Node {
        name: if matches!(value.kind, Kind::Input(..)) {
            "hgl.designation"
        } else {
            "hgl.assembly"
        }
        .into(),
        inputs,
        result,
        alarm: false,
        generator: None,
        start: Vec::new(),
        stop: Vec::new(),
        global_state: false,
        globals: Vec::new(),
        configuration: Vec::new(),
        caches: Vec::new(),
        handlers: vec![(
            Some(Value::new(Ty::Bool, Kind::Literal(Literal::Bool(true)))),
            vec![Statement::Return(value)],
        )],
    });
    index
}

fn assembly(node: &Node) -> bool {
    let [(_, body)] = node.handlers.as_slice() else {
        return false;
    };
    let [Statement::Return(value)] = body.as_slice() else {
        return false;
    };
    matches!(&value.kind, Kind::Query(op, args) if op == "\0assembly" && args.is_empty())
}
