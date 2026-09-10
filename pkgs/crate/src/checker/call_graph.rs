//! A flow-insensitive call graph for lexical bindings and ESM imports/exports.
//! Dynamic dispatch, higher-order parameters, and unsupported expressions retain
//! an unresolved target. SCCs describe the resolved edges, not a proof that the
//! complete runtime call graph has no additional cycles.

use crate::prelude::*;

use oxc_ast::{AstKind, ast::*};
use oxc_semantic::Semantic;
use oxc_span::Span;
use oxc_syntax::{node::NodeId, symbol::SymbolId};
use petgraph::{algo::tarjan_scc, graph::NodeIndex};
use std::collections::BTreeSet;

type SymbolKey = (EscModuleId, SymbolId);
type ExportKey = (EscModuleId, String);

#[derive(Clone, Debug)]
enum Value {
    Function(NodeIndex),
    Binding(SymbolKey),
    Export(ExportKey),
    Namespace(EscModuleId),
    Member(Box<Value>, String),
    Union(Vec<Value>),
    Unknown,
}

#[derive(Hash, Eq, PartialEq)]
enum Lookup {
    Binding(SymbolKey),
    Export(ExportKey),
}

struct PendingCall {
    caller: Option<NodeIndex>,
    site: EscCallSite,
    callee: Value,
}

#[derive(Default)]
struct Builder {
    result: EscCallGraph,
    functions: HashMap<(EscModuleId, NodeId), NodeIndex>,
    bindings: HashMap<SymbolKey, Value>,
    exports: HashMap<ExportKey, Value>,
    stars: HashMap<EscModuleId, Vec<Option<EscModuleId>>>,
    calls: Vec<PendingCall>,
}

#[derive(Default)]
struct Targets {
    functions: BTreeSet<NodeIndex>,
    namespaces: BTreeSet<EscModuleId>,
    unknown: bool,
}

impl Targets {
    fn merge(&mut self, other: Self) {
        self.functions.extend(other.functions);
        self.namespaces.extend(other.namespaces);
        self.unknown |= other.unknown;
    }
}

impl EscCallGraph {
    pub(crate) fn build(parsed: &EscProjectStateParsed, repo: &EscRepoPath) -> Result<Self> {
        let mut builder = Builder::default();
        let mut modules = parsed.parsed_files.iter().collect::<Vec<_>>();
        modules.sort_by_key(|(id, _)| *id);

        // Register every body first, including uncalled functions, expressions,
        // arrows and methods. Bodyless TS declarations have no executable body.
        for (module_id, module) in &modules {
            module.with_semantic(|result| {
                for node in result.semantic.nodes().iter() {
                    let name = match node.kind() {
                        AstKind::Function(function) if function.body.is_some() => {
                            function.id.as_ref().map(|id| id.name.to_string())
                        }
                        AstKind::ArrowFunctionExpression(_) => None,
                        _ => continue,
                    };
                    let name =
                        name.or_else(|| match result.semantic.nodes().parent_kind(node.id()) {
                            AstKind::VariableDeclarator(declaration) => declaration
                                .id
                                .get_binding_identifier()
                                .map(|id| id.name.to_string()),
                            _ => None,
                        });
                    let index = builder.result.graph.add_node(EscFunction {
                        module_id: (*module_id).clone(),
                        node_id: node.id(),
                        name,
                    });
                    builder
                        .functions
                        .insert(((*module_id).clone(), node.id()), index);
                }
            });
        }

        for (module_id, module) in modules {
            // Resolve source spans rather than matching a possibly repeated
            // module name with a different import mode.
            let mut sources = HashMap::new();
            for reference in &module.references.imports {
                if let EscModuleReference::External { info, module, .. } = reference
                    && !info.type_only
                {
                    let id = EscModuleId::from_path(module, repo)?;
                    if parsed.parsed_files.contains_key(&id) {
                        sources.insert(info.span, id);
                    }
                }
            }
            module.with_semantic(|result| {
                builder.collect(module_id, &result.semantic, &sources);
            });
        }

        builder.finish();
        Ok(builder.result)
    }
}

