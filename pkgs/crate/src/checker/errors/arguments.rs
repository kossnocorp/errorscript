use super::*;

/// Only close over source functions whose binding is used exclusively by
/// resolved local direct calls. Exported/escaped/mutated functions retain their
/// open-world parameter types.
fn local_parameters(
    parsed: &EscProjectStateParsed,
    graph: &EscCallGraph,
) -> HashMap<EscFnId, usize> {
    let mut result = HashMap::new();
    for id in graph.sccs.iter().flatten() {
        let function = &graph.graph[id.node()];
        parsed.parsed_files[&function.module_id].with_semantic(|data| {
            let semantic = &data.semantic;
            let AstKind::Function(f) = semantic.nodes().kind(function.node_id) else { return; };
            if !matches!(semantic.nodes().parent_kind(function.node_id), AstKind::Program(_) | AstKind::FunctionBody(_) | AstKind::BlockStatement(_)) { return; }
            let Some(symbol) = f.id.as_ref().and_then(|id| id.symbol_id.get()) else { return; };
            if semantic.scoping().symbol_is_mutated(symbol) || f.params.rest.is_some() { return; }
            let references = semantic.scoping().get_resolved_references(symbol).collect::<Vec<_>>();
            if references.is_empty() { return; }
            if references.iter().all(|reference| {
                let AstKind::CallExpression(call) = semantic.nodes().parent_kind(reference.node_id()) else { return false; };
                if !matches!(&call.callee, Expression::Identifier(callee) if callee.node_id.get() == reference.node_id()) { return false; }
                graph.calls.iter().any(|linked| linked.site.module_id == function.module_id
                    && linked.site.node_id == call.node_id.get() && !linked.unresolved
                    && linked.targets == [id.clone()] && linked.caller.is_some())
            }) { result.insert(id.clone(), f.params.items.len()); }
        });
    }
    // A recursive component without a source caller still needs open-world
    // entry values, just like an otherwise uncalled function.
    for component in &graph.sccs {
        if !graph.calls.iter().any(|call| {
            call.caller
                .as_ref()
                .is_some_and(|caller| !component.contains(caller))
                && call.targets.iter().any(|target| component.contains(target))
        }) {
            for id in component {
                result.remove(id);
            }
        }
    }
    result
}

#[derive(Default)]
pub(super) struct Arguments {
    counts: HashMap<EscFnId, usize>,
    values: RefCell<HashMap<EscFnId, Vec<Value>>>,
}

impl Arguments {
    pub(super) fn new(parsed: &EscProjectStateParsed, graph: &EscCallGraph) -> Self {
        Self {
            counts: local_parameters(parsed, graph),
            ..Self::default()
        }
    }

    pub(super) fn parameters(&self, id: &EscFnId) -> Option<Option<Vec<Value>>> {
        self.counts
            .get(id)
            .map(|_| self.values.borrow().get(id).cloned())
    }

    pub(super) fn snapshot(&self) -> HashMap<EscFnId, Vec<Value>> {
        self.values.borrow().clone()
    }

    pub(super) fn record(&self, call: &EscCall, arguments: &[Value], spread: bool) {
        for target in &call.targets {
            let Some(&count) = self.counts.get(target) else {
                continue;
            };
            let mut values = self.values.borrow_mut();
            let parameters = values
                .entry(target.clone())
                .or_insert_with(|| vec![Value::default(); count]);
            for (index, parameter) in parameters.iter_mut().enumerate() {
                parameter.join(if spread {
                    Value::unknown()
                } else {
                    arguments
                        .get(index)
                        .cloned()
                        .unwrap_or_else(Value::undefined)
                });
            }
        }
    }
}
