//! Finite, context-insensitive may-throw analysis. Each SCC is solved jointly,
//! retaining separate function summaries. Structured completion records preserve
//! return/throw/break/continue through catch and finally. Unsupported runtime
//! operations contribute `unknown`, rather than an empty error summary.

use crate::prelude::*;
use oxc_ast::{AstKind, ast::*};
use oxc_semantic::Semantic;
use oxc_syntax::{node::NodeId, symbol::SymbolId};
use std::cell::RefCell;

mod arguments;
mod expression;
mod statement;
#[cfg(test)]
mod tests;
mod types;

type Types = HashSet<EscErrorType>;
type Env = HashMap<SymbolId, Value>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Value {
    types: Types,
    /// Deferred failures of a promise or iterator, not synchronous call errors.
    deferred: Types,
    awaited: Types,
    /// Array element identities stay shallow so recursive interfaces remain finite.
    elements: Types,
    globals: HashSet<&'static str>,
    nonglobal: bool,
    /// Bit 0: may be falsy; bit 1: may be truthy.
    truth: u8,
}

impl Value {
    fn types(types: Types) -> Self {
        Self {
            types,
            nonglobal: true,
            truth: 3,
            ..Self::default()
        }
    }
    fn builtin(name: &'static str) -> Self {
        Self::types(HashSet::from([EscErrorType::builtin(name)]))
    }
    fn unknown() -> Self {
        Self::types(HashSet::from([EscErrorType::UNKNOWN]))
    }
    fn global(global: &EscGlobal) -> Self {
        if !matches!(global.value_type, "Function" | "object") {
            return Self::builtin(global.value_type);
        }
        Self {
            globals: HashSet::from([global.path]),
            nonglobal: false,
            truth: 2,
            ..Self::builtin(global.value_type)
        }
    }
    fn undefined() -> Self {
        Self {
            truth: 1,
            ..Self::builtin("undefined")
        }
    }
    fn boolean(value: bool) -> Self {
        Self {
            truth: if value { 2 } else { 1 },
            ..Self::builtin("boolean")
        }
    }
    fn plain_primitive(&self) -> bool {
        self.types.iter().all(|ty| matches!(ty, EscErrorType::Buildin(name) if matches!(name.as_str(), "number" | "string" | "boolean" | "undefined" | "null")))
    }
    fn join(&mut self, other: Self) {
        self.types.extend(other.types);
        self.deferred.extend(other.deferred);
        self.awaited.extend(other.awaited);
        self.elements.extend(other.elements);
        self.globals.extend(other.globals);
        self.nonglobal |= other.nonglobal;
        self.truth |= other.truth;
    }
}

