//! Adapted from vendor/oxc/crates/oxc_type_checker/src/compiler/references.rs,
//! itself a port of typescript-go's internal/parser/references.go.
//!
//! Preserve imports, augmentations and reference pragmas, but collect metadata
//! and resolve runtime/declaration pairs in one AST walk instead of retaining
//! only module names. Type syntax must remain available, unlike a bundler's
//! post-transform scan.

use crate::prelude::*;
use oxc_ast::ast::{
    CallExpression, ExportAllDeclaration, ExportFromDeclaration, Expression, ImportDeclaration,
    ImportDeclarationSpecifier, ImportExpression, Program, TSExternalModuleDeclaration,
    TSImportEqualsDeclaration, TSImportType, TSModuleReference,
};
use oxc_ast_visit::{Visit, walk};
use oxc_span::{GetSpan, Span};
use oxc_syntax::module_record::ModuleRecord;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EscModuleReferenceKind {
    Import,
    Export,
    Require,
    ImportEquals,
    DynamicImport,
    ImportType,
    Augmentation,
    Path,
    Types,
}

#[derive(Clone, Debug)]
pub struct EscModuleReferenceInfo {
    pub specifier: String,
    /// Location of the module specifier (the comment for reference pragmas).
    pub span: Span,
    pub kind: EscModuleReferenceKind,
    /// Explicit type syntax, or a reference inside a declaration/ambient body.
    pub type_only: bool,
}

#[derive(Clone, Debug)]
pub enum EscModuleReference {
    /// OS/runtime-provided modules, such as fs and node:fs; no source to load.
    Internal {
        info: EscModuleReferenceInfo,
        name: String,
    },
    External {
        info: EscModuleReferenceInfo,
        /// Runtime/source file, or the declaration when no runtime file exists.
        module: EscModulePath,
        /// Separate type-facing file, if resolution found one.
        types: Option<EscModulePath>,
    },
    /// Preserve the request and its location even when it cannot be resolved.
    Unresolved { info: EscModuleReferenceInfo },
}

impl EscModuleReference {
    pub fn info(&self) -> &EscModuleReferenceInfo {
        match self {
            Self::Internal { info, .. }
            | Self::External { info, .. }
            | Self::Unresolved { info } => info,
        }
    }

    fn dependencies(&self) -> impl Iterator<Item = &EscModulePath> {
        let paths = match self {
            Self::External {
                info,
                module,
                types,
            } if info.type_only => [Some(types.as_ref().unwrap_or(module)), None],
            Self::External { module, types, .. } => [Some(module), types.as_ref()],
            _ => [None, None],
        };
        paths.into_iter().flatten()
    }
}

#[derive(Debug, Default)]
pub struct EscModuleReferences {
    /// Static imports/re-exports, then dynamic calls and import types, each in source order.
    pub imports: Vec<EscModuleReference>,
    pub module_augmentations: Vec<EscModuleReference>,
    pub referenced_files: Vec<EscModuleReference>,
    pub type_reference_directives: Vec<EscModuleReference>,
}

impl EscModuleReferences {
    pub fn collect(
        program: &Program<'_>,
        module_record: &ModuleRecord<'_>,
        path: &EscModulePath,
        resolver: &EscResolver,
    ) -> Self {
        let mut collector = Collector {
            path,
            resolver,
            is_external_module: module_record.has_module_syntax,
            type_only: program.source_type.is_typescript_definition(),
            in_ambient_module: false,
            references: Self::default(),
        };
        collector.visit_program(program);
        // Keep Oxc's ordering without another AST walk.
        collector.references.imports.sort_by_key(|reference| {
            let info = reference.info();
            let dynamic = matches!(
                info.kind,
                EscModuleReferenceKind::DynamicImport
                    | EscModuleReferenceKind::ImportType
                    | EscModuleReferenceKind::Require
            );
            (dynamic, info.span.start)
        });
        collect_reference_pragmas(program, &mut collector);
        collector.references
    }

