//! Typed lowering of checked generator source control flow.
use hgl_rust_ir::{Plan, Statement, Value};
use hgl_rust_values::{condition_code, owned_type, statements};
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
    /// Emit concrete fields for the resume position, pending scalar and locals.
    pub fn fields(&self, result: &Ty) -> String {
        let mut code = vec![format!(
            "generator_pc: usize,\ngenerator_pending: Option<{}>,\n",
            owned_type(
                &result
                    .clone()
                    .delta()
                    .unwrap_or_else(|_| unreachable!("checked generator result"))
            )
        )];
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
            "generator_pc: {},\ngenerator_pending: None,\n",
            self.entry
        )];
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
        format!(
            "self.generator_pc = {};\nself.generator_pending = None;\n{} _ctx.alarm_in(hgl_types::EngineDelta::from_micros(0))?;\n",
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
        let mut code = vec![String::from(
            "let mut generator_published = false;\nif let Some(publication) = self.generator_pending.take() { _ctx.set(self._output, publication); generator_published = true; }\nloop { match self.generator_pc {\n",
        )];
        code[0] = code[0].replace("_ctx.set(self._output, publication);", &publication);
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
        "hgl_types::EngineTime::from_micros(now.micros().checked_add(time.micros()).ok_or_else(|| hgl_types::NodeError::new(\"generator target time overflow\"))?)"
    } else {
        assert_eq!(time.ty, Ty::DateTime, "checked generator time operand");
        "time"
    };
    let publish = hgl_rust_deltas::publish(&payload.ty, "payload");
    format!(
        "let time = {};\nlet payload = {};\nlet now = _ctx.evaluation_time();\nlet target = {resolution};\nif target < now {{ self.generator_pc = {next}; continue; }}\nif target == now {{\nif generator_published {{ return Err(hgl_types::NodeError::new(\"generator duplicate publication time\")); }}\n{publish} generator_published = true; self.generator_pc = {next}; continue;\n}}\nlet delay = target.micros().checked_sub(now.micros()).ok_or_else(|| hgl_types::NodeError::new(\"generator target time difference overflow\"))?;\n_ctx.alarm_in(hgl_types::EngineDelta::from_micros(delay))?;\nself.generator_pending = Some(payload); self.generator_pc = {next}; return Ok(());\n",
        condition_code(plan, time),
        condition_code(plan, payload)
    )
}