fn join_env(env: &mut Env, other: Env) {
    for (id, value) in env.iter_mut() {
        if !other.contains_key(id) {
            value.join(Value::unknown());
        }
    }
    for (id, value) in other {
        env.entry(id).or_insert_with(Value::unknown).join(value);
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Completion {
    Normal,
    Return,
    Throw,
    Break(Option<String>),
    Continue(Option<String>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    env: Env,
    value: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Flow(HashMap<Completion, State>);

impl Flow {
    fn one(kind: Completion, env: Env, value: Value) -> Self {
        Self(HashMap::from([(kind, State { env, value })]))
    }
    fn normal(env: Env, value: Value) -> Self {
        Self::one(Completion::Normal, env, value)
    }
    fn add(&mut self, kind: Completion, state: State) {
        if let Some(current) = self.0.get_mut(&kind) {
            join_env(&mut current.env, state.env);
            current.value.join(state.value);
        } else {
            self.0.insert(kind, state);
        }
    }
    fn join(&mut self, other: Self) {
        for (kind, state) in other.0 {
            self.add(kind, state);
        }
    }
    fn then(mut self, next: impl FnOnce(State) -> Self) -> Self {
        if let Some(state) = self.0.remove(&Completion::Normal) {
            self.join(next(state));
        }
        self
    }
    fn value(self, value: Value) -> Self {
        self.then(|state| Self::normal(state.env, value))
    }
    fn possible_throw(mut self, env: &Env, types: Types) -> Self {
        if !types.is_empty() {
            self.add(
                Completion::Throw,
                State {
                    env: env.clone(),
                    value: Value::types(types),
                },
            );
        }
        self
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Summary {
    errors: Types,
    returned: Value,
    completes: bool,
}

impl Summary {
    fn join(&mut self, other: Self) {
        self.errors.extend(other.errors);
        self.returned.join(other.returned);
        self.completes |= other.completes;
    }
}

pub(crate) fn resolve_errors(
    parsed: &EscProjectStateParsed,
    graph: &EscCallGraph,
) -> HashMap<EscFnId, Types> {
    let mut summaries = graph
        .sccs
        .iter()
        .flatten()
        .map(|id| (id.clone(), Summary::default()))
        .collect::<HashMap<_, _>>();
    let calls = graph
        .calls
        .iter()
        .map(|call| ((call.site.module_id.clone(), call.site.node_id), call))
        .collect::<HashMap<_, _>>();
    let arguments = arguments::Arguments::new(parsed, graph);

    // The outer worklist also covers value dependencies through captured/global
    // initializers, which are not necessarily edges in the direct call graph.
    loop {
        let mut changed = false;
        let previous_arguments = arguments.snapshot();
        for component in &graph.sccs {
            loop {
                let mut component_changed = false;
                for id in component {
                    let function = &graph.graph[id.node()];
                    let next = parsed.parsed_files[&function.module_id].with_semantic(|result| {
                        Analyzer {
                            modules: &parsed.parsed_files,
                            module: &function.module_id,
                            semantic: &result.semantic,
                            graph,
                            calls: &calls,
                            summaries: &summaries,
                            reading: RefCell::new(HashSet::new()),
                            arguments: &arguments,
                        }
                        .function(function, id)
                    });
                    let summary = summaries.get_mut(id).unwrap();
                    let previous = summary.clone();
                    summary.join(next);
                    component_changed |= *summary != previous;
                }
                changed |= component_changed;
                if !component_changed {
                    break;
                }
            }
        }
        if !changed && arguments.snapshot() == previous_arguments {
            break;
        }
    }
    summaries
        .into_iter()
        .map(|(id, summary)| (id, summary.errors))
        .collect()
}

struct Analyzer<'s, 'a> {
    modules: &'s HashMap<EscModuleId, EscModule>,
    module: &'s EscModuleId,
    semantic: &'s Semantic<'a>,
    graph: &'s EscCallGraph,
    calls: &'s HashMap<(EscModuleId, NodeId), &'s EscCall>,
    summaries: &'s HashMap<EscFnId, Summary>,
    reading: RefCell<HashSet<SymbolId>>,
    arguments: &'s arguments::Arguments,
}

impl Analyzer<'_, '_> {
    fn function(&self, function: &EscFunction, id: &EscFnId) -> Summary {
        let incoming = match self.arguments.parameters(id) {
            Some(None) => return Summary::default(),
            Some(Some(values)) => Some(values),
            None => None,
        };
        let (params, body, expression) = match self.semantic.nodes().kind(function.node_id) {
            AstKind::Function(f) => (
                &f.params,
                f.body.as_ref().map(|body| body.statements.as_slice()),
                None,
            ),
            AstKind::ArrowFunctionExpression(f) => match &f.body {
                ArrowFunctionBody::FunctionBody(body) => {
                    (&f.params, Some(body.statements.as_slice()), None)
                }
                body => (&f.params, None, body.as_expression()),
            },
            _ => unreachable!(),
        };
        let mut flow = Flow::normal(Env::new(), Value::undefined());
        let declared = self.declared_parameters(id, params.items.len());
        for (index, param) in params.items.iter().enumerate() {
            flow = flow.then(|state| {
                let mut value = param.type_annotation.as_ref().map_or_else(
                    || {
                        declared
                            .as_ref()
                            .or(incoming.as_ref())
                            .map_or_else(Value::unknown, |values| values[index].clone())
                    },
                    |annotation| self.annotation(&annotation.type_annotation),
                );
                if param.optional {
                    value.join(Value::undefined());
                }
                let mut values = Flow::default();
                if let Some(initializer) = &param.initializer {
                    let defaulted = value.types.remove(&EscErrorType::builtin("undefined"))
                        || value.types.contains(&EscErrorType::UNKNOWN);
                    if !value.types.is_empty() {
                        values.join(Flow::normal(state.env.clone(), value));
                    }
                    if defaulted {
                        values.join(self.expr(initializer, state.env));
                    }
                } else {
                    values = Flow::normal(state.env, value);
                }
                values.then(|state| self.bind(&param.pattern, state.env, state.value))
            });
        }
        if let Some(rest) = &params.rest {
            flow = flow
                .then(|state| self.bind(&rest.rest.argument, state.env, Value::builtin("Array")));
        }
        flow = flow.then(|state| {
            if let Some(body) = body {
                self.statements(body, state.env)
            } else if let Some(expr) = expression {
                self.expr(expr, state.env)
                    .then(|state| Flow::one(Completion::Return, state.env, state.value))
            } else {
                Flow::normal(state.env, Value::undefined())
            }
        });
        let mut summary = Summary::default();
        for (kind, state) in flow.0 {
            match kind {
                Completion::Throw => summary.errors.extend(state.value.types),
                Completion::Normal | Completion::Return => {
                    summary.completes = true;
                    let mut value = if kind == Completion::Normal {
                        Value::undefined()
                    } else {
                        state.value
                    };
                    if function.is_async {
                        summary.errors.extend(value.deferred.drain());
                        if value.types.remove(&EscErrorType::builtin("Promise")) {
                            value.types.extend(value.awaited.drain());
                        }
                        if value.types.contains(&EscErrorType::UNKNOWN) {
                            summary.errors.insert(EscErrorType::UNKNOWN);
                        }
                    }
                    summary.returned.join(value);
                }
                _ => {
                    summary.errors.insert(EscErrorType::UNKNOWN);
                }
            }
        }
        summary
    }

    fn symbol(&self, id: &IdentifierReference<'_>) -> Option<SymbolId> {
        id.reference_id
            .get()
            .and_then(|id| self.semantic.scoping().get_reference(id).symbol_id())
    }

    fn captured_mutation(&self, symbol: SymbolId) -> bool {
        let owner = |mut node| loop {
            match self.semantic.nodes().kind(node) {
                AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::Program(_) => break node,
                _ => node = self.semantic.nodes().parent_id(node),
            }
        };
        let declaration_owner = owner(self.semantic.scoping().symbol_declaration(symbol));
        self.semantic
            .scoping()
            .get_resolved_references(symbol)
            .any(|reference| {
                reference.is_write() && owner(reference.node_id()) != declaration_owner
            })
    }

    fn read(&self, id: &IdentifierReference<'_>, env: Env) -> Flow {
        let Some(symbol) = self.symbol(id) else {
            if let Some(global) = self.graph.global(id.name.as_str()) {
                return Flow::normal(env.clone(), Value::global(global))
                    .possible_throw(&env, global.read_errors.iter().cloned().collect());
            }
            if id.name == "undefined" {
                return Flow::normal(env, Value::undefined());
            }
            if matches!(id.name.as_str(), "NaN" | "Infinity") {
                return Flow::normal(env, Value::builtin("number"));
            }
            // An unresolved reference may be supplied by the host, but if it
            // isn't present, evaluating it throws before any coercion or call.
            let flow = Flow::normal(env.clone(), Value::unknown());
            if EscGlobals::get(id.name.as_str()).is_some() {
                return flow;
            }
            return flow.possible_throw(
                &env,
                HashSet::from([EscErrorType::builtin("ReferenceError")]),
            );
        };
        if let Some(value) = env.get(&symbol).cloned() {
            return Flow::normal(env, value);
        }
        if !self.reading.borrow_mut().insert(symbol) {
            return Flow::normal(env, Value::unknown());
        }
        let declaration = self
            .semantic
            .nodes()
            .kind(self.semantic.scoping().symbol_declaration(symbol));
        let value = if let AstKind::VariableDeclarator(declaration) = declaration {
            declaration
                .init
                .as_ref()
                .map_or(Some(Value::undefined()), |expr| {
                    self.expr(expr, env.clone())
                        .0
                        .remove(&Completion::Normal)
                        .map(|state| state.value)
                })
        } else {
            Some(Value::unknown())
        };
        self.reading.borrow_mut().remove(&symbol);
        value.map_or_else(Flow::default, |mut value| {
            if self.semantic.scoping().symbol_is_mutated(symbol) {
                value.join(Value::unknown());
            }
            Flow::normal(env, value)
        })
    }

    fn subtype(
        &self,
        ty: &EscErrorType,
        base: &EscErrorType,
        seen: &mut HashSet<EscErrorType>,
    ) -> Option<bool> {
        if *ty == EscErrorType::UNKNOWN || *base == EscErrorType::UNKNOWN {
            return None;
        }
        if ty == base {
            return Some(true);
        }
        if !seen.insert(ty.clone()) {
            return None;
        }
        let answer = if let Some(bases) = self.graph.super_types.get(ty) {
            let answers = bases
                .iter()
                .map(|ty| self.subtype(ty, base, seen))
                .collect::<Vec<_>>();
            if answers.contains(&Some(true)) {
                Some(true)
            } else if answers.contains(&None) {
                None
            } else {
                Some(false)
            }
        } else {
            Some(
                *base == EscErrorType::builtin("Error")
                    && matches!(ty, EscErrorType::Buildin(name) if EscErrorType::constructor(name.as_str()).is_some()),
            )
        };
        seen.remove(ty);
        answer
    }
}