    pub fn dependencies(&self) -> impl Iterator<Item = &EscModulePath> {
        self.referenced_files
            .iter()
            .chain(&self.type_reference_directives)
            .chain(&self.imports)
            .chain(&self.module_augmentations)
            .flat_map(EscModuleReference::dependencies)
    }
}

struct Collector<'a> {
    path: &'a EscModulePath,
    resolver: &'a EscResolver,
    is_external_module: bool,
    type_only: bool,
    in_ambient_module: bool,
    references: EscModuleReferences,
}

impl Collector<'_> {
    fn resolve(
        &self,
        span: Span,
        name: &str,
        kind: EscModuleReferenceKind,
        type_only: bool,
    ) -> EscModuleReference {
        self.resolver.resolve_reference(
            self.path,
            EscModuleReferenceInfo {
                specifier: name.to_string(),
                span,
                kind,
                type_only: self.type_only || type_only,
            },
        )
    }

    fn add_static(
        &mut self,
        span: Span,
        name: &str,
        kind: EscModuleReferenceKind,
        type_only: bool,
    ) {
        if !name.is_empty() && !(self.in_ambient_module && is_external_module_name_relative(name)) {
            let reference = self.resolve(span, name, kind, type_only);
            self.references.imports.push(reference);
        }
    }

    fn add_string_literal_like(
        &mut self,
        expression: &Expression<'_>,
        kind: EscModuleReferenceKind,
    ) {
        let name = match expression {
            Expression::StringLiteral(literal) => literal.value.as_str(),
            Expression::TemplateLiteral(template) if template.is_no_substitution_template() => {
                let Some(cooked) = template.quasis[0].value.cooked.as_ref() else {
                    return;
                };
                cooked.as_str()
            }
            _ => return,
        };
        if !name.is_empty() {
            let reference = self.resolve(expression.span(), name, kind, false);
            self.references.imports.push(reference);
        }
    }
}

impl<'a> Visit<'a> for Collector<'_> {
    fn visit_import_declaration(&mut self, it: &ImportDeclaration<'a>) {
        let type_only = it.import_kind.is_type() || it.specifiers.as_ref().is_some_and(|specifiers| {
            !specifiers.is_empty() && specifiers.iter().all(|specifier| {
                matches!(specifier, ImportDeclarationSpecifier::ImportSpecifier(specifier) if specifier.import_kind.is_type())
            })
        });
        self.add_static(
            it.source.span,
            &it.source.value,
            EscModuleReferenceKind::Import,
            type_only,
        );
    }

    fn visit_export_from_declaration(&mut self, it: &ExportFromDeclaration<'a>) {
        let type_only = it.export_kind.is_type()
            || (!it.specifiers.is_empty()
                && it
                    .specifiers
                    .iter()
                    .all(|specifier| specifier.export_kind.is_type()));
        self.add_static(
            it.source.span,
            &it.source.value,
            EscModuleReferenceKind::Export,
            type_only,
        );
    }

    fn visit_export_all_declaration(&mut self, it: &ExportAllDeclaration<'a>) {
        self.add_static(
            it.source.span,
            &it.source.value,
            EscModuleReferenceKind::Export,
            it.export_kind.is_type(),
        );
    }

    fn visit_ts_import_equals_declaration(&mut self, it: &TSImportEqualsDeclaration<'a>) {
        if let TSModuleReference::ExternalModuleReference(reference) = &it.module_reference {
            self.add_static(
                reference.expression.span,
                &reference.expression.value,
                EscModuleReferenceKind::ImportEquals,
                it.import_kind.is_type(),
            );
        }
    }

    fn visit_ts_external_module_declaration(&mut self, it: &TSExternalModuleDeclaration<'a>) {
        if !(self.in_ambient_module || it.declare || self.type_only) {
            return;
        }
        if self.is_external_module
            || (self.in_ambient_module && !is_external_module_name_relative(&it.id.value))
        {
            let reference = self.resolve(
                it.id.span,
                &it.id.value,
                EscModuleReferenceKind::Augmentation,
                true,
            );
            self.references.module_augmentations.push(reference);
        }
        // A top-level ambient declaration in a script declares a module; it is
        // not an import. Its body can still refer to other packages.
        let previous_ambient = self.in_ambient_module;
        let previous_type_only = self.type_only;
        self.in_ambient_module = true;
        self.type_only = true;
        walk::walk_ts_external_module_declaration(self, it);
        self.type_only = previous_type_only;
        self.in_ambient_module = previous_ambient;
    }

    fn visit_import_expression(&mut self, it: &ImportExpression<'a>) {
        self.add_string_literal_like(&it.source, EscModuleReferenceKind::DynamicImport);
        walk::walk_import_expression(self, it);
    }

    fn visit_ts_import_type(&mut self, it: &TSImportType<'a>) {
        if !it.source.value.is_empty() {
            let reference = self.resolve(
                it.source.span,
                &it.source.value,
                EscModuleReferenceKind::ImportType,
                true,
            );
            self.references.imports.push(reference);
        }
        // Type arguments may nest further import types.
        walk::walk_ts_import_type(self, it);
    }

    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        // As in Oxc's collector, this recognizes the call shape. Keep requires
        // in TS as well as JS, since we also need the executable module graph.
        if let Expression::Identifier(callee) = &it.callee
            && callee.name == "require"
            && it.arguments.len() == 1
            && let Some(argument) = it.arguments[0].as_expression()
        {
            self.add_string_literal_like(argument, EscModuleReferenceKind::Require);
        }
        walk::walk_call_expression(self, it);
    }
}

