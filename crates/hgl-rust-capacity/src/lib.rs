//! Cold finite replay capacity planning with statically selected value layouts.
use hgl_rust_ir::Plan;
use hgl_rust_layouts::{delta_storage, delta_type, global_type, whole_payload};
use hgl_source::Ty;
use std::collections::BTreeMap;
fn fields(ty: &Ty) -> Vec<Ty> {
    match ty {
        Ty::Struct(_, fields) => fields.iter().map(|(_, ty)| ty.clone()).collect(),
        Ty::Tuple(fields) => fields.clone(),
        Ty::Delta(origin) => fields(&delta_storage(origin)),
        Ty::Enum(_)
        | Ty::Atomic(_)
        | Ty::Map(..)
        | Ty::List(..)
        | Ty::I64
        | Ty::F64
        | Ty::Bool
        | Ty::Str
        | Ty::Duration
        | Ty::Date
        | Ty::Time
        | Ty::DateTime
        | Ty::CivilDateTime
        | Ty::TimeZone
        | Ty::ZonedTime
        | Ty::ZonedDateTime
        | Ty::Ref(_)
        | Ty::Set(_)
        | Ty::Nullable(_)
        | Ty::Void => Vec::new(),
    }
}
fn collect(ty: &Ty, types: &mut BTreeMap<String, Ty>) {
    if types.insert(global_type(ty), ty.clone()).is_some() {
        return;
    }
    if let Ty::List(child, _) = ty {
        collect(child, types);
    }
    for child in fields(ty) {
        collect(&child, types);
    }
}
/// Exact type-indexed cold maxima for one independently constructed graph.
#[derive(Debug)]
pub struct Capacity {
    types: BTreeMap<String, Ty>,
}
impl Capacity {
    /// Collect ordinary payloads, configurations and global destinations.
    pub fn new(plan: &Plan) -> Self {
        let mut types = BTreeMap::new();
        for node in &plan.nodes {
            for value in &node.configuration {
                collect(&value.ty, &mut types);
            }
            for (_, ty) in &node.globals {
                collect(ty, &mut types);
            }
            for ty in std::iter::once(&node.result).chain(node.inputs.iter().map(|(_, _, ty)| ty)) {
                if ty.publication() {
                    collect(&delta_type(ty), &mut types);
                }
            }
        }
        Self { types }
    }
    fn index(&self, ty: &Ty) -> usize {
        self.types
            .keys()
            .position(|key| *key == global_type(ty))
            .unwrap_or_else(|| unreachable!("collected ordinary type"))
    }
    /// Include one already materialized configuration, recursively and without recipes.
    pub fn include(&self, ty: &Ty, value: &str) -> String {
        let index = self.index(ty);
        if let Ty::List(child, _) = ty {
            return format!(
                "capacity.limit{index}=capacity.limit{index}.max(({value}).len());for value in ({value}).iter() {{{}}}",
                self.include(child, "value")
            );
        }
        let children = fields(ty);
        if !children.is_empty() || matches!(ty, Ty::Struct(..) | Ty::Tuple(_) | Ty::Delta(_)) {
            return children
                .iter()
                .enumerate()
                .map(|(i, child)| self.include(child, &format!("&({value}).{i}")))
                .collect();
        }
        format!(
            "<{} as hgl_store::PreparedValue>::include(&mut capacity.limit{index},{value});",
            global_type(ty)
        )
    }
    /// Assemble exact typed bounds; structural deltas include every possible changed member.
    pub fn bounds(&self, ty: &Ty) -> String {
        if let Ty::Delta(origin) = ty {
            let children = fields(ty);
            let lengths = match origin.as_ref() {
                Ty::List(_, Some(n)) => vec![n.to_string(); children.len()],
                Ty::Map(key, _) | Ty::Set(key) => vec![
                    format!(
                        "<{} as hgl_store::Key>::ids(&store.keys).len()",
                        global_type(key)
                    );
                    children.len()
                ],
                Ty::Enum(_)
                | Ty::Atomic(_)
                | Ty::Tuple(_)
                | Ty::Delta(_)
                | Ty::List(_, None)
                | Ty::Struct(..)
                | Ty::I64
                | Ty::F64
                | Ty::Bool
                | Ty::Str
                | Ty::Duration
                | Ty::Date
                | Ty::Time
                | Ty::DateTime
                | Ty::CivilDateTime
                | Ty::TimeZone
                | Ty::ZonedTime
                | Ty::ZonedDateTime
                | Ty::Ref(_)
                | Ty::Nullable(_)
                | Ty::Void => vec!["1".into(); children.len()],
            };
            let args = children
                .iter()
                .zip(lengths)
                .enumerate()
                .map(|(i, (child, len))| {
                    let Ty::List(element, _) = child else {
                        unreachable!("delta list field")
                    };
                    format!(
                        "field{i}:hgl_store::ListBounds {{len:{len},element:{}}}",
                        self.bounds(element)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            return format!("PreparedBounds{} {{{args}}}", global_type(ty));
        }
        if let Ty::List(child, size) = ty {
            return format!(
                "hgl_store::ListBounds {{len:{},element:{}}}",
                size.map_or_else(
                    || format!("capacity.limit{}", self.index(ty)),
                    |n| n.to_string()
                ),
                self.bounds(child)
            );
        }
        if matches!(ty, Ty::Struct(..) | Ty::Tuple(_)) {
            let args = fields(ty)
                .iter()
                .enumerate()
                .map(|(i, child)| format!("field{i}:{}", self.bounds(child)))
                .collect::<Vec<_>>()
                .join(",");
            return format!("PreparedBounds{} {{{args}}}", global_type(ty));
        }
        format!("capacity.limit{}", self.index(ty))
    }
    /// Emit a compact cold capacity record without runtime type lookup.
    pub fn declaration(&self) -> String {
        let fields = (0..self.types.len())
            .map(|i| format!("limit{i}:usize,"))
            .collect::<Vec<_>>()
            .concat();
        format!("#[derive(Default)] struct FiniteCapacity {{{fields}}}\n")
    }
    /// Prepare every concrete leaf, including inactive finite keyed descendants.
    pub fn output(&self, ty: &Ty, id: &str) -> String {
        if let Some(payload) = whole_payload(ty) {
            return format!(
                "{{let bounds={};let storage=store.prepared();storage.atomic.prepare_output::<{}>(storage.bindings,{id},&bounds)?;}}",
                self.bounds(payload),
                global_type(payload)
            );
        }
        match ty {
            Ty::Map(key, child) => format!(
                "{{let output={id};let keys=<{} as hgl_store::Key>::ids(&store.keys).to_vec();store.prepare_collection(output,&keys,|store,owner|store.add_shaped_output(owner,<{} as hgl_store::shapes::Shape>::shape()));for index in 0..store.bindings().output(output).members.prepared.len() {{let child=store.bindings().output(output).members.prepared[index].1;{}}}}}",
                global_type(key),
                hgl_rust_deltas::shape_marker(child),
                self.output(child, "child")
            ),
            Ty::Set(key) => format!(
                "{{let output={id};let keys=<{} as hgl_store::Key>::ids(&store.keys).to_vec();store.prepare_collection(output,&keys,|store,owner|store.add_output::<bool>(owner).id());}}",
                global_type(key)
            ),
            Ty::List(child, Some(_)) => format!(
                "{{let output={id};for index in 0..store.bindings().output(output).fixed.len() {{let child=store.bindings().output(output).fixed[index];{}}}}}",
                self.output(child, "child")
            ),
            Ty::Struct(_, children) => children
                .iter()
                .enumerate()
                .map(|(i, (_, child))| {
                    format!(
                        "{{let child=store.bindings().output({id}).fixed[{i}];{}}}",
                        self.output(child, "child")
                    )
                })
                .collect::<Vec<_>>()
                .concat(),
            Ty::Tuple(children) => children
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    format!(
                        "{{let child=store.bindings().output({id}).fixed[{i}];{}}}",
                        self.output(child, "child")
                    )
                })
                .collect::<Vec<_>>()
                .concat(),
            Ty::Enum(_)
            | Ty::Atomic(_)
            | Ty::Delta(_)
            | Ty::List(_, None)
            | Ty::I64
            | Ty::F64
            | Ty::Bool
            | Ty::Str
            | Ty::Duration
            | Ty::Date
            | Ty::Time
            | Ty::DateTime
            | Ty::CivilDateTime
            | Ty::TimeZone
            | Ty::ZonedTime
            | Ty::ZonedDateTime
            | Ty::Ref(_)
            | Ty::Nullable(_)
            | Ty::Void => format!(
                "store.prepared().prepare_scalar::<{}>({id},{})?;",
                global_type(ty),
                self.bounds(ty)
            ),
        }
    }
}
