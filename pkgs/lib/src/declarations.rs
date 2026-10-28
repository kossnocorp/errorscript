use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;
use oxc_syntax::scope::ScopeFlags;

use crate::{Suite, parse};

/// Parse callable declarations without guessing their runtime error behavior.
/// Node's legacy `"console"` and canonical `"node:console"` module names merge.
pub fn parse_declarations(source: &str, node: bool) -> Result<Suite> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source, true)?;
    let mut collector = Collector {
        source,
        node,
        scope: Vec::new(),
        shapes: BTreeMap::new(),
        references: Vec::new(),
    };
    collector.visit_program(&program);
    // Resolve local aliases without expanding recursive object graphs. Referenced callable
    // properties contribute calls/constructors; variables may expose an object's direct methods.
    let mut result = Suite::default();
    for (path, shape) in &collector.shapes {
        result.at(path).merge(shape);
    }
    for reference in &collector.references {
        let shape = collector.resolve(&reference.target, &reference.scope, &mut BTreeSet::new());
        if reference.object {
            result.at(&reference.path).merge(&shape);
        } else {
            let target = result.at(&reference.path);
            target.callable |= shape.callable;
            if let Some(new) = shape.children.get("new") {
                target.children.entry("new".into()).or_default().merge(new);
            }
        }
    }
    result.prune();
    Ok(result)
}

struct Reference {
    path: Vec<String>,
    scope: Vec<String>,
    target: Vec<String>,
    object: bool,
}

struct Collector<'s> {
    source: &'s str,
    node: bool,
    scope: Vec<String>,
    shapes: BTreeMap<Vec<String>, Suite>,
    references: Vec<Reference>,
}

impl Collector<'_> {
    fn path(&self, name: &str) -> Vec<String> {
        let mut path = self.scope.clone();
        path.push(name.to_owned());
        path
    }

    fn resolve(
        &self,
        name: &[String],
        scope: &[String],
        seen: &mut BTreeSet<Vec<String>>,
    ) -> Suite {
        for depth in (0..=scope.len()).rev() {
            let path: Vec<_> = scope[..depth].iter().chain(name).cloned().collect();
            if self.shapes.contains_key(&path) || self.references.iter().any(|r| r.path == path) {
                if !seen.insert(path.clone()) {
                    return Suite::default();
                }
                let mut result = self.shapes.get(&path).cloned().unwrap_or_default();
                for reference in self.references.iter().filter(|r| r.path == path) {
                    result.merge(&self.resolve(&reference.target, &reference.scope, seen));
                }
                seen.remove(&path);
                return result;
            }
        }
        Suite::default()
    }

    fn key(&self, key: &PropertyKey<'_>, computed: bool) -> String {
        if let Some(name) = key.static_name() {
            return name.to_string();
        }
        let span = key.span();
        let text: String = self.source[span.start as usize..span.end as usize]
            .chars()
            .filter(|c| !c.is_whitespace())
            .collect();
        if computed { format!("[{text}]") } else { text }
    }

    fn callable(&mut self, path: &[String], name: &str) {
        self.shapes
            .entry(path.to_vec())
            .or_default()
            .children
            .entry(name.to_owned())
            .or_default()
            .callable = true;
    }

    fn ty(&mut self, path: &[String], ty: &TSType<'_>, object: bool) {
        match ty {
            TSType::TSFunctionType(_) => {
                self.shapes.entry(path.to_vec()).or_default().callable = true
            }
            TSType::TSConstructorType(_) => self.callable(path, "new"),
            TSType::TSTypeLiteral(literal) => self.members(path, &literal.members),
            TSType::TSParenthesizedType(ty) => self.ty(path, &ty.type_annotation, object),
            TSType::TSUnionType(ty) => {
                for ty in &ty.types {
                    self.ty(path, ty, object);
                }
            }
            TSType::TSIntersectionType(ty) => {
                for ty in &ty.types {
                    self.ty(path, ty, object);
                }
            }
            TSType::TSTypeReference(ty) => {
                let name = type_name(&ty.type_name);
                self.references.push(Reference {
                    path: path.to_vec(),
                    scope: self.scope.clone(),
                    target: name,
                    object,
                });
            }
            // A conditional declaration is still an API inventory, not a claim that either
            // branch exists in every runtime (notably Node's web-global fallbacks).
            TSType::TSConditionalType(ty) => {
                self.ty(path, &ty.true_type, object);
                self.ty(path, &ty.false_type, object);
            }
            _ => {}
        }
    }

    fn members(&mut self, path: &[String], members: &[TSSignature<'_>]) {
        for member in members {
            match member {
                TSSignature::TSMethodSignature(method) => {
                    let name = self.key(&method.key, method.computed);
                    let name = match method.kind {
                        TSMethodSignatureKind::Get => format!("get {name}"),
                        TSMethodSignatureKind::Set => format!("set {name}"),
                        TSMethodSignatureKind::Method => name,
                    };
                    self.callable(path, &name);
                }
                TSSignature::TSCallSignatureDeclaration(_) => {
                    self.shapes.entry(path.to_vec()).or_default().callable = true;
                }
                TSSignature::TSConstructSignatureDeclaration(_) => self.callable(path, "new"),
                TSSignature::TSPropertySignature(property) => {
                    if let Some(ty) = &property.type_annotation {
                        let mut child = path.to_vec();
                        child.push(self.key(&property.key, property.computed));
                        // prototype links are not functions; their members already have a suite.
                        if child.last().is_some_and(|key| key == "prototype") {
                            continue;
                        }
                        self.ty(&child, &ty.type_annotation, false);
                    }
                }
                _ => {}
            }
        }
    }
}