fn is_external_module_name_relative(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("..").or_else(|| name.strip_prefix('.')) else {
        return false;
    };
    rest.is_empty() || rest.starts_with('/') || rest.starts_with('\\')
}

/// As in Oxc, only leading line-comment reference pragmas are collected.
fn collect_reference_pragmas(program: &Program<'_>, collector: &mut Collector<'_>) {
    let first_token_start = program
        .directives
        .first()
        .map(|directive| directive.span.start)
        .or_else(|| program.body.first().map(|statement| statement.span().start))
        .unwrap_or(u32::MAX);
    for comment in &program.comments {
        if comment.span.start >= first_token_start {
            break;
        }
        if !comment.is_line() {
            continue;
        }
        let Some((name, value)) =
            parse_reference_pragma(comment.content_span().source_text(program.source_text))
        else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let kind = match name {
            "path" => EscModuleReferenceKind::Path,
            "types" => EscModuleReferenceKind::Types,
            _ => continue,
        };
        let reference = collector.resolve(comment.span, value, kind, true);
        match kind {
            EscModuleReferenceKind::Path => collector.references.referenced_files.push(reference),
            EscModuleReferenceKind::Types => collector
                .references
                .type_reference_directives
                .push(reference),
            _ => unreachable!(),
        }
    }
}
/// Parse a `/ <reference path|types = "..." />` line-comment body (the leading `//` is already
/// stripped), mirroring tsc's `fullTripleSlashReference(Path|TypeReference)RegEx`: the attribute
/// must directly follow `<reference`.
fn parse_reference_pragma(content: &str) -> Option<(&'static str, &str)> {
    let rest = content.strip_prefix('/')?.trim_start();
    let rest = rest.strip_prefix("<reference")?;
    let rest = rest.trim_start();
    let (name, rest) = if let Some(rest) = rest.strip_prefix("path") {
        ("path", rest)
    } else if let Some(rest) = rest.strip_prefix("types") {
        ("types", rest)
    } else if let Some(rest) = rest.strip_prefix("lib") {
        ("lib", rest)
    } else {
        return None;
    };
    let rest = rest.trim_start().strip_prefix('=')?.trim_start();
    let quote = rest.chars().next().filter(|&c| c == '"' || c == '\'')?;
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    Some((name, &rest[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, tempdir};

    fn fixture(files: &[(&str, &str)]) -> TempDir {
        let dir = tempdir().unwrap();
        for (name, source) in files {
            let path = dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, source).unwrap();
        }
        dir
    }

    fn parse(dir: &TempDir, name: &str) -> EscModule {
        let path = EscModulePath::try_new(dir.path().join(name)).unwrap();
        let resolver = EscResolver::resolve(Some(&dir.path().to_path_buf())).unwrap();
        let module =
            EscModule::parse(std::fs::read_to_string(&path).unwrap(), &path, &resolver).unwrap();
        module.with_parsed(|parsed| {
            assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics)
        });
        module
    }

    fn dependency_names(dir: &TempDir, module: &EscModule) -> HashSet<String> {
        module
            .references
            .dependencies()
            .map(|path| {
                path.as_path()
                    .strip_prefix(dir.path())
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn pairs_conditional_exports_with_separate_declaration_directories() {
        let dir = fixture(&[
            (
                "entry.ts",
                "import 'dual'; import cjs = require('dual'); require('dual'); import('dual');",
            ),
            (
                "node_modules/dual/package.json",
                r#"{"exports":{".":{
                "import":{"types":"./types/esm.d.mts","default":"./dist/esm.mjs"},
                "require":{"types":"./types/cjs.d.cts","default":"./dist/cjs.cjs"}
            }}}"#,
            ),
            (
                "node_modules/dual/types/esm.d.mts",
                "export declare const value: number;",
            ),
            (
                "node_modules/dual/types/cjs.d.cts",
                "export declare const value: number;",
            ),
            ("node_modules/dual/dist/esm.mjs", "export const value = 1;"),
            ("node_modules/dual/dist/cjs.cjs", "exports.value = 1;"),
        ]);
        let module = parse(&dir, "entry.ts");
        assert_eq!(module.references.imports.len(), 4);
        for reference in &module.references.imports {
            let EscModuleReference::External {
                info,
                module,
                types: Some(types),
            } = reference
            else {
                panic!("Expected a module/declaration pair: {reference:?}");
            };
            assert_eq!(info.specifier, "dual");
            assert!(!info.type_only);
            let (runtime, declaration) = if matches!(
                info.kind,
                EscModuleReferenceKind::Require | EscModuleReferenceKind::ImportEquals
            ) {
                ("dist/cjs.cjs", "types/cjs.d.cts")
            } else {
                ("dist/esm.mjs", "types/esm.d.mts")
            };
            assert!(module.as_path().ends_with(runtime));
            assert!(types.as_path().ends_with(declaration));
        }
        assert_eq!(dependency_names(&dir, &module).len(), 4);
    }

    #[test]
    fn preserves_type_only_metadata_and_loads_only_type_dependencies() {
        let source = r#"
            import type { T } from './types.js';
            import { type T as U } from './types.js';
            export { type T } from './types.js';
            export type * from './types.js';
            import type C = require('./types.js');
            type Query = import('./types.js').T<import('./nested.js').N>;
            import { type T as V, value } from './value.js';
        "#;
        let dir = fixture(&[
            ("entry.ts", source),
            ("types.js", "export const value = 1;"),
            ("types.d.ts", "export interface T<X = unknown> {}"),
            ("nested.js", "export const value = 1;"),
            ("nested.d.ts", "export interface N {}"),
            ("value.js", "export const value = 1;"),
            ("value.d.ts", "export declare const value: number;"),
        ]);
        let module = parse(&dir, "entry.ts");
        assert_eq!(module.references.imports.len(), 8);
        for reference in &module.references.imports {
            let info = reference.info();
            assert_eq!(info.type_only, info.specifier != "./value.js");
            assert_eq!(
                info.span.source_text(source),
                format!("'{}'", info.specifier)
            );
        }
        assert_eq!(
            dependency_names(&dir, &module),
            HashSet::from([
                "types.d.ts".into(),
                "nested.d.ts".into(),
                "value.js".into(),
                "value.d.ts".into(),
            ])
        );
    }

    #[test]
    fn retains_builtins_and_unresolved_requests_without_file_dependencies() {
        let dir = fixture(&[(
            "entry.js",
            "import fs from 'node:fs'; require(`os`); import('not-installed');",
        )]);
        let module = parse(&dir, "entry.js");
        assert!(
            matches!(&module.references.imports[0], EscModuleReference::Internal { name, .. } if name == "node:fs")
        );
        assert!(
            matches!(&module.references.imports[1], EscModuleReference::Internal { name, .. } if name == "node:os")
        );
        assert!(
            matches!(&module.references.imports[2], EscModuleReference::Unresolved { info } if info.specifier == "not-installed")
        );
        assert!(module.references.dependencies().next().is_none());
    }

    #[test]
    fn preserves_pragmas_ambient_imports_and_augmentations() {
        let dir = fixture(&[
            (
                "entry.d.ts",
                "/// <reference path=\"local.d.ts\" />\n/// <reference types=\"ambient\" />\nimport './value.js'; declare module './value.js' {}",
            ),
            (
                "local.d.ts",
                "declare module 'virtual' { import { Value } from 'ambient'; import { Ignored } from './ignored'; }",
            ),
            ("value.js", "export const value = 1;"),
            ("value.d.ts", "export interface Value {}"),
            (
                "node_modules/@types/ambient/index.d.ts",
                "export interface Value {}",
            ),
        ]);
        let module = parse(&dir, "entry.d.ts");
        assert_eq!(module.references.referenced_files.len(), 1);
        assert_eq!(module.references.type_reference_directives.len(), 1);
        assert_eq!(module.references.module_augmentations.len(), 1);
        assert!(module.references.imports[0].info().type_only);
        assert_eq!(
            dependency_names(&dir, &module),
            HashSet::from([
                "local.d.ts".into(),
                "node_modules/@types/ambient/index.d.ts".into(),
                "value.d.ts".into(),
            ])
        );
        let ambient = parse(&dir, "local.d.ts");
        assert!(ambient.references.module_augmentations.is_empty());
        assert_eq!(ambient.references.imports.len(), 1);
        assert!(ambient.references.imports[0].info().type_only);
        assert_eq!(ambient.references.imports[0].info().specifier, "ambient");
    }

    #[test]
    fn pairs_package_imports_and_tsconfig_aliases() {
        let dir = fixture(&[
            ("entry.ts", "import '#local'; import '@mapped/local';"),
            (
                "package.json",
                r##"{"imports":{"#local":"./src/local.js"}}"##,
            ),
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"paths":{"@mapped/*":["./src/*"]}}}"#,
            ),
            ("src/local.js", "export const value = 1;"),
            ("src/local.d.ts", "export declare const value: number;"),
        ]);
        let module = parse(&dir, "entry.ts");
        assert_eq!(module.references.imports.len(), 2);
        for reference in &module.references.imports {
            assert!(
                matches!(reference, EscModuleReference::External { module, types: Some(types), .. }
                if module.as_path().ends_with("local.js") && types.as_path().ends_with("local.d.ts")),
                "{reference:?}"
            );
        }
    }

    #[test]
    fn supports_declaration_only_modules_and_ts_extension_substitution() {
        let dir = fixture(&[
            (
                "entry.ts",
                "import type { T } from './only.js'; import './source.js';",
            ),
            ("only.d.ts", "export interface T {}"),
            ("source.ts", "export const value = 1;"),
        ]);
        let module = parse(&dir, "entry.ts");
        assert_eq!(
            dependency_names(&dir, &module),
            HashSet::from(["only.d.ts".into(), "source.ts".into()])
        );
        assert!(
            matches!(&module.references.imports[0], EscModuleReference::External { module, types: None, .. } if module.as_path().ends_with("only.d.ts"))
        );
        assert!(matches!(
            &module.references.imports[1],
            EscModuleReference::External { types: None, .. }
        ));
    }
}
