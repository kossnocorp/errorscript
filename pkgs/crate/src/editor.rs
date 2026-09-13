//! Persistent editor snapshots. ASTs and reports outside the affected module
//! component survive edits. Edges are traversed both ways because argument
//! inference carries information from callers into callees.
use crate::prelude::*;
use oxc_ast::AstKind;
use oxc_span::{GetSpan, Span};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug)]
pub struct EscCallReport {
    pub start: u32,
    pub end: u32,
    pub errors: Vec<String>,
    pub uncaught: Vec<String>,
}

#[derive(Default)]
pub struct EscEditor {
    modules: HashMap<EscModulePath, EscModule>,
    pub reports: HashMap<EscModulePath, Vec<EscCallReport>>,
    shared_globals: bool,
}

impl EscEditor {
    pub fn source(&self, path: &EscModulePath) -> Option<&str> {
        self.modules.get(path).map(EscModule::source)
    }

    /// Refresh configured roots, open documents and their transitive imports.
    /// Open buffers always take precedence over disk contents.
    pub async fn update(
        &mut self,
        root: &PathBuf,
        overlays: &HashMap<EscModulePath, String>,
    ) -> Result<HashSet<EscModulePath>> {
        let mut project = EscProject::resolve(Some(root)).await?;
        let (roots, excluded) = project.files_with_exclusions().await?;
        let mut pending = roots.into_iter().collect::<Vec<_>>();
        pending.extend(overlays.keys().cloned());
        let mut seen = HashSet::new();
        let mut changed = HashSet::new();
        let mut edges = Vec::new();
        for (path, module) in &self.modules {
            edges.extend(
                module
                    .extract_dependencies()
                    .into_iter()
                    .map(|dep| (path.clone(), dep)),
            );
        }
        while let Some(path) = pending.pop() {
            if excluded.contains(&path) || seen.contains(&path) {
                continue;
            }
            let source = match overlays.get(&path) {
                Some(source) => source.clone(),
                None => match std::fs::read_to_string(&path) {
                    Ok(source) => source,
                    Err(_) => {
                        changed.insert(path.clone());
                        continue;
                    }
                },
            };
            seen.insert(path.clone());
            if self
                .modules
                .get(&path)
                .is_none_or(|module| module.source() != source)
            {
                let module = EscModule::parse(source, &path, &project.resolver)?;
                self.modules.insert(path.clone(), module);
                changed.insert(path.clone());
            } else {
                // Refresh resolution against the current filesystem/configuration
                // while retaining the unchanged semantic arena.
                let module = self.modules.get_mut(&path).unwrap();
                let references = module.with_program(|program| {
                    EscModuleReferences::collect_with_module_syntax(
                        program,
                        module.has_module_syntax,
                        &path,
                        &project.resolver,
                    )
                });
                if module.extract_dependencies()
                    != references.dependencies().cloned().collect::<Vec<_>>()
                {
                    changed.insert(path.clone());
                }
                module.references = references;
            }
            let dependencies = self.modules[&path].extract_dependencies();
            edges.extend(dependencies.iter().cloned().map(|dep| (path.clone(), dep)));
            pending.extend(dependencies);
        }
        changed.extend(
            self.modules
                .keys()
                .filter(|path| !seen.contains(*path))
                .cloned(),
        );
        changed.extend(
            self.modules
                .keys()
                .filter(|path| !self.reports.contains_key(*path))
                .cloned(),
        );
        if !changed.is_empty() {
            // Global mutations can affect modules with no import edge. The
            // linker detects aliases as well as direct global assignments.
            let mut paths = HashMap::new();
            let mut parsed_files = HashMap::new();
            for (path, module) in std::mem::take(&mut self.modules) {
                let id = project.module_id(&path)?;
                paths.insert(id.clone(), path);
                parsed_files.insert(id, module);
            }
            let parsed = EscProjectStateParsed { parsed_files };
            let graph = EscCallGraph::build(&parsed, &project.repo_path);
            for (id, module) in parsed.parsed_files {
                self.modules.insert(paths[&id].clone(), module);
            }
            let shared_globals = !graph?.modified_globals.is_empty();
            if shared_globals || self.shared_globals {
                changed.extend(self.modules.keys().cloned());
            }
            self.shared_globals = shared_globals;
        }
        let mut adjacent: HashMap<EscModulePath, Vec<EscModulePath>> = HashMap::new();
        for (a, b) in edges {
            adjacent.entry(a.clone()).or_default().push(b.clone());
            adjacent.entry(b).or_default().push(a);
        }
        let mut pending = changed.iter().cloned().collect::<Vec<_>>();
        while let Some(path) = pending.pop() {
            for dependent in adjacent.get(&path).into_iter().flatten() {
                if changed.insert(dependent.clone()) {
                    pending.push(dependent.clone());
                }
            }
        }
        self.modules.retain(|path, _| seen.contains(path));
        if changed.is_empty() {
            return Ok(changed);
        }
        let mut paths = HashMap::new();
        let mut parsed_files = HashMap::new();
        for path in &changed {
            self.reports.remove(path);
            if let Some(module) = self.modules.remove(path) {
                let id = project.module_id(path)?;
                paths.insert(id.clone(), path.clone());
                parsed_files.insert(id, module);
            }
        }
        project.state = EscProjectState::Parsed(EscProjectStateParsed { parsed_files });
        let checked = project.check_files().await;
        match project.state {
            EscProjectState::Checked(state) => {
                for (id, path) in &paths {
                    self.reports.insert(path.clone(), reports(&state, id));
                }
                for (id, module) in state.checked_files {
                    self.modules.insert(paths[&id].clone(), module);
                }
            }
            EscProjectState::Parsed(state) => {
                for (id, module) in state.parsed_files {
                    self.modules.insert(paths[&id].clone(), module);
                }
            }
            _ => {}
        }
        checked?;
        Ok(changed)
    }
}