impl Builder {
    fn function(&self, module: &EscModuleId, node: NodeId) -> Value {
        self.functions
            .get(&(module.clone(), node))
            .copied()
            .map_or(Value::Unknown, Value::Function)
    }

    fn identifier(
        &self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        id: &IdentifierReference<'_>,
    ) -> Value {
        id.reference_id
            .get()
            .and_then(|reference| semantic.scoping().get_reference(reference).symbol_id())
            .map_or(Value::Unknown, |symbol| {
                Value::Binding((module.clone(), symbol))
            })
    }

    fn expression(
        &self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        expr: &Expression<'_>,
    ) -> Value {
        let expr = expr.get_inner_expression();
        match expr {
            Expression::Identifier(id) => self.identifier(module, semantic, id),
            Expression::FunctionExpression(function) => {
                self.function(module, function.node_id.get())
            }
            Expression::ArrowFunctionExpression(function) => {
                self.function(module, function.node_id.get())
            }
            Expression::ConditionalExpression(expr) => Value::Union(vec![
                self.expression(module, semantic, &expr.consequent),
                self.expression(module, semantic, &expr.alternate),
            ]),
            Expression::LogicalExpression(expr) => Value::Union(vec![
                self.expression(module, semantic, &expr.left),
                self.expression(module, semantic, &expr.right),
            ]),
            Expression::SequenceExpression(expr) => {
                expr.expressions.last().map_or(Value::Unknown, |expr| {
                    self.expression(module, semantic, expr)
                })
            }
            Expression::ChainExpression(expr) => expr
                .expression
                .member_expression()
                .map_or(Value::Unknown, |member| {
                    self.member(module, semantic, member)
                }),
            _ => expr
                .as_member_expression()
                .map_or(Value::Unknown, |member| {
                    self.member(module, semantic, member)
                }),
        }
    }

    fn member(
        &self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        member: &MemberExpression<'_>,
    ) -> Value {
        member
            .static_property_name()
            .map_or(Value::Unknown, |name| {
                Value::Member(
                    Box::new(self.expression(module, semantic, member.object())),
                    name.to_string(),
                )
            })
    }

    fn bind(&mut self, module: &EscModuleId, id: &BindingIdentifier<'_>, value: Value) {
        if let Some(symbol) = id.symbol_id.get() {
            self.bindings.insert((module.clone(), symbol), value);
        }
    }

    fn imported(sources: &HashMap<Span, EscModuleId>, source: Span, name: &str) -> Value {
        sources.get(&source).map_or(Value::Unknown, |module| {
            Value::Export((module.clone(), name.to_string()))
        })
    }

    fn export_binding(&mut self, module: &EscModuleId, id: &BindingIdentifier<'_>) {
        if let Some(symbol) = id.symbol_id.get() {
            self.exports.insert(
                (module.clone(), id.name.to_string()),
                Value::Binding((module.clone(), symbol)),
            );
        }
    }

