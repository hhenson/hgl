use crate::lex::error;
use crate::model::{
    Expression, ExpressionKind, Function, Guard, ParsedStatement, Statement, Value, ValueKind,
};
use crate::{CheckedModule, Diagnostic, Source, parse};
use std::collections::{BTreeMap, BTreeSet};

/// Parse and resolve the admitted subset without running user code.
/// Errors include unsupported syntax, invalid calls and unmatched selected parts.
pub fn check(sources: &[Source]) -> Result<CheckedModule, Vec<Diagnostic>> {
    checked(sources).map_err(|issue| vec![issue])
}
fn checked(sources: &[Source]) -> Result<CheckedModule, Diagnostic> {
    let Some(first) = sources.first() else {
        return Err(Diagnostic {
            source: String::new(),
            start: 0,
            end: 0,
            message: "no source files".into(),
        });
    };
    let Declarations {
        module,
        functions,
        names,
    } = declarations(sources)?;
    let mut bodies = Vec::new();
    let mut calls = Vec::new();
    for function in &functions {
        let source = &sources[function.source];
        if function.native {
            bodies.push(Vec::new());
            calls.push(Vec::new());
            continue;
        }
        let Some(body) = &function.body else {
            continue;
        };
        if (body.start || body.guard == Some(Guard::Scheduled)) && !body.scheduler {
            return Err(error(
                source,
                function.span.clone(),
                "scheduler access requires inject scheduler in this slice",
            ));
        }
        if body.guard.is_none() && (body.start || body.scheduler) {
            return Err(error(
                source,
                function.span.clone(),
                "lifecycle and scheduler require a node handler",
            ));
        }
        let mut context = Context {
            source,
            functions: &functions,
            names: &names,
            function,
            node: body.guard.is_some(),
            locals: BTreeMap::new(),
            calls: Vec::new(),
        };
        let statements = context.statements(&body.statements)?;
        bodies.push(statements);
        calls.push(context.calls);
    }
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    for index in 0..functions.len() {
        if !acyclic(index, &calls, &mut active, &mut done) {
            return Err(error(
                first,
                0..0,
                "recursive construction is not supported",
            ));
        }
    }
    Ok(CheckedModule {
        name: module,
        sources: sources.iter().map(|source| source.name.clone()).collect(),
        functions,
        bodies,
    })
}
struct Declarations {
    module: String,
    functions: Vec<Function>,
    names: BTreeMap<String, usize>,
}
fn declarations(sources: &[Source]) -> Result<Declarations, Diagnostic> {
    let mut module = String::new();
    let mut functions: Vec<Function> = Vec::new();
    let mut implementations = Vec::new();
    let mut parts = BTreeSet::new();
    for (id, source) in sources.iter().enumerate() {
        let parsed = parse::parse(source, id)?;
        if id == 0 {
            module.clone_from(&parsed.module);
        }
        if module != parsed.module {
            return Err(error(
                source,
                0..0,
                "selected parts must belong to the same module",
            ));
        }
        if !parts.insert(parsed.part) {
            return Err(error(source, 0..0, "duplicate module part"));
        }
        for function in parsed.functions {
            if function.native && function.body.is_some() {
                implementations.push(function);
            } else {
                functions.push(function);
            }
        }
    }
    let mut names = BTreeMap::new();
    for (index, function) in functions.iter().enumerate() {
        if names.insert(function.name.clone(), index).is_some() {
            return Err(error(
                &sources[function.source],
                function.span.clone(),
                "duplicate function or unsupported overload",
            ));
        }
        let mut parameters = BTreeSet::new();
        for parameter in &function.parameters {
            if !parameters.insert(&parameter.name) {
                return Err(error(
                    &sources[function.source],
                    function.span.clone(),
                    "duplicate parameter",
                ));
            }
        }
    }
    for implementation in implementations {
        let matching = names.get(&implementation.name).copied().filter(|&index| {
            let declaration = &functions[index];
            declaration.native
                && declaration.parameters == implementation.parameters
                && declaration.result == implementation.result
        });
        let Some(index) = matching else {
            return Err(error(
                &sources[implementation.source],
                implementation.span,
                "implementation does not match a shared native signature",
            ));
        };
        if functions[index].body.is_some() {
            return Err(error(
                &sources[implementation.source],
                implementation.span,
                "duplicate native implementation",
            ));
        }
        functions[index].body = implementation.body;
    }
    Ok(Declarations {
        module,
        functions,
        names,
    })
}