fn error_name(state: &EscProjectStateChecked, error: &EscErrorType) -> String {
    match error {
        EscErrorType::Buildin(name) => name.to_string(),
        EscErrorType::Node(id) => state
            .checked_files
            .get(&id.module_id)
            .map(|module| {
                module.with_semantic(|result| match result.semantic.nodes().kind(id.node) {
                    AstKind::Class(class) => class
                        .id
                        .as_ref()
                        .map(|id| id.name.to_string())
                        .unwrap_or_else(|| "anonymous class".into()),
                    AstKind::TSInterfaceDeclaration(interface) => interface.id.name.to_string(),
                    AstKind::TSTypeAliasDeclaration(alias) => alias.id.name.to_string(),
                    AstKind::TSTypeLiteral(_) => "object".into(),
                    _ => "unknown".into(),
                })
            })
            .unwrap_or_else(|| "unknown".into()),
    }
}

fn reports(state: &EscProjectStateChecked, module: &EscModuleId) -> Vec<EscCallReport> {
    state.checked_files[module].with_semantic(|result| {
        let nodes = result.semantic.nodes();
        state
            .call_graph
            .calls
            .iter()
            .filter(|call| &call.site.module_id == module)
            .filter_map(|call| {
                let span = match nodes.kind(call.site.node_id) {
                    AstKind::CallExpression(expression) => expression.span,
                    AstKind::NewExpression(expression) => expression.span,
                    _ => return None,
                };
                let effects = state
                    .call_errors
                    .get(&(module.clone(), call.site.node_id))?;
                let sync = &effects.sync;
                let deferred = &effects.deferred;
                let mut caught = false;
                let mut awaited = false;
                let mut rejection_handler = false;
                let mut promise_path = true;
                for kind in nodes.ancestor_kinds(call.site.node_id) {
                    match kind {
                        AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => break,
                        AstKind::AwaitExpression(_) => {
                            awaited |= promise_path;
                            promise_path = false;
                        }
                        AstKind::CallExpression(parent) => {
                            let mut chained = false;
                            if let Some(member) =
                                parent.callee.get_inner_expression().as_member_expression()
                            {
                                let object = member.object().span();
                                if promise_path && contains(object, span) {
                                    chained = matches!(
                                        member.static_property_name(),
                                        Some("catch" | "then" | "finally")
                                    );
                                    rejection_handler |= match member.static_property_name() {
                                        Some("catch") => {
                                            parent.arguments.first().is_some_and(|argument| {
                                                handler(state, module, argument)
                                            })
                                        }
                                        Some("then") => {
                                            parent.arguments.get(1).is_some_and(|argument| {
                                                handler(state, module, argument)
                                            })
                                        }
                                        _ => false,
                                    };
                                }
                            }
                            promise_path &= chained;
                        }
                        AstKind::TryStatement(statement)
                            if statement.handler.is_some()
                                && contains(statement.block.span, span) =>
                        {
                            caught = true;
                        }
                        _ => {}
                    }
                }
                let mut errors = sync
                    .iter()
                    .chain(deferred)
                    .map(|error| error_name(state, error))
                    .collect::<Vec<_>>();
                errors.sort();
                errors.dedup();
                let mut uncaught = Vec::new();
                if call.caller.is_none() {
                    if !caught {
                        uncaught.extend(sync.iter().map(|error| error_name(state, error)));
                    }
                    if !rejection_handler
                        && !(awaited && caught)
                        && !bound_promise_handled(
                            state,
                            module,
                            &result.semantic,
                            call.site.node_id,
                            span,
                        )
                    {
                        uncaught.extend(deferred.iter().map(|error| error_name(state, error)));
                    }
                }
                uncaught.sort();
                uncaught.dedup();
                Some(EscCallReport {
                    start: span.start,
                    end: span.end,
                    errors,
                    uncaught,
                })
            })
            .collect()
    })
}

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

