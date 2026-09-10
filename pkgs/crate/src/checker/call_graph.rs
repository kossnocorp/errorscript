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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Value {
    Signature(EscErrorId),
    Function(NodeIndex),
    Type(EscErrorType),
    TypeOnly(Box<Value>),
    Global(&'static str),
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
    export_names: HashMap<EscModuleId, HashSet<String>>,
    unknown_exports: HashSet<EscModuleId>,
    resolving: bool,
    resolved: std::cell::RefCell<HashMap<Value, Targets>>,
    module_pairs: HashSet<(EscModuleId, EscModuleId)>,
    sources: HashMap<(EscModuleId, Span), EscModuleId>,
    result: EscCallGraph,
    functions: HashMap<(EscModuleId, NodeId), NodeIndex>,
    bindings: HashMap<SymbolKey, Value>,
    exports: HashMap<ExportKey, Value>,
    stars: HashMap<EscModuleId, Vec<Option<EscModuleId>>>,
    calls: Vec<PendingCall>,
    classes: HashMap<EscErrorType, ClassInfo>,
    type_queries: Vec<(EscErrorId, Value, bool)>,
    modified_globals: HashSet<String>,
}

struct ClassInfo {
    constructor: Option<NodeIndex>,
    superclass: Option<Value>,
    unknown_initialization: bool,
}

#[derive(Clone, Default)]
struct Targets {
    signatures: HashSet<EscErrorId>,
    functions: BTreeSet<NodeIndex>,
    namespaces: BTreeSet<EscModuleId>,
    unknown: bool,
    types: HashSet<EscErrorType>,
    annotation_types: HashSet<EscErrorType>,
    globals: HashSet<&'static str>,
}

impl Targets {
    fn merge(&mut self, other: Self) {
        self.signatures.extend(other.signatures);
        self.functions.extend(other.functions);
        self.namespaces.extend(other.namespaces);
        self.unknown |= other.unknown;
        self.types.extend(other.types);
        self.annotation_types.extend(other.annotation_types);
        self.globals.extend(other.globals);
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
                        is_async: match node.kind() {
                            AstKind::Function(f) => f.r#async,
                            AstKind::ArrowFunctionExpression(f) => f.r#async,
                            _ => false,
                        },
                        is_generator: matches!(node.kind(), AstKind::Function(f) if f.generator),
                    });
                    builder
                        .functions
                        .insert(((*module_id).clone(), node.id()), index);
                }
            });
        }

        let mut module_ids = HashMap::new();
        let mut module_id_for = |path: &EscModulePath| -> Result<EscModuleId> {
            if let Some(id) = module_ids.get(path) {
                return Ok(EscModuleId::clone(id));
            }
            let id = EscModuleId::from_path(path, repo)?;
            module_ids.insert(path.clone(), id.clone());
            Ok(id)
        };
        for (module_id, module) in modules {
            // Resolve source spans rather than matching a possibly repeated
            // module name with a different import mode.
            let mut sources = HashMap::new();
            for reference in &module.references.imports {
                if let EscModuleReference::External {
                    info,
                    module,
                    types,
                } = reference
                {
                    if let Some(types) = types {
                        builder
                            .module_pairs
                            .insert((module_id_for(module)?, module_id_for(types)?));
                    }
                    let module = if info.type_only {
                        types.as_ref().unwrap_or(module)
                    } else {
                        module
                    };
                    let id = module_id_for(module)?;
                    if parsed.parsed_files.contains_key(&id) {
                        sources.insert(info.span, id);
                    }
                }
            }
            module.with_semantic(|result| {
                builder.sources.extend(
                    sources
                        .iter()
                        .map(|(span, source)| ((module_id.clone(), *span), source.clone())),
                );
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
            .map_or_else(
                || {
                    EscGlobals::get(id.name.as_str())
                        .map_or(Value::Unknown, |global| Value::Global(global.path))
                },
                |symbol| Value::Binding((module.clone(), symbol)),
            )
    }

    fn expression(
        &self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        expr: &Expression<'_>,
    ) -> Value {
        let expr = expr.get_inner_expression();
        match expr {
            Expression::CallExpression(call)
                if matches!(call.callee.get_inner_expression(), Expression::Identifier(id)
                if id.name == "require" && id.reference_id.get().is_some_and(|id| semantic.scoping().get_reference(id).symbol_id().is_none())) =>
            {
                call.arguments
                    .first()
                    .and_then(|argument| argument.as_expression())
                    .and_then(|argument| {
                        if let Expression::StringLiteral(source) = argument {
                            self.sources.get(&(module.clone(), source.span)).cloned()
                        } else {
                            None
                        }
                    })
                    .map_or(Value::Unknown, Value::Namespace)
            }
            Expression::Identifier(id) => self.identifier(module, semantic, id),
            Expression::FunctionExpression(function) => {
                self.function(module, function.node_id.get())
            }
            Expression::ArrowFunctionExpression(function) => {
                self.function(module, function.node_id.get())
            }
            Expression::ClassExpression(class) => Value::Type(EscErrorType::Node(EscErrorId {
                module_id: module.clone(),
                node: class.node_id.get(),
            })),
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
                AstKind::TSInterfaceDeclaration(interface) => {
                    self.bind(
                        module,
                        &interface.id,
                        Value::TypeOnly(Box::new(Value::Type(EscErrorType::Node(EscErrorId {
                            module_id: module.clone(),
                            node: node.id(),
                        })))),
                    );
                }
                AstKind::TSTypeAliasDeclaration(alias) => {
                    self.bind(
                        module,
                        &alias.id,
                        Value::TypeOnly(Box::new(Value::Type(EscErrorType::Node(EscErrorId {
                            module_id: module.clone(),
                            node: node.id(),
                        })))),
                    );
                }
                AstKind::Class(class) => {
                    let ty = EscErrorType::Node(EscErrorId {
                        module_id: module.clone(),
                        node: node.id(),
                    });
                    if let Some(id) = &class.id {
                        self.bind(module, id, Value::Type(ty.clone()));
                    }
                    let constructor = class.body.body.iter().find_map(|element| {
                        if let ClassElement::MethodDefinition(method) = element
                            && method.kind == MethodDefinitionKind::Constructor
                        {
                            self.functions
                                .get(&(module.clone(), method.value.node_id.get()))
                                .copied()
                        } else {
                            None
                        }
                    });
                    self.classes.insert(ty, ClassInfo {
                        constructor,
                        superclass: class.heritage.as_ref().map(|heritage| self.expression(module, semantic, &heritage.expression)),
                        unknown_initialization: class.declare || !class.decorators.is_empty() || class.body.body.iter().any(|element| {
                            matches!(element, ClassElement::PropertyDefinition(field) if !field.r#static && field.value.is_some())
                                || matches!(element, ClassElement::AccessorProperty(_))
                        }),
                    });
                }
                AstKind::Function(function) => {
                    if let Some(id) = &function.id {
                        let value = if function.body.is_none() {
                            Value::Signature(EscErrorId {
                                module_id: module.clone(),
                                node: node.id(),
                            })
                        } else {
                            self.function(module, node.id())
                        };
                        let key = id.symbol_id.get().map(|symbol| (module.clone(), symbol));
                        if function.body.is_none()
                            && let Some(previous) =
                                key.and_then(|key| self.bindings.get(&key)).cloned()
                        {
                            self.bind(module, id, Value::Union(vec![previous, value]));
                        } else {
                            self.bind(module, id, value);
                        }
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
                AstKind::ImportDeclaration(import) => {
                    for specifier in import.specifiers.iter().flatten() {
                        let (local, value) = match specifier {
                            ImportDeclarationSpecifier::ImportSpecifier(specifier) => {
                                let value = Self::imported(
                                    sources,
                                    import.source.span,
                                    specifier.imported.name().as_str(),
                                );
                                (
                                    &specifier.local,
                                    if specifier.import_kind.is_type() {
                                        Value::TypeOnly(Box::new(value))
                                    } else {
                                        value
                                    },
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
                        };
                        let value = if import.import_kind.is_type() {
                            Value::TypeOnly(Box::new(value))
                        } else {
                            value
                        };
                        self.bind(module, local, value);
                    }
                }
                AstKind::ExportDeclaration(export) if top_level => match &export.declaration {
                    Declaration::TSInterfaceDeclaration(interface) => {
                        self.export_binding(module, &interface.id)
                    }
                    Declaration::TSTypeAliasDeclaration(alias) => {
                        self.export_binding(module, &alias.id)
                    }
                    Declaration::ClassDeclaration(class) => {
                        if let Some(id) = &class.id {
                            self.export_binding(module, id);
                        }
                    }
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
                AstKind::ExportNamedDeclaration(export) if top_level => {
                    for specifier in &export.specifiers {
                        let value = match &specifier.local {
                            ModuleExportName::IdentifierReference(id) => {
                                self.identifier(module, semantic, id)
                            }
                            _ => Value::Unknown,
                        };
                        let value =
                            if export.export_kind.is_type() || specifier.export_kind.is_type() {
                                Value::TypeOnly(Box::new(value))
                            } else {
                                value
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
                        ExportDefaultDeclarationKind::ClassDeclaration(class) => {
                            Value::Type(EscErrorType::Node(EscErrorId {
                                module_id: module.clone(),
                                node: class.node_id.get(),
                            }))
                        }
                        declaration => declaration.as_expression().map_or(Value::Unknown, |expr| {
                            self.expression(module, semantic, expr)
                        }),
                    };
                    self.exports
                        .insert((module.clone(), "default".into()), value);
                }
                AstKind::ExportFromDeclaration(export) if top_level => {
                    for specifier in &export.specifiers {
                        let value = Self::imported(
                            sources,
                            export.source.span,
                            specifier.local.name().as_str(),
                        );
                        let value =
                            if export.export_kind.is_type() || specifier.export_kind.is_type() {
                                Value::TypeOnly(Box::new(value))
                            } else {
                                value
                            };
                        self.exports.insert(
                            (module.clone(), specifier.exported.name().to_string()),
                            value,
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
                AstKind::BinaryExpression(binary) if binary.operator.is_instance_of() => {
                    self.type_queries.push((
                        EscErrorId {
                            module_id: module.clone(),
                            node: node.id(),
                        },
                        self.expression(module, semantic, &binary.right),
                        false,
                    ));
                }
                AstKind::TSTypeReference(reference) => {
                    self.type_queries.push((
                        EscErrorId {
                            module_id: module.clone(),
                            node: node.id(),
                        },
                        self.type_name(module, semantic, &reference.type_name),
                        true,
                    ));
                }
                AstKind::TSInterfaceHeritage(heritage) => {
                    self.type_queries.push((
                        EscErrorId {
                            module_id: module.clone(),
                            node: node.id(),
                        },
                        self.type_name(module, semantic, &heritage.type_name),
                        true,
                    ));
                }
                _ => {}
            }
        }

        // Collect simple assignments after declarations so an assignment before
        // a declaration isn't overwritten by the initializer. This is a may-call
        // union; execution order and conditional writes belong to later analysis.
        for node in semantic.nodes().iter() {
            if let AstKind::AssignmentExpression(assignment) = node.kind() {
                if let Some(member) = assignment.left.as_member_expression() {
                    if let Expression::Identifier(object) = member.object().get_inner_expression()
                        && object.name == "exports"
                        && object.reference_id.get().is_some_and(|id| {
                            semantic.scoping().get_reference(id).symbol_id().is_none()
                        })
                        && let Some(name) = member.static_property_name()
                    {
                        self.exports.insert(
                            (module.clone(), name.to_owned()),
                            self.expression(module, semantic, &assignment.right),
                        );
                    }
                    let value = self.expression(module, semantic, member.object());
                    for path in self.resolve(&value, &mut HashSet::new()).globals {
                        self.modified_globals
                            .insert(path.split('.').next().unwrap().to_owned());
                    }
                }
                if let AssignmentTarget::AssignmentTargetIdentifier(id) = &assignment.left
                    && id.reference_id.get().is_some_and(|id| {
                        semantic.scoping().get_reference(id).symbol_id().is_none()
                    })
                    && EscGlobals::get(id.name.as_str()).is_some()
                {
                    self.modified_globals.insert(id.name.to_string());
                }
            }
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
        let callee = if matches!(callee, Expression::Super(_)) {
            semantic
                .nodes()
                .ancestor_kinds(node)
                .find_map(|kind| {
                    if let AstKind::Class(class) = kind {
                        Some(class.heritage.as_ref().map_or(Value::Unknown, |heritage| {
                            self.expression(module, semantic, &heritage.expression)
                        }))
                    } else {
                        None
                    }
                })
                .unwrap_or(Value::Unknown)
        } else {
            self.expression(module, semantic, callee)
        };
        self.calls.push(PendingCall {
            caller,
            site: EscCallSite {
                module_id: module.clone(),
                node_id: node,
            },
            callee,
        });
    }

    fn type_name(
        &self,
        module: &EscModuleId,
        semantic: &Semantic<'_>,
        name: &TSTypeName<'_>,
    ) -> Value {
        match name {
            TSTypeName::IdentifierReference(id) => self.identifier(module, semantic, id),
            TSTypeName::QualifiedName(name) => Value::Member(
                Box::new(self.type_name(module, semantic, &name.left)),
                name.right.name.to_string(),
            ),
            _ => Value::Unknown,
        }
    }

    fn resolve(&self, value: &Value, visiting: &mut HashSet<Lookup>) -> Targets {
        // Only context-free roots are memoized. A lookup reached while breaking
        // an export/binding cycle can have a deliberately partial result.
        if self.resolving
            && let Some(result) = self.resolved.borrow().get(value)
        {
            return result.clone();
        }
        if self.resolving && visiting.is_empty() {
            let result = self.resolve_inner(value, visiting);
            self.resolved
                .borrow_mut()
                .insert(value.clone(), result.clone());
            result
        } else {
            self.resolve_inner(value, visiting)
        }
    }

    fn resolve_inner(&self, value: &Value, visiting: &mut HashSet<Lookup>) -> Targets {
        let mut result = Targets::default();
        match value {
            Value::Signature(signature) => {
                result.signatures.insert(signature.clone());
                result.unknown = true;
            }
            Value::Unknown => result.unknown = true,
            Value::Function(index) => {
                result.functions.insert(*index);
            }
            Value::Type(ty) => {
                result.types.insert(ty.clone());
            }
            Value::TypeOnly(value) => {
                result = self.resolve(value, visiting);
                result.annotation_types.extend(result.types.drain());
                result.functions.clear();
                result.globals.clear();
            }
            Value::Global(path) => {
                if self.modified_globals.contains("globalThis")
                    || self
                        .modified_globals
                        .contains(path.split('.').next().unwrap())
                {
                    result.unknown = true;
                } else if let Some(global) = EscGlobals::get(path) {
                    result.globals.insert(global.path);
                    if let Some(ty) = global.instance_type {
                        result.types.insert(EscErrorType::builtin(ty));
                    }
                } else {
                    result.unknown = true;
                }
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
                            Some(module) => {
                                if !self.resolving
                                    || self.unknown_exports.contains(module)
                                    || self
                                        .export_names
                                        .get(module)
                                        .is_some_and(|names| names.contains(&key.1))
                                {
                                    result.merge(self.resolve(
                                        &Value::Export((module.clone(), key.1.clone())),
                                        visiting,
                                    ));
                                }
                            }
                            None => result.unknown = true,
                        }
                    }
                }
                visiting.remove(&Lookup::Export(key.clone()));
            }
            Value::Member(object, property) => {
                let object = self.resolve(object, visiting);
                result.unknown = object.unknown
                    || (object.namespaces.is_empty() && object.globals.is_empty())
                    || !object.functions.is_empty();
                for object in object.globals {
                    if let Some(global) = EscGlobals::property(object, property) {
                        result.merge(self.resolve(&Value::Global(global.path), visiting));
                    } else {
                        result.unknown = true;
                    }
                }
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
        self.resolving = true;
        // Export names, rather than declaration-local names or filenames,
        // connect signatures across aliases and re-export barrels.
        for (module, name) in self.exports.keys() {
            self.export_names
                .entry(module.clone())
                .or_default()
                .insert(name.clone());
        }
        let mut parents: HashMap<EscModuleId, HashSet<EscModuleId>> = HashMap::new();
        for (module, sources) in &self.stars {
            for source in sources {
                if let Some(source) = source {
                    parents
                        .entry(source.clone())
                        .or_default()
                        .insert(module.clone());
                } else {
                    self.unknown_exports.insert(module.clone());
                }
            }
        }
        let mut queued = self
            .export_names
            .keys()
            .chain(&self.unknown_exports)
            .cloned()
            .collect::<HashSet<_>>();
        let mut pending = queued
            .iter()
            .cloned()
            .collect::<std::collections::VecDeque<_>>();
        while let Some(module) = pending.pop_front() {
            queued.remove(&module);
            let names = self.export_names.get(&module).cloned().unwrap_or_default();
            for parent in parents.get(&module).into_iter().flatten() {
                let target = self.export_names.entry(parent.clone()).or_default();
                let before = target.len();
                target.extend(
                    names
                        .iter()
                        .filter(|name| name.as_str() != "default")
                        .cloned(),
                );
                let changed = target.len() != before;
                let unknown = self.unknown_exports.contains(&module)
                    && self.unknown_exports.insert(parent.clone());
                if (changed || unknown) && queued.insert(parent.clone()) {
                    pending.push_back(parent.clone());
                }
            }
        }
        for (runtime, types) in &self.module_pairs {
            if runtime == types {
                continue;
            }
            // Search only exports reachable from this declaration module, not
            // every export name in the project for every runtime/type pair.
            for name in self.export_names.get(types).into_iter().flatten() {
                let implementation = self.resolve(
                    &Value::Export((runtime.clone(), name.clone())),
                    &mut HashSet::new(),
                );
                let declaration = self.resolve(
                    &Value::Export((types.clone(), name.clone())),
                    &mut HashSet::new(),
                );
                for function in implementation.functions {
                    let signatures = self.result.signatures.entry(self.id(function)).or_default();
                    for signature in &declaration.signatures {
                        if !signatures.contains(signature) {
                            signatures.push(signature.clone());
                        }
                    }
                    signatures.sort();
                }
            }
        }
        self.result.modified_globals = self.modified_globals.clone();
        for (ty, class) in &self.classes {
            let mut bases = HashSet::new();
            if let Some(base) = &class.superclass {
                let resolved = self.resolve(base, &mut HashSet::new());
                bases.extend(resolved.types);
                if resolved.unknown || bases.is_empty() {
                    bases.insert(EscErrorType::UNKNOWN);
                }
            }
            self.result.super_types.insert(ty.clone(), bases);
        }
        for (site, value, annotation) in std::mem::take(&mut self.type_queries) {
            let resolved = self.resolve(&value, &mut HashSet::new());
            let mut types = resolved.types;
            if annotation {
                types.extend(resolved.annotation_types);
            }
            if resolved.unknown || types.is_empty() {
                types.insert(EscErrorType::UNKNOWN);
            }
            self.result.type_references.insert(site, types);
        }
        for call in std::mem::take(&mut self.calls) {
            let mut targets = self.resolve(&call.callee, &mut HashSet::new());
            for ty in targets.types.clone() {
                let constructors = self.constructors(&ty, &mut HashSet::new());
                targets.functions.extend(constructors.functions);
                targets.unknown |= constructors.unknown;
            }
            let unresolved = targets.unknown
                || (targets.functions.is_empty()
                    && targets.types.is_empty()
                    && targets.globals.is_empty())
                || !targets.namespaces.is_empty();
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
                constructed_types: targets.types,
                global_calls: targets.globals,
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

    fn constructors(&self, ty: &EscErrorType, visiting: &mut HashSet<EscErrorType>) -> Targets {
        let mut targets = Targets::default();
        if !visiting.insert(ty.clone()) {
            targets.unknown = true;
            return targets;
        }
        if let Some(class) = self.classes.get(ty) {
            targets.unknown = class.unknown_initialization;
            if let Some(index) = class.constructor {
                targets.functions.insert(index);
            } else if let Some(base) = &class.superclass {
                let base = self.resolve(base, &mut HashSet::new());
                targets.unknown |= base.unknown || base.types.is_empty();
                targets.functions.extend(base.functions);
                for ty in base.types {
                    targets.merge(self.constructors(&ty, visiting));
                }
            }
        }
        visiting.remove(ty);
        targets
    }
}
