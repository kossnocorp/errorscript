use super::*;

/// Snapshot-local lexical facts, independent of summaries and argument values.
/// Only owned symbol IDs cross worker boundaries.
pub(super) struct BindingFacts {
    symbols: Vec<SymbolId>,
    flags: Vec<bool>,
    mutated: Vec<bool>,
}

impl BindingFacts {
    pub(super) fn new(semantic: &Semantic<'_>) -> Self {
        let owner = |mut node| loop {
            match semantic.nodes().kind(node) {
                AstKind::Function(_)
                | AstKind::ArrowFunctionExpression(_)
                | AstKind::Program(_) => break node,
                _ => node = semantic.nodes().parent_id(node),
            }
        };
        let mut result = Self {
            symbols: Vec::new(),
            flags: vec![false; semantic.scoping().symbols_len()],
            mutated: vec![false; semantic.scoping().symbols_len()],
        };
        for symbol in semantic.scoping().symbol_ids() {
            let mut writes = semantic
                .scoping()
                .get_resolved_references(symbol)
                .filter(|reference| reference.is_write())
                .peekable();
            if writes.peek().is_none() {
                continue;
            }
            result.mutated[symbol.index()] = semantic.scoping().symbol_is_mutated(symbol);
            let declaration_owner = owner(semantic.scoping().symbol_declaration(symbol));
            if writes.any(|reference| owner(reference.node_id()) != declaration_owner) {
                result.symbols.push(symbol);
                result.flags[symbol.index()] = true;
            }
        }
        result
    }

    pub(super) fn is_mutated(&self, symbol: SymbolId) -> bool {
        self.mutated[symbol.index()]
    }

    pub(super) fn widen(&self, env: &mut Env) {
        let needs_widening = |value: &Value| {
            !value.nonglobal || value.truth != 3 || !value.types.contains(&EscErrorType::UNKNOWN)
        };
        // Small functions in a large bundle have fewer bindings than the module
        // has captures; large functions often have very few captured bindings.
        let mutated = if env.len() < self.symbols.len() {
            env.iter()
                .filter(|(symbol, value)| self.flags[symbol.index()] && needs_widening(value))
                .map(|(symbol, _)| *symbol)
                .collect::<Vec<_>>()
        } else {
            self.symbols
                .iter()
                .copied()
                .filter(|symbol| env.get(symbol).is_some_and(needs_widening))
                .collect()
        };
        for symbol in mutated {
            let value = env.get_mut(&symbol).unwrap();
            value.types.insert(EscErrorType::UNKNOWN);
            value.nonglobal = true;
            value.truth = 3;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_nested_writes_and_preserves_unaffected_bindings() {
        let module = EscModule::from_analysis_source(
            "function outer() { let captured = 0; let local = 0; function nested() { captured++; let own = 0; own++; } local++; const arrow = () => { captured = 2; }; }",
            oxc_span::SourceType::default(),
        );
        module.with_semantic(|data| {
            let writes = BindingFacts::new(&data.semantic);
            let names = writes
                .symbols
                .iter()
                .map(|id| data.semantic.scoping().symbol_name(*id))
                .collect::<Vec<_>>();
            assert_eq!(names, ["captured"]);
            let mut env = Env::new();
            for symbol in data.semantic.scoping().symbol_ids() {
                assert_eq!(
                    writes.is_mutated(symbol),
                    data.semantic.scoping().symbol_is_mutated(symbol)
                );
                env.insert(symbol, Value::builtin("number"));
            }
            let original = env.clone();
            writes.widen(&mut env);
            for symbol in data.semantic.scoping().symbol_ids() {
                assert_eq!(
                    env[&symbol].types.contains(&EscErrorType::UNKNOWN),
                    writes.flags[symbol.index()]
                );
                assert!(!original[&symbol].types.contains(&EscErrorType::UNKNOWN));
            }
            let widened = env.clone();
            writes.widen(&mut env);
            assert!(env.ptr_eq(&widened));
            // Exercise the environment-first path with many module captures
            // and only one currently bound symbol.
            let all = BindingFacts {
                symbols: data.semantic.scoping().symbol_ids().collect(),
                flags: vec![true; data.semantic.scoping().symbols_len()],
                mutated: Vec::new(),
            };
            let mut sparse = Env::new();
            sparse.insert(writes.symbols[0], Value::boolean(false));
            all.widen(&mut sparse);
            assert_eq!(sparse.len(), 1);
            assert_eq!(sparse[&writes.symbols[0]].truth, 3);
            assert!(
                sparse[&writes.symbols[0]]
                    .types
                    .contains(&EscErrorType::UNKNOWN)
            );
        });
    }
}