fn handler(
    state: &EscProjectStateChecked,
    module: &EscModuleId,
    argument: &oxc_ast::ast::Argument<'_>,
) -> bool {
    argument
        .as_expression()
        .and_then(|expression| {
            state
                .call_graph
                .callbacks
                .get(&(module.clone(), expression.span()))
        })
        .is_some_and(|callback| {
            !callback.unresolved
                && callback.globals.iter().all(|path| {
                    state
                        .call_graph
                        .global(path)
                        .is_some_and(|global| global.call.is_some())
                })
        })
}

/// A promise kept in an immutable local can be handled at its use site. Every
/// reference must be a recognized handled use; escapes remain conservative.
fn bound_promise_handled(
    state: &EscProjectStateChecked,
    module: &EscModuleId,
    semantic: &oxc_semantic::Semantic<'_>,
    node: oxc_syntax::node::NodeId,
    span: Span,
) -> bool {
    let nodes = semantic.nodes();
    let AstKind::VariableDeclarator(declaration) = nodes.parent_kind(node) else {
        return false;
    };
    if declaration
        .init
        .as_ref()
        .is_none_or(|expression| expression.span() != span)
    {
        return false;
    }
    let Some(symbol) = declaration
        .id
        .get_binding_identifier()
        .and_then(|id| id.symbol_id.get())
    else {
        return false;
    };
    if semantic.scoping().symbol_is_mutated(symbol) {
        return false;
    }
    let references = semantic
        .scoping()
        .get_resolved_references(symbol)
        .collect::<Vec<_>>();
    !references.is_empty()
        && references.iter().all(|reference| {
            if nodes.ancestor_kinds(reference.node_id()).any(|kind| {
                matches!(
                    kind,
                    AstKind::Function(_)
                        | AstKind::ArrowFunctionExpression(_)
                        | AstKind::IfStatement(_)
                        | AstKind::ConditionalExpression(_)
                        | AstKind::ForStatement(_)
                        | AstKind::ForInStatement(_)
                        | AstKind::ForOfStatement(_)
                        | AstKind::WhileStatement(_)
                        | AstKind::DoWhileStatement(_)
                        | AstKind::SwitchStatement(_)
                )
            }) {
                return false;
            }
            let reference_span = nodes.kind(reference.node_id()).span();
            if reference_span.start < span.end {
                return false;
            }
            let mut awaited = false;
            for kind in nodes.ancestor_kinds(reference.node_id()) {
                match kind {
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_) => return false,
                    AstKind::AwaitExpression(_) => awaited = true,
                    AstKind::CallExpression(call) if !awaited => {
                        let Some(member) =
                            call.callee.get_inner_expression().as_member_expression()
                        else {
                            return false;
                        };
                        if !contains(member.object().span(), reference_span) {
                            return false;
                        }
                        match member.static_property_name() {
                            Some("catch")
                                if call
                                    .arguments
                                    .first()
                                    .is_some_and(|argument| handler(state, module, argument)) =>
                            {
                                return true;
                            }
                            Some("then")
                                if call
                                    .arguments
                                    .get(1)
                                    .is_some_and(|argument| handler(state, module, argument)) =>
                            {
                                return true;
                            }
                            _ => return false,
                        }
                    }
                    AstKind::TryStatement(statement)
                        if awaited
                            && statement.handler.is_some()
                            && contains(statement.block.span, reference_span) =>
                    {
                        return true;
                    }
                    _ => {}
                }
            }
            false
        })
}
