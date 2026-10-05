//! Cold finite replay capacity planning with statically selected value layouts.
use hgl_rust_ir::Plan;
use hgl_rust_layouts::{delta_storage, delta_type, global_type, whole_payload};
use hgl_source::Ty;
use std::collections::BTreeMap;
use std::fmt::Write as _;
fn fields(ty: &Ty) -> Vec<Ty> {
    if let Ty::Struct(_, fields) = ty {
        return fields.iter().map(|(_, ty)| ty.clone()).collect();
    }
    if let Ty::Tuple(fields) = ty {
        return fields.clone();
    }
    if let Ty::Delta(origin) = ty {
        return fields(&delta_storage(origin));
    }
    Vec::new()
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
    scalar_sets: Vec<Ty>,
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
        for value in hgl_rust_finite_domains::constants(plan) {
            collect(&value.ty, &mut types);
        }
        let scalar_sets = plan
            .nodes
            .iter()
            .filter_map(|node| {
                if let Ty::Set(key) = &node.result {
                    if matches!(key.as_ref(), Ty::I64 | Ty::Bool)
                        && node.inputs.iter().any(|(_, _, ty)| ty == key.as_ref())
                    {
                        Some((**key).clone())
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        Self { types, scalar_sets }
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
            let mut extra = String::new();
            if let Ty::Delta(origin) = ty {
                extra = hgl_rust_finite_domains::include(
                    origin,
                    value,
                    &format!("capacity.topology{index}"),
                );
                for i in 0..children.len() {
                    write!(extra,"capacity.width{index}_{i}=capacity.width{index}_{i}.max(({value}).{i}.len());").unwrap_or_else(|_|unreachable!("String formatting"));
                }
            }
            return extra
                + &children
                    .iter()
                    .enumerate()
                    .map(|(i, child)| self.include(child, &format!("&({value}).{i}")))
                    .collect::<String>();
        }
        let mut result = format!(
            "<{} as hgl_store::PreparedValue>::include(&mut capacity.limit{index},{value});",
            global_type(ty)
        );
        if self.scalar_sets.contains(ty) {
            let root = self.index(&Ty::Delta(Box::new(Ty::Set(Box::new(ty.clone())))));
            let id = if *ty == Ty::Bool {
                format!("i64::from(*({value}))")
            } else {
                format!("*({value})")
            };
            write!(
                result,
                "capacity.topology{root}.children.entry({id}).or_default();"
            )
            .unwrap_or_else(|_| unreachable!("String formatting"));
            if *ty == Ty::I64 {
                write!(
                    result,
                    "capacity.topology{root}.children.entry(({id}).wrapping_neg()).or_default();"
                )
                .unwrap_or_else(|_| unreachable!("String formatting"));
            }
        }
        result
    }
    /// Merge checked compile-time constants without moving hook evaluation earlier.
    pub fn constants(&self, plan: &Plan, emit: impl Fn(&hgl_rust_ir::Value) -> String) -> String {
        hgl_rust_finite_domains::constants(plan).into_iter().map(|value|format!("{{let value=(||->hgl_types::NodeResult<_>{{Ok({})}})().map_err(|e|e.message)?;{}}}",emit(value),self.include(&value.ty,"&value"))).collect::<Vec<_>>().concat()
    }
    /// Retain finite primitive domains for the pre-existing scalar-to-set operators.
    pub fn scalar_sets(&self) -> String {
        self.scalar_sets.iter().map(|ty|{let index=self.index(&Ty::Delta(Box::new(Ty::Set(Box::new(ty.clone())))));let value=if *ty==Ty::Bool {"(key!=0)"}else{"key"};format!("for &key in capacity.topology{index}.children.keys() {{<{} as hgl_store::Key>::prepare(&mut store.keys,&{value}).map_err(|e|e.message)?;}}capacity.width{index}_0=capacity.width{index}_0.max(capacity.topology{index}.children.len());capacity.width{index}_1=capacity.width{index}_1.max(capacity.topology{index}.children.len());",global_type(ty))}).collect::<Vec<_>>().concat()
    }
    /// Reserve the fixed textual envelope of built-in scalar formatters.
    pub fn native_limits(&self, plan: &Plan) -> String {
        if plan.natives.iter().any(|native| {
            native.name == "hgraph.native::as_str"
                && native.result == Ty::Str
                && native.args.as_slice() != [Ty::Str]
        }) {
            format!(
                "capacity.limit{}=capacity.limit{}.max(64);",
                self.index(&Ty::Str),
                self.index(&Ty::Str)
            )
        } else {
            String::new()
        }
    }
    /// Assemble exact typed bounds; structural deltas include every possible changed member.
    pub fn bounds(&self, ty: &Ty) -> String {
        if let Ty::Delta(origin) = ty {
            let children = fields(ty);
            let lengths = if let Ty::List(_, Some(n)) = origin.as_ref() {
                vec![n.to_string(); children.len()]
            } else if matches!(origin.as_ref(), Ty::Map(..) | Ty::Set(_)) {
                (0..children.len())
                    .map(|i| format!("capacity.width{}_{i}", self.index(ty)))
                    .collect()
            } else {
                vec!["1".into(); children.len()]
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
        let domains = self
            .types
            .values()
            .enumerate()
            .filter_map(|(index, ty)| {
                if matches!(ty, Ty::Delta(_)) {
                    Some(format!(
                        "topology{index}:FiniteTopology,{}",
                        (0..self::fields(ty).len())
                            .map(|i| format!("width{index}_{i}:usize,"))
                            .collect::<Vec<_>>()
                            .concat()
                    ))
                } else {
                    None
                }
            })
            .collect::<String>();
        format!(
            "{}#[derive(Default)] struct FiniteCapacity {{{fields}{domains}}}\n",
            hgl_rust_finite_domains::DECLARATION
        )
    }
    /// Prepare every concrete leaf, including inactive finite keyed descendants.
    pub fn output(&self, ty: &Ty, id: &str) -> String {
        let domain = if hgl_rust_deltas::structural(ty) {
            format!("&capacity.topology{}", self.index(&delta_type(ty)))
        } else {
            "&empty".into()
        };
        self.output_with(ty, id, &domain)
    }
    fn output_with(&self, ty: &Ty, id: &str, domain: &str) -> String {
        if let Some(payload) = whole_payload(ty) {
            return format!(
                "{{let bounds={};let storage=store.prepared();storage.atomic.prepare_output::<{}>(storage.bindings,{id},&bounds)?;}}",
                self.bounds(payload),
                global_type(payload)
            );
        }
        match ty {
            Ty::Map(_, child) => format!(
                "{{let output={id};let domain={domain};let keys=domain.children.keys().copied().collect::<Vec<_>>();store.prepare_collection(output,&keys,|store,owner|store.add_shaped_output(owner,<{} as hgl_store::shapes::Shape>::shape()));for (&key,domain) in &domain.children {{let child=store.bindings().prepared_output(output,key).expect(\"prepared domain child\");{}}}}}",hgl_rust_deltas::shape_marker(child),self.output_with(child,"child","domain")
            ),
            Ty::Set(_) => format!("{{let output={id};let domain={domain};let keys=domain.children.keys().copied().collect::<Vec<_>>();store.prepare_collection(output,&keys,|store,owner|store.add_output::<bool>(owner).id());}}"),
            Ty::List(child, Some(_)) => format!(
                "{{let output={id};let domain={domain};for index in 0..store.bindings().output(output).fixed.len() {{let child=store.bindings().output(output).fixed[index];let domain=domain.children.get(&(index as i64)).unwrap_or(&empty);{}}}}}",
                self.output_with(child, "child","domain")
            ),
            Ty::Struct(_, children) => children
                .iter()
                .enumerate()
                .map(|(i, (_, child))| {
                    format!(
                        "{{let child=store.bindings().output({id}).fixed[{i}];let domain=({domain}).children.get(&{i}).unwrap_or(&empty);{}}}",
                        self.output_with(child, "child","domain")
                    )
                })
                .collect::<Vec<_>>()
                .concat(),
            Ty::Tuple(children) => children
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    format!(
                        "{{let child=store.bindings().output({id}).fixed[{i}];let domain=({domain}).children.get(&{i}).unwrap_or(&empty);{}}}",
                        self.output_with(child, "child","domain")
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