    fn collect(
        &mut self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        sources: &HashMap<Span, EscModuleId>,
    ) {
        for node in semantic.nodes().iter() {
            // Namespace exports are not ESM exports of the containing file.
            let top_level = matches!(semantic.nodes().parent_kind(node.id()), AstKind::Program(_));
            match node.kind() {
                AstKind::Function(function) => {
                    if let Some(id) = &function.id {
                        self.bind(module, id, self.function(module, node.id()));
                    }
                }
                AstKind::VariableDeclarator(declaration) => {
                    if let Some(id) = declaration.id.get_binding_identifier() {
                        let value = declaration.init.as_ref().map_or(Value::Unknown, |expr| {
                            self.expression(module, semantic, expr)
                        });
                        self.bind(module, id, value);
                    }
                }
                AstKind::ImportDeclaration(import) if !import.import_kind.is_type() => {
                    for specifier in import.specifiers.iter().flatten() {
                        let (local, value) = match specifier {
                            ImportDeclarationSpecifier::ImportSpecifier(specifier)
                                if !specifier.import_kind.is_type() =>
                            {
                                (
                                    &specifier.local,
                                    Self::imported(
                                        sources,
                                        import.source.span,
                                        specifier.imported.name().as_str(),
                                    ),
                                )
                            }
                            ImportDeclarationSpecifier::ImportDefaultSpecifier(specifier) => (
                                &specifier.local,
                                Self::imported(sources, import.source.span, "default"),
                            ),
                            ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => (
                                &specifier.local,
                                sources
                                    .get(&import.source.span)
                                    .cloned()
                                    .map_or(Value::Unknown, Value::Namespace),
                            ),
                            _ => continue,
                        };
                        self.bind(module, local, value);
                    }
                }
                AstKind::ExportDeclaration(export) if top_level => match &export.declaration {
                    Declaration::FunctionDeclaration(function) => {
                        if let Some(id) = &function.id {
                            self.export_binding(module, id);
                        }
                    }
                    Declaration::VariableDeclaration(declaration) => {
                        for declaration in &declaration.declarations {
                            for id in declaration.id.get_binding_identifiers() {
                                self.export_binding(module, id);
                            }
                        }
                    }
                    _ => {}
                },
                AstKind::ExportNamedDeclaration(export)
                    if top_level && !export.export_kind.is_type() =>
                {
                    for specifier in &export.specifiers {
                        if specifier.export_kind.is_type() {
                            continue;
                        }
                        let value = match &specifier.local {
                            ModuleExportName::IdentifierReference(id) => {
                                self.identifier(module, semantic, id)
                            }
                            _ => Value::Unknown,
                        };
                        self.exports.insert(
                            (module.clone(), specifier.exported.name().to_string()),
                            value,
                        );
                    }
                }
                AstKind::ExportDefaultDeclaration(export) if top_level => {
                    let value = match &export.declaration {
                        ExportDefaultDeclarationKind::FunctionDeclaration(function) => {
                            self.function(module, function.node_id.get())
                        }
                        declaration => declaration.as_expression().map_or(Value::Unknown, |expr| {
                            self.expression(module, semantic, expr)
                        }),
                    };
                    self.exports
                        .insert((module.clone(), "default".into()), value);
                }
                AstKind::ExportFromDeclaration(export)
                    if top_level && !export.export_kind.is_type() =>
                {
                    for specifier in &export.specifiers {
                        if specifier.export_kind.is_type() {
                            continue;
                        }
                        self.exports.insert(
                            (module.clone(), specifier.exported.name().to_string()),
                            Self::imported(
                                sources,
                                export.source.span,
                                specifier.local.name().as_str(),
                            ),
                        );
                    }
                }
                AstKind::ExportAllDeclaration(export)
                    if top_level && !export.export_kind.is_type() =>
                {
                    let target = sources.get(&export.source.span).cloned();
                    if let Some(name) = &export.exported {
                        self.exports.insert(
                            (module.clone(), name.name().to_string()),
                            target.map_or(Value::Unknown, Value::Namespace),
                        );
                    } else {
                        self.stars.entry(module.clone()).or_default().push(target);
                    }
                }
                AstKind::CallExpression(call) => {
                    self.call(module, semantic, node.id(), &call.callee)
                }
                AstKind::NewExpression(call) => {
                    self.call(module, semantic, node.id(), &call.callee)
                }
                AstKind::TaggedTemplateExpression(call) => {
                    self.call(module, semantic, node.id(), &call.tag)
                }
                _ => {}
            }
        }

        // Collect simple assignments after declarations so an assignment before
        // a declaration isn't overwritten by the initializer. This is a may-call
        // union; execution order and conditional writes belong to later analysis.
        for node in semantic.nodes().iter() {
            if let AstKind::AssignmentExpression(assignment) = node.kind()
                && let AssignmentTarget::AssignmentTargetIdentifier(id) = &assignment.left
                && let Value::Binding(key) = self.identifier(module, semantic, id)
            {
                let assigned = self.expression(module, semantic, &assignment.right);
                let initial = self.bindings.remove(&key).unwrap_or(Value::Unknown);
                self.bindings
                    .insert(key, Value::Union(vec![initial, assigned]));
            }
        }

        // Keep the initial known target, but don't claim a mutated binding's
        // initializer accounts for every value it might hold at a call site.
        for symbol in semantic.scoping().symbol_ids() {
            if semantic.scoping().symbol_is_mutated(symbol)
                && let Some(value) = self.bindings.get_mut(&(module.clone(), symbol))
            {
                *value = Value::Union(vec![value.clone(), Value::Unknown]);
            }
        }
    }