fn acyclic(
    index: usize,
    calls: &[Vec<usize>],
    active: &mut BTreeSet<usize>,
    done: &mut BTreeSet<usize>,
) -> bool {
    if done.contains(&index) {
        return true;
    }
    if !active.insert(index) {
        return false;
    }
    for &callee in &calls[index] {
        if !acyclic(callee, calls, active, done) {
            return false;
        }
    }
    active.remove(&index);
    done.insert(index);
    true
}
struct Context<'a> {
    source: &'a Source,
    functions: &'a [Function],
    names: &'a BTreeMap<String, usize>,
    function: &'a Function,
    node: bool,
    locals: BTreeMap<String, (usize, ValueKind)>,
    calls: Vec<usize>,
}
impl Context<'_> {
    fn statements(&mut self, statements: &[ParsedStatement]) -> Result<Vec<Statement>, Diagnostic> {
        let mut checked = Vec::new();
        let mut returned = false;
        for statement in statements {
            if returned {
                return Err(error(
                    self.source,
                    self.function.span.clone(),
                    "statements after return are outside this slice",
                ));
            }
            let expression = match statement {
                ParsedStatement::Let(_, value)
                | ParsedStatement::Return(value)
                | ParsedStatement::Call(value) => value,
            };
            let (value, kind) = self.expression(expression)?;
            checked.push(match statement {
                ParsedStatement::Let(name, _) => {
                    if kind == ValueKind::Void
                        || self.locals.contains_key(name)
                        || self.function.parameters.iter().any(|p| p.name == *name)
                    {
                        return Err(error(
                            self.source,
                            expression.span.clone(),
                            "local binding must have a value and a unique name",
                        ));
                    }
                    self.locals.insert(name.clone(), (self.locals.len(), kind));
                    Statement::Let(value)
                }
                ParsedStatement::Return(_) => {
                    let expected = if self.node {
                        ValueKind::Scalar
                    } else {
                        ValueKind::Port
                    };
                    if !self.function.result || kind != expected {
                        return Err(error(
                            self.source,
                            expression.span.clone(),
                            "return type or temporal role does not match",
                        ));
                    }
                    returned = true;
                    Statement::Return(value)
                }
                ParsedStatement::Call(_) => {
                    if !matches!(expression.kind, ExpressionKind::Call(..)) {
                        return Err(error(
                            self.source,
                            expression.span.clone(),
                            "an expression statement must be a call",
                        ));
                    }
                    Statement::Call(value)
                }
            });
        }
        if self.function.result && !returned {
            return Err(error(
                self.source,
                self.function.span.clone(),
                "this slice requires a result return",
            ));
        }
        Ok(checked)
    }
    fn expression(&mut self, expression: &Expression) -> Result<(Value, ValueKind), Diagnostic> {
        match &expression.kind {
            ExpressionKind::Integer(value) => Ok((Value::Integer(*value), ValueKind::Scalar)),
            ExpressionKind::Name(name) => {
                if let Some((index, parameter)) = self
                    .function
                    .parameters
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.name == *name)
                {
                    let kind = if self.node || parameter.constant {
                        ValueKind::Scalar
                    } else {
                        ValueKind::Port
                    };
                    return Ok((Value::Parameter(index), kind));
                }
                if let Some(&(index, kind)) = self.locals.get(name) {
                    return Ok((Value::Local(index), kind));
                }
                Err(error(
                    self.source,
                    expression.span.clone(),
                    &format!("unknown value '{name}'"),
                ))
            }
            ExpressionKind::Call(name, arguments) => self.call(expression, name, arguments),
        }
    }
    fn call(
        &mut self,
        expression: &Expression,
        name: &str,
        arguments: &[Expression],
    ) -> Result<(Value, ValueKind), Diagnostic> {
        if self.locals.contains_key(name)
            || self
                .function
                .parameters
                .iter()
                .any(|parameter| parameter.name == name)
        {
            return Err(error(
                self.source,
                expression.span.clone(),
                "call target is shadowed by a value",
            ));
        }
        let Some(&index) = self.names.get(name) else {
            return Err(error(
                self.source,
                expression.span.clone(),
                &format!("unknown function '{name}'"),
            ));
        };
        let target = &self.functions[index];
        if target.native != self.node {
            return Err(error(
                self.source,
                expression.span.clone(),
                "node handlers call native values; graph bodies construct temporal functions in this slice",
            ));
        }
        if target.parameters.len() != arguments.len() {
            return Err(error(
                self.source,
                expression.span.clone(),
                "wrong argument count",
            ));
        }
        let mut values = Vec::new();
        for (parameter, argument) in target.parameters.iter().zip(arguments) {
            let (value, kind) = self.expression(argument)?;
            let expected = if target.native || parameter.constant {
                ValueKind::Scalar
            } else {
                ValueKind::Port
            };
            if kind != expected {
                return Err(error(
                    self.source,
                    argument.span.clone(),
                    "argument type or scalar/temporal role does not match",
                ));
            }
            values.push(value);
        }
        self.calls.push(index);
        let kind = if !target.result {
            ValueKind::Void
        } else if target.native {
            ValueKind::Scalar
        } else {
            ValueKind::Port
        };
        Ok((Value::Call(index, values), kind))
    }
}