fn type_name(name: &TSTypeName<'_>) -> Vec<String> {
    match name {
        TSTypeName::IdentifierReference(id) => vec![id.name.to_string()],
        TSTypeName::QualifiedName(name) => {
            let mut path = type_name(&name.left);
            path.push(name.right.name.to_string());
            path
        }
        _ => Vec::new(),
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_function(&mut self, function: &Function<'a>, _: ScopeFlags) {
        if let Some(id) = &function.id {
            self.shapes
                .entry(self.path(id.name.as_str()))
                .or_default()
                .callable = true;
        }
    }

    fn visit_ts_interface_declaration(&mut self, interface: &TSInterfaceDeclaration<'a>) {
        self.members(&self.path(interface.id.name.as_str()), &interface.body.body);
    }

    fn visit_ts_type_alias_declaration(&mut self, alias: &TSTypeAliasDeclaration<'a>) {
        self.ty(
            &self.path(alias.id.name.as_str()),
            &alias.type_annotation,
            true,
        );
    }

    fn visit_variable_declaration(&mut self, declaration: &VariableDeclaration<'a>) {
        for variable in &declaration.declarations {
            if let (BindingPattern::BindingIdentifier(id), Some(ty)) =
                (&variable.id, &variable.type_annotation)
            {
                self.ty(&self.path(id.name.as_str()), &ty.type_annotation, true);
            }
        }
    }

    fn visit_class(&mut self, class: &Class<'a>) {
        let Some(id) = &class.id else {
            return;
        };
        let path = self.path(id.name.as_str());
        for element in &class.body.body {
            match element {
                ClassElement::MethodDefinition(method) => {
                    if matches!(
                        method.accessibility,
                        Some(TSAccessibility::Private | TSAccessibility::Protected)
                    ) || matches!(method.key, PropertyKey::PrivateIdentifier(_))
                    {
                        continue;
                    }
                    let name = self.key(&method.key, method.computed);
                    let name = match method.kind {
                        MethodDefinitionKind::Constructor => "new".into(),
                        MethodDefinitionKind::Get => format!("get {name}"),
                        MethodDefinitionKind::Set => format!("set {name}"),
                        MethodDefinitionKind::Method => name,
                    };
                    // Static and instance members can have the same spelling.
                    let name = if method.r#static {
                        format!("static {name}")
                    } else {
                        name
                    };
                    self.callable(&path, &name);
                }
                ClassElement::PropertyDefinition(property) => {
                    if matches!(
                        property.accessibility,
                        Some(TSAccessibility::Private | TSAccessibility::Protected)
                    ) || matches!(property.key, PropertyKey::PrivateIdentifier(_))
                    {
                        continue;
                    }
                    if let Some(ty) = &property.type_annotation {
                        let mut child = path.clone();
                        let name = self.key(&property.key, property.computed);
                        child.push(if property.r#static {
                            format!("static {name}")
                        } else {
                            name
                        });
                        self.ty(&child, &ty.type_annotation, false);
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_ts_external_module_declaration(&mut self, module: &TSExternalModuleDeclaration<'a>) {
        let name = module.id.value.as_str();
        self.scope.push(if self.node && !name.starts_with("node:") {
            format!("node:{name}")
        } else {
            name.to_owned()
        });
        if let Some(body) = &module.body {
            self.visit_ts_module_block(body);
        }
        self.scope.pop();
    }

    fn visit_ts_namespace_declaration(&mut self, namespace: &TSNamespaceDeclaration<'a>) {
        self.scope.push(namespace.id.name.to_string());
        self.visit_ts_namespace_declaration_body(&namespace.body);
        self.scope.pop();
    }

    fn visit_ts_global_declaration(&mut self, global: &TSGlobalDeclaration<'a>) {
        self.scope.push("global".into());
        self.visit_ts_module_block(&global.body);
        self.scope.pop();
    }
}
