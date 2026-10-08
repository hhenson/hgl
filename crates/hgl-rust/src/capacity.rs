//! Cold finite replay capacity planning with statically selected value layouts.
use crate::layouts::{delta_storage, delta_type, global_type, whole_payload};
use hgl_semantics::ir::Plan;
use hgl_source::Ty;
use std::collections::BTreeMap;
use std::fmt::Write as _;
fn fields(ty: &Ty) -> Vec<Ty> {
    if let Ty::Family(family) = ty {
        return fields(&crate::layouts::family_storage(family));
    }
    if let Ty::Struct(_, fields, _) = ty {
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
    if let Some(child) = crate::collections::element(ty) {
        collect(&child, types);
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
        for value in crate::finite_domains::constants(plan) {
            collect(&value.ty, &mut types);
        }
        let mut scalar_sets: Vec<_> = plan
            .nodes
            .iter()
            .filter_map(|node| {
                if let Ty::Set(key) = &node.result {
                    if matches!(key.as_ref(), Ty::I64 | Ty::Bool)
                        && (crate::finite_domains::mutations(&node.start) > 0
                            || node
                                .handlers
                                .iter()
                                .any(|(_, body)| crate::finite_domains::mutations(body) > 0))
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
        scalar_sets.sort();
        scalar_sets.dedup();
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
        if let Some(child) = crate::collections::element(ty) {
            return format!(
                "capacity.limit{index}=capacity.limit{index}.max(({value}).len());for value in ({value}).iter() {{{}}}",
                self.include(&child, "value")
            );
        }
        let children = fields(ty);
        if !children.is_empty() || matches!(ty, Ty::Struct(..) | Ty::Tuple(_) | Ty::Delta(_)) {
            let mut extra = String::new();
            if let Ty::Delta(origin) = ty {
                extra = crate::finite_domains::include(
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
                    .map(|(i, child)| {
                        if optional(ty, i) {
                            format!(
                                "if let Some(value)=(&({value}).{i}).as_ref() {{{}}}",
                                self.include(child, "value")
                            )
                        } else {
                            self.include(child, &format!("&({value}).{i}"))
                        }
                    })
                    .collect::<String>();
        }
        format!(
            "<{} as hgl_store::PreparedValue>::include(&mut capacity.limit{index},{value});",
            global_type(ty)
        )
    }
    /// Merge checked compile-time constants without moving hook evaluation earlier.
    pub fn constants(
        &self,
        plan: &Plan,
        emit: impl Fn(&hgl_semantics::ir::Value) -> String,
    ) -> String {
        let constants = crate::finite_domains::constants(plan).into_iter().map(|value|format!("{{let value=(||->hgl_types::NodeResult<_>{{Ok({})}})().map_err(|e|e.message)?;{}}}",emit(value),self.include(&value.ty,"&value"))).collect::<Vec<_>>().concat();
        constants
            + &crate::keyed::constructors(
                plan,
                |ty| {
                    self.types
                        .contains_key(&global_type(ty))
                        .then(|| format!("capacity.topology{}", self.index(ty)))
                },
                |ty, field, count| {
                    let index = self.index(ty);
                    format!(
                        "capacity.width{index}_{field}=capacity.width{index}_{field}.max({count});"
                    )
                },
                emit,
            )
    }
    /// Retain finite primitive domains for the pre-existing scalar-to-set operators.
    pub fn scalar_sets(&self, plan: &Plan) -> String {
        let mut code = crate::finite_domains::widths(plan, |ty| {
            let index = self.index(&delta_type(ty));
            format!("capacity.width{index}_0.max(capacity.width{index}_1).max(1)")
        });
        for ty in &self.scalar_sets {
            let index = self.index(&Ty::Delta(Box::new(Ty::Set(Box::new(ty.clone())))));
            let width = plan
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, node)| node.result == Ty::Set(Box::new(ty.clone())))
                .fold("1usize".to_owned(), |width, (node, _)| {
                    format!("({width}).max(width{node})")
                });
            let capacity = if *ty == Ty::Bool {
                "2"
            } else {
                "cycles.checked_add(1).and_then(|cycles|cycles.checked_mul(width)).ok_or(\"prepared key pool capacity overflow\")?"
            };
            write!(code,"{{let width={width};capacity.pool{index}={capacity};capacity.width{index}_0=width;capacity.width{index}_1=width;}}").unwrap_or_else(|_|unreachable!("String formatting"));
        }
        code
    }
    /// Reserve the fixed textual envelope of built-in scalar formatters.
    pub fn native_limits(&self, plan: &Plan) -> String {
        crate::pure_bounds::native_limits(plan, || self.index(&Ty::Str))
    }
    /// Widen retained text for checked pure concatenation expressions.
    pub fn text_limits(&self, plan: &Plan) -> String {
        let factor = crate::pure_bounds::text_factor(plan);
        if factor == 1 {
            return String::new();
        }
        let index = self.index(&Ty::Str);
        format!(
            "capacity.limit{index}=capacity.limit{index}.checked_mul({factor}).ok_or(\"prepared text capacity overflow\")?;"
        )
    }
    /// Assemble exact typed bounds; structural deltas include every possible changed member.
    pub fn bounds(&self, ty: &Ty) -> String {
        if let Some(bounds) =
            crate::snapshots::bounds(ty, |ty| self.bounds(ty), |ty| self.snapshot_domain(ty))
        {
            return bounds;
        }
        if matches!(ty, Ty::Recursive(_)) {
            return format!("capacity.limit{}.clone()", self.index(ty));
        }
        if let Ty::Delta(origin) = ty {
            let children = fields(ty);
            let lengths = if let Ty::List(_, Some(n)) = origin.as_ref() {
                vec![n.to_string(); children.len()]
            } else if matches!(
                origin.as_ref(),
                Ty::Map(..) | Ty::Set(_) | Ty::List(_, None)
            ) {
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
        if let Some(child) = crate::collections::element(ty) {
            let size = if let Ty::List(_, size) = ty {
                *size
            } else {
                None
            };
            return format!(
                "hgl_store::ListBounds {{len:{},element:{}}}",
                size.map_or_else(
                    || format!("capacity.limit{}", self.index(ty)),
                    |n| n.to_string()
                ),
                self.bounds(&child)
            );
        }
        if matches!(ty, Ty::Family(_) | Ty::Struct(..) | Ty::Tuple(_)) {
            let args = fields(ty)
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    let bounds = self.bounds(child);
                    if optional(ty, i) {
                        format!("field{i}:Some({bounds})")
                    } else {
                        format!("field{i}:{bounds}")
                    }
                })
                .collect::<Vec<_>>()
                .join(",");
            return format!("PreparedBounds{} {{{args}}}", global_type(ty));
        }
        format!("capacity.limit{}", self.index(ty))
    }
    fn snapshot_domain(&self, ty: &Ty) -> String {
        format!(
            "capacity.topology{}.children.len()",
            self.index(&delta_type(ty))
        )
    }
    /// Emit a compact cold capacity record without runtime type lookup.
    pub fn declaration(&self) -> String {
        let fields = self
            .types
            .values()
            .enumerate()
            .map(|(i, ty)| {
                if matches!(ty, Ty::Recursive(_)) {
                    format!(
                        "limit{i}:<{} as hgl_store::PreparedValue>::Bounds,",
                        global_type(ty)
                    )
                } else {
                    format!("limit{i}:usize,")
                }
            })
            .collect::<Vec<_>>()
            .concat();
        let domains = self
            .types
            .values()
            .enumerate()
            .filter_map(|(index, ty)| {
                if matches!(ty, Ty::Delta(_)) {
                    Some(format!(
                        "topology{index}:FiniteTopology,pool{index}:usize,{}",
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
            "{}#[derive(Default)] struct FiniteCapacity {{arrivals:usize,{fields}{domains}}}\n",
            crate::finite_domains::DECLARATION
        )
    }
    /// Prepare every concrete leaf, including inactive finite keyed descendants.
    pub fn output(&self, ty: &Ty, id: &str) -> String {
        let domain = if crate::deltas::structural(ty) {
            format!("&capacity.topology{}", self.index(&delta_type(ty)))
        } else {
            "&empty".into()
        };
        self.output_with(ty, id, &domain)
    }
    fn output_with(&self, ty: &Ty, id: &str, domain: &str) -> String {
        if let Ty::Rolling(payload, _) = ty {
            return format!(
                "{{let bounds={};let storage=store.prepared();storage.rolling.prepare_output::<{}>(storage.bindings,{id},&bounds,capacity.arrivals)?;}}",
                self.bounds(payload),
                crate::windows::marker(ty)
            );
        }
        if let Some(payload) = whole_payload(ty) {
            return format!(
                "{{let bounds={};let storage=store.prepared();storage.atomic.prepare_output::<{}>(storage.bindings,{id},&bounds)?;}}",
                self.bounds(payload),
                global_type(payload)
            );
        }
        match ty {
            Ty::Map(_, child) | Ty::List(child,None) => format!(
                "{{let output={id};let domain={domain};let keys=domain.children.keys().copied().collect::<Vec<_>>();store.prepare_collection(output,&keys,|store,owner|store.add_shaped_output(owner,<{} as hgl_store::shapes::Shape>::shape()));for (&key,domain) in &domain.children {{let child=store.prepared().bindings.prepared_output(output,key).expect(\"prepared domain child\");{}}}}}",crate::deltas::shape_marker(child),self.output_with(child,"child","domain")
            ),
            Ty::Set(key) => {
                let pooled=self.scalar_sets.contains(key);
                let index=self.index(&delta_type(ty));
                let keys=if pooled {format!("(0..capacity.pool{index}).map(|n|i64::try_from(n).expect(\"bounded key capacity\")).collect::<Vec<_>>()")} else {"domain.children.keys().copied().collect::<Vec<_>>()".into()};
                let prepare=if pooled {"store.prepared().bindings.prepare_pool(output);"}else{""};
                format!("{{let output={id};let domain={domain};let keys={keys};store.prepare_collection(output,&keys,|store,owner|store.add_output::<bool>(owner).id());{prepare}}}")
            },
            Ty::List(child, Some(_)) => format!(
                "{{let output={id};let domain={domain};for index in 0..store.bindings().output(output).fixed.len() {{let child=store.bindings().output(output).fixed[index];let domain=domain.children.get(&(index as i64)).unwrap_or(&empty);{}}}}}",
                self.output_with(child, "child","domain")
            ),
            Ty::Struct(_, children, _) => children
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
            | Ty::Atomic(_) | Ty::Rolling(..)
            | Ty::Delta(_)
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
            | Ty::Recursive(_)
            | Ty::Family(_)
            | Ty::Void => format!(
                "store.prepared().prepare_scalar::<{}>({id},{})?;",
                global_type(ty),
                self.bounds(ty)
            ),
        }
    }
}

fn optional(ty: &Ty, index: usize) -> bool {
    matches!(ty,Ty::Struct(_,_,optional) if optional.contains(&index))
        || matches!(ty, Ty::Family(_)) && index > 0
}
