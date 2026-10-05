//! Typed lowering of checked generator source control flow.
use hgl_rust_ir::{Plan, Statement, Value};
use hgl_rust_values::{condition_code, global_type, owned_type, statements};
use hgl_source::Ty;
use std::collections::BTreeMap;
mod hoist;

#[derive(Debug)]
enum Block {
    Done,
    Bind(usize, Value, usize),
    Execute(Statement, usize),
    Branch(Value, usize, usize),
    Yield(Value, Value, usize),
}
/// A closed resume graph and the concrete lexical owners retained by its node.
#[derive(Debug)]
pub struct Generator {
    entry: usize,
    blocks: Vec<Block>,
    locals: BTreeMap<usize, Ty>,
}
impl Generator {
    /// Lower a checked generator body; ordinary helper bodies stay lexical.
    pub fn lower(body: &[Statement]) -> Self {
        let mut result = Self {
            entry: 0,
            blocks: vec![Block::Done],
            locals: BTreeMap::new(),
        };
        result.entry = result.sequence(body, 0);
        result
    }
    fn block(&mut self, block: Block) -> usize {
        let id = self.blocks.len();
        self.blocks.push(block);
        id
    }
    fn sequence(&mut self, body: &[Statement], mut next: usize) -> usize {
        for statement in body.iter().rev() {
            next = self.statement(statement, next);
        }
        next
    }
    fn statement(&mut self, statement: &Statement, next: usize) -> usize {
        let block = match statement {
            Statement::Exit => return 0,
            Statement::Let(id, value) | Statement::Var(id, value) => {
                self.locals.insert(*id, value.ty.clone());
                Block::Bind(*id, hoist::value(value), next)
            }
            Statement::Call(value) => Block::Execute(Statement::Call(hoist::value(value)), next),
            Statement::Assign(target, value) => Block::Execute(
                Statement::Assign(hoist::value(target), hoist::value(value)),
                next,
            ),
            Statement::If(condition, yes, no) => {
                let yes = self.sequence(yes, next);
                let no = self.sequence(no, next);
                Block::Branch(hoist::value(condition), yes, no)
            }
            Statement::While(condition, body) => {
                let header = self.block(Block::Done);
                let body = self.sequence(body, header);
                self.blocks[header] = Block::Branch(hoist::value(condition), body, next);
                return header;
            }
            Statement::TimedYield(time, value) => {
                Block::Yield(hoist::value(time), hoist::value(value), next)
            }
            Statement::Borrow(..)
            | Statement::For(..)
            | Statement::Return(_)
            | Statement::Yield(_) => unreachable!("checked generator statement"),
        };
        self.block(block)
    }
    fn has_slots(&self) -> bool {
        self.blocks.iter().any(|block| {
            if let Block::Yield(_, value, _) = block {
                hgl_rust_source_slots::projection(value, |_| String::new()).is_some()
            } else {
                false
            }
        })
    }
    /// Emit concrete fields for the resume position, pending scalar and locals.
    pub fn fields(&self, result: &Ty) -> String {
        let mut code = vec![format!(
            "generator_pc: usize,\ngenerator_previous: Option<hgl_types::EngineTime>,\ngenerator_pending: Option<{}>,\n",
            owned_type(
                &result
                    .clone()
                    .delta()
                    .unwrap_or_else(|_| unreachable!("checked generator result"))
            )
        )];
        if self.has_slots() {
            let payload = result
                .clone()
                .delta()
                .unwrap_or_else(|_| unreachable!("checked generator result"));
            code.push(format!(
                "generator_pending_slot: Option<hgl_store::ValueSlot<{}>>,\n",
                global_type(&payload)
            ));
        }
        for (id, ty) in &self.locals {
            code.push(format!(
                "generator_local{id}: Option<{}>,\n",
                owned_type(ty)
            ));
        }
        code.concat()
    }
    /// Emit construction values without executing any generator expression.
    pub fn initialize(&self) -> String {
        let mut code = vec![format!(
            "generator_pc: {},\ngenerator_previous: None,\ngenerator_pending: None,\n",
            self.entry
        )];
        if self.has_slots() {
            code.push("generator_pending_slot: None,\n".into());
        }
        for id in self.locals.keys() {
            code.push(format!("generator_local{id}: None,\n"));
        }
        code.concat()
    }
    fn clear_locals(&self) -> String {
        self.locals
            .keys()
            .map(|id| format!("self.generator_local{id} = None;\n"))
            .collect::<Vec<_>>()
            .concat()
    }
    /// Reset reconstructible storage and request the body's first evaluation.
    pub fn start(&self) -> String {
        let clear_slot = if self.has_slots() {
            "self.generator_pending_slot=None;"
        } else {
            ""
        };
        format!(
            "self.generator_pc = {};\nself.generator_previous = None;\nself.generator_pending = None;\n{clear_slot} {} _ctx.alarm_in(hgl_types::EngineDelta::from_micros(0))?;\n",
            self.entry,
            self.clear_locals()
        )
    }
    /// Emit a typed evaluation loop which resumes without reevaluating a yield.
    pub fn evaluation(&self, plan: &Plan, result: &Ty) -> String {
        let publication = hgl_rust_deltas::publish(
            &result
                .clone()
                .delta()
                .unwrap_or_else(|_| unreachable!("checked generator result")),
            "publication",
        );
        let mut code = vec![format!(
            "if let Some(publication) = self.generator_pending.take() {{ {publication} }}\n",
        )];
        if self.has_slots() {
            let payload = result
                .clone()
                .delta()
                .unwrap_or_else(|_| unreachable!("checked generator result"));
            code.push(format!(
                "if let Some(publication)=self.generator_pending_slot.take() {{ {} }}\n",
                slot_publication(&payload, "publication")
            ));
        }
        code.push("loop { match self.generator_pc {\n".into());
        for (id, block) in self.blocks.iter().enumerate() {
            code.push(format!("{id} => {{\n"));
            self.emit_block(plan, block, &mut code);
            code.push("}\n".into());
        }
        code.push("_ => return Err(hgl_types::NodeError::new(\"invalid generator resume position\")),\n} }\n".into());
        code.concat()
    }
    fn emit_block(&self, plan: &Plan, block: &Block, code: &mut Vec<String>) {
        match block {
            Block::Done => {
                code.push(self.clear_locals());
                code.push("self.generator_pc = 0; return Ok(());\n".into());
            }
            Block::Bind(id, value, next) => code.push(format!(
                "let value = {}; self.generator_local{id} = Some(value); self.generator_pc = {next};\n",
                condition_code(plan, value)
            )),
            Block::Execute(statement, next) => {
                statements(plan, std::slice::from_ref(statement), code);
                code.push(format!("self.generator_pc = {next};\n"));
            }
            Block::Branch(condition, yes, no) => code.push(format!(
                "self.generator_pc = if {} {{ {yes} }} else {{ {no} }};\n",
                condition_code(plan, condition)
            )),
            Block::Yield(time, value, next) => {
                code.push(timed_yield(plan, time, value, *next));
            }
        }
    }
}
fn timed_yield(plan: &Plan, time: &Value, payload: &Value, next: usize) -> String {
    let resolution = if time.ty == Ty::Duration {
        "{ if time.micros() < 0 { return Err(hgl_types::NodeError::new(\"generator negative yield duration\")); } hgl_types::EngineTime::from_micros(now.micros().checked_add(time.micros()).ok_or_else(|| hgl_types::NodeError::new(\"generator target time overflow\"))?) }"
    } else {
        assert_eq!(time.ty, Ty::DateTime, "checked generator time operand");
        "time"
    };
    let projection = if hgl_rust_finite_domains::prepared(plan) {
        hgl_rust_source_slots::projection(payload, |value| condition_code(plan, value))
    } else {
        None
    };
    let (value, publish, pending) = projection.map_or_else(
        || {
            (
                condition_code(plan, payload),
                hgl_rust_deltas::publish(&payload.ty, "payload"),
                "generator_pending",
            )
        },
        |slot| {
            (
                slot,
                slot_publication(&payload.ty, "payload"),
                "generator_pending_slot",
            )
        },
    );
    format!(
        "let time = {};\nlet payload = {};\nlet now = _ctx.evaluation_time();\nlet target = {resolution};\nif self.generator_previous.is_some_and(|previous| target <= previous) {{ return Err(hgl_types::NodeError::new(\"generator yield times must strictly increase\")); }}\nself.generator_previous = Some(target);\nif target < now {{ self.generator_pc = {next}; continue; }}\nif target == now {{\n{publish} self.generator_pc = {next}; continue;\n}}\nlet delay = target.micros().checked_sub(now.micros()).ok_or_else(|| hgl_types::NodeError::new(\"generator target time difference overflow\"))?;\n_ctx.alarm_in(hgl_types::EngineDelta::from_micros(delay))?;\nself.{pending} = Some(payload); self.generator_pc = {next}; return Ok(());\n",
        condition_code(plan, time),
        value
    )
}
fn slot_publication(ty: &Ty, slot: &str) -> String {
    let marker = global_type(ty);
    if matches!(ty, Ty::Delta(_)) {
        format!("{marker}::apply_slot(self._output,&self.configuration_columns,{slot},_ctx)?;")
    } else if matches!(
        ty,
        Ty::Recursive(_) | Ty::Enum(_) | Ty::List(..) | Ty::Tuple(_) | Ty::Struct(..)
    ) {
        format!(
            "_ctx.prepared().atomic_from::<{marker}>(&self.configuration_columns,{slot},self._output)?;"
        )
    } else {
        format!(
            "_ctx.prepared().scalar::<{marker}>(self._output.id(),self._output.generation(),self.configuration_columns.scalar::<{marker}>(({slot}).fields()))?;"
        )
    }
}