    fn call(
        &mut self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        node: NodeId,
        callee: &Expression<'_>,
    ) {
        let caller = semantic
            .nodes()
            .ancestor_ids(node)
            .find_map(|ancestor| self.functions.get(&(module.clone(), ancestor)).copied());
        self.calls.push(PendingCall {
            caller,
            site: EscCallSite {
                module_id: module.clone(),
                node_id: node,
            },
            callee: self.expression(module, semantic, callee),
        });
    }

    fn resolve(&self, value: &Value, visiting: &mut HashSet<Lookup>) -> Targets {
        let mut result = Targets::default();
        match value {
            Value::Unknown => result.unknown = true,
            Value::Function(index) => {
                result.functions.insert(*index);
            }
            Value::Namespace(module) => {
                result.namespaces.insert(module.clone());
            }
            Value::Union(values) => {
                for value in values {
                    result.merge(self.resolve(value, visiting));
                }
            }
            Value::Binding(key) => {
                let lookup = Lookup::Binding(key.clone());
                if !visiting.insert(lookup) {
                    return result;
                }
                result = self.bindings.get(key).map_or_else(
                    || Targets {
                        unknown: true,
                        ..Targets::default()
                    },
                    |value| self.resolve(value, visiting),
                );
                visiting.remove(&Lookup::Binding(key.clone()));
            }
            Value::Export(key) => {
                let lookup = Lookup::Export(key.clone());
                if !visiting.insert(lookup) {
                    return result;
                }
                if let Some(value) = self.exports.get(key) {
                    result = self.resolve(value, visiting);
                } else if key.1 != "default" {
                    for source in self.stars.get(&key.0).into_iter().flatten() {
                        match source {
                            Some(module) => result.merge(self.resolve(
                                &Value::Export((module.clone(), key.1.clone())),
                                visiting,
                            )),
                            None => result.unknown = true,
                        }
                    }
                }
                visiting.remove(&Lookup::Export(key.clone()));
            }
            Value::Member(object, property) => {
                let object = self.resolve(object, visiting);
                result.unknown =
                    object.unknown || object.namespaces.is_empty() || !object.functions.is_empty();
                for module in object.namespaces {
                    result
                        .merge(self.resolve(&Value::Export((module, property.clone())), visiting));
                }
            }
        }
        result
    }

    fn id(&self, index: NodeIndex) -> EscFnId {
        EscFnId::new(self.result.graph[index].module_id.clone(), index)
    }

    fn finish(&mut self) {
        for call in std::mem::take(&mut self.calls) {
            let targets = self.resolve(&call.callee, &mut HashSet::new());
            let unresolved =
                targets.unknown || targets.functions.is_empty() || !targets.namespaces.is_empty();
            if let Some(caller) = call.caller {
                for &callee in &targets.functions {
                    self.result
                        .graph
                        .add_edge(caller, callee, call.site.clone());
                }
            }
            self.result.calls.push(EscCall {
                caller: call.caller.map(|index| self.id(index)),
                site: call.site,
                targets: targets
                    .functions
                    .into_iter()
                    .map(|index| self.id(index))
                    .collect(),
                unresolved,
            });
        }
        for (component, mut members) in tarjan_scc(&self.result.graph).into_iter().enumerate() {
            members.sort_unstable();
            let members = members
                .into_iter()
                .map(|index| self.id(index))
                .collect::<Vec<_>>();
            for id in &members {
                self.result.component_of.insert(id.clone(), component);
            }
            self.result.sccs.push(members);
        }
    }
}
