use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};

use crate::{Suite, parse};

const HEADER: &str = "// Synced by errs-lib. Describe paths are managed; test bodies are yours.\n";
const EMPTY_MARKER: &str = "// @errs-lib:empty";

/// Inventories stay relative to the library wrapper, allowing old flat files to be adopted.
pub(crate) fn synchronize_library(
    source: Option<&str>,
    previous: &Suite,
    desired: &Suite,
    name: &str,
) -> Result<String> {
    let wrapped = |suite: &Suite| Suite {
        children: BTreeMap::from([(name.to_owned(), suite.clone())]),
        ..Suite::default()
    };
    let allocator = Allocator::default();
    let has_wrapper = if let Some(source) = source {
        let program = parse(&allocator, source, false)?;
        let bindings = Bindings::from_program(&program, source);
        !suites(
            &program.body,
            &wrapped(previous),
            &wrapped(desired),
            &bindings,
        )?
        .is_empty()
    } else {
        false
    };
    let mut result = if has_wrapper {
        synchronize(source, &wrapped(previous), &wrapped(desired))?
    } else {
        let flat = synchronize(source, previous, desired)?;
        wrap_library(&flat, name)?
    };

    // Reference-only libraries still need a test placeholder inside their library suite.
    let program = parse(&allocator, &result, false)?;
    let bindings = Bindings::from_program(&program, &result);
    let roots = suites(
        &program.body,
        &wrapped(previous),
        &wrapped(desired),
        &bindings,
    )?;
    let root = roots.get(name).context("Missing library wrapper")?;
    let newline = if result.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let marker = format!("{EMPTY_MARKER}{newline}");
    let body_start = root.open + 1;
    let body = &result[body_start..root.close];
    let body_program = parse(&allocator, body, false)?;
    let sentinel = body_program.body.iter().find_map(|statement| {
        let span = statement.span();
        let prefix = body[..span.start as usize].trim_end_matches([' ', '\t']);
        prefix
            .ends_with(&marker)
            .then(|| (prefix.len() - marker.len(), span.end as usize))
    });
    if !desired.children.is_empty() {
        if let Some((start, end)) = sentinel {
            let (start, end) = whole_line(&result, body_start + start, body_start + end);
            result.replace_range(start..end, "");
        }
    } else if sentinel.is_none() {
        let close = root.close;
        let line_start = result[..close].rfind('\n').map_or(0, |pos| pos + 1);
        let position = if result[line_start..close].trim().is_empty() {
            line_start
        } else {
            close
        };
        let prefix = if position == close { newline } else { "" };
        let text = format!(
            "{prefix}  {marker}  {}.todo(\"No callable declarations in this file\");{newline}",
            bindings.it
        );
        result.insert_str(position, &text);
    }
    parse(&allocator, &result, false).context("Wrapped library test is invalid")?;
    Ok(result)
}

fn wrap_library(source: &str, name: &str) -> Result<String> {
    let allocator = Allocator::default();
    let program = parse(&allocator, source, false)?;
    let bindings = Bindings::from_program(&program, source);
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut body = String::new();
    let mut removals = Vec::new();
    let mut preceding = program
        .directives
        .last()
        .map_or(0, |directive| directive.span.end as usize);
    for statement in &program.body {
        let span = statement.span();
        if !statement.is_module_declaration() {
            // Carry leading comments with their statements, while keeping imports/exports outside.
            let start = if preceding == 0 {
                span.start as usize
            } else {
                preceding
            };
            body.push_str(&source[start..span.end as usize]);
            removals.push((start, span.end as usize));
        }
        preceding = span.end as usize;
    }
    let mut result = source.to_owned();
    for (start, end) in removals.into_iter().rev() {
        result.replace_range(start..end, "");
    }
    if let Some(import) = &bindings.import {
        result.push_str(&format!("{newline}{import}{newline}"));
    }
    let body = indent_source(body.trim(), newline)?;
    let name = serde_json::to_string(name)?;
    Ok(format!(
        "{}{newline}{newline}{}({name}, () => {{{newline}{body}{newline}}});{newline}",
        result.trim_end(),
        bindings.describe
    ))
}

/// Indent code, but not the contents of multiline literals in hand-written assertions.
fn indent_source(source: &str, newline: &str) -> Result<String> {
    struct Literals(Vec<Span>);
    impl<'a> Visit<'a> for Literals {
        fn visit_template_literal(&mut self, literal: &TemplateLiteral<'a>) {
            self.0.push(literal.span);
        }
        fn visit_string_literal(&mut self, literal: &StringLiteral<'a>) {
            self.0.push(literal.span);
        }
    }
    let allocator = Allocator::default();
    let program = parse(&allocator, source, false)?;
    let mut literals = Literals(Vec::new());
    literals.visit_program(&program);
    let mut offset = 0;
    Ok(source
        .split(newline)
        .map(|line| {
            let inside_literal = literals
                .0
                .iter()
                .any(|span| (span.start as usize) < offset && offset < span.end as usize);
            offset += line.len() + newline.len();
            if line.is_empty() || inside_literal {
                line.to_owned()
            } else {
                format!("  {line}")
            }
        })
        .collect::<Vec<_>>()
        .join(newline))
}

/// Synchronize only managed describe nodes. Every surviving node's source is retained byte for
/// byte, except for edits to its managed children and spacing between sibling suites.
/// Unknown/custom suites and helper code survive.
pub fn synchronize(source: Option<&str>, previous: &Suite, desired: &Suite) -> Result<String> {
    let initial = format!("{HEADER}import {{ describe, it }} from \"vitest\";\n\n");
    let source = source.unwrap_or(&initial);
    let allocator = Allocator::default();
    let program = parse(&allocator, source, false)?;
    let bindings = Bindings::from_program(&program, source);
    let existing = suites(&program.body, previous, desired, &bindings)?;
    let newline = if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut edits = Vec::new();
    sync_level(
        source,
        &existing,
        previous,
        desired,
        source.len(),
        "",
        newline,
        &bindings,
        &mut edits,
    )?;

    // The exact sentinel is generator-owned; ordinary user-written tests are never rewritten.
    let marker = format!("{EMPTY_MARKER}{newline}");
    let sentinel = program.body.iter().find_map(|statement| {
        let span = statement.span();
        source[..span.start as usize]
            .ends_with(&marker)
            .then_some(span)
    });
    if let Some(span) = sentinel {
        if !desired.children.is_empty() {
            let (_, end) = whole_line(source, span.start as usize, span.end as usize);
            edits.push(Edit {
                start: span.start as usize - marker.len(),
                end,
                text: String::new(),
            });
        }
    } else if desired.children.is_empty() && source.starts_with(HEADER.trim_end()) {
        let text = format!(
            "{marker}{}.todo(\"No callable declarations in this file\");{newline}",
            bindings.it
        );
        edits.push(Edit {
            start: source.len(),
            end: source.len(),
            text,
        });
    }
    if edits.iter().any(|edit| !edit.text.is_empty())
        && let Some(import) = &bindings.import
    {
        // Insert after directives/hashbangs and before statements; don't rewrite existing imports.
        let position = program
            .body
            .first()
            .map_or(source.len(), |statement| statement.span().start as usize);
        edits.push(Edit {
            start: position,
            end: position,
            text: format!("{import}{newline}"),
        });
    }
    edits.sort_by_key(|edit| (edit.start, edit.end));
    for pair in edits.windows(2) {
        ensure!(
            pair[0].end <= pair[1].start,
            "Overlapping synchronization edits"
        );
    }
    let mut result = source.to_owned();
    for edit in edits.into_iter().rev() {
        result.replace_range(edit.start..edit.end, &edit.text);
    }
    // Validate before returning, so the caller never writes a broken test file.
    let program = parse(&allocator, &result, false).context("Generated test is invalid")?;
    let bindings = Bindings::from_program(&program, &result);
    let existing = suites(&program.body, desired, desired, &bindings)?;
    let mut spacing = Vec::new();
    separate_suites(&result, &existing, newline, &mut spacing);
    spacing.sort_by_key(|edit| edit.start);
    for edit in spacing.into_iter().rev() {
        result.insert_str(edit.start, &edit.text);
    }
    Ok(result)
}

fn separate_suites(
    source: &str,
    suites: &BTreeMap<String, Existing>,
    newline: &str,
    edits: &mut Vec<Edit>,
) {
    let mut ordered: Vec<_> = suites.values().collect();
    ordered.sort_by_key(|suite| suite.span.start);
    for pair in ordered.windows(2) {
        let end = pair[0].span.end as usize;
        let gap = &source[end..pair[1].span.start as usize];
        let lines = gap.bytes().filter(|&byte| byte == b'\n').count();
        if gap.trim().is_empty() && lines < 2 {
            edits.push(Edit {
                start: end,
                end,
                text: newline.repeat(2 - lines),
            });
        }
    }
    for suite in ordered {
        separate_suites(source, &suite.children, newline, edits);
    }
}

pub(crate) struct Existing {
    span: Span,
    open: usize,
    close: usize,
    pub pending: Option<u32>,
    pub children: BTreeMap<String, Existing>,
}

struct Edit {
    start: usize,
    end: usize,
    text: String,
}

pub(crate) struct Bindings {
    names: BTreeSet<String>,
    tests: BTreeSet<String>,
    namespaces: BTreeSet<String>,
    describe: String,
    it: String,
    import: Option<String>,
}

impl Bindings {
    pub(crate) fn from_program(program: &Program<'_>, source: &str) -> Self {
        let mut bindings = Self {
            names: BTreeSet::from(["describe".into(), "suite".into()]),
            tests: BTreeSet::from(["it".into(), "test".into()]),
            namespaces: BTreeSet::new(),
            describe: String::new(),
            it: String::new(),
            import: None,
        };
        for statement in &program.body {
            let Statement::ImportDeclaration(import) = statement else {
                continue;
            };
            if import.source.value != "vitest" || import.import_kind == ImportOrExportKind::Type {
                continue;
            }
            for specifier in import.specifiers.iter().flatten() {
                match specifier {
                    ImportDeclarationSpecifier::ImportSpecifier(specifier)
                        if specifier.import_kind != ImportOrExportKind::Type =>
                    {
                        let name = specifier.imported.name();
                        let local = specifier.local.name.to_string();
                        match name.as_str() {
                            "describe" | "suite" => {
                                bindings.names.insert(local.clone());
                                bindings.describe = local;
                            }
                            "it" | "test" => {
                                bindings.tests.insert(local.clone());
                                bindings.it = local;
                            }
                            _ => {}
                        }
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(specifier) => {
                        let name = specifier.local.name.to_string();
                        bindings.namespaces.insert(name.clone());
                        bindings.describe = format!("{name}.describe");
                        bindings.it = format!("{name}.it");
                    }
                    _ => {}
                }
            }
        }
        if bindings.describe.is_empty() || bindings.it.is_empty() {
            let mut name = "__errsVitest".to_owned();
            while source.contains(&name) {
                name.push('_');
            }
            if bindings.describe.is_empty() {
                bindings.describe = format!("{name}.describe");
            }
            if bindings.it.is_empty() {
                bindings.it = format!("{name}.it");
            }
            bindings.import = Some(format!("import * as {name} from \"vitest\";"));
        }
        bindings
    }

    fn is_describe(&self, expression: &Expression<'_>) -> bool {
        self.is_binding(expression, &self.names, &["describe", "suite"])
    }

    fn is_test(&self, expression: &Expression<'_>) -> bool {
        self.is_binding(expression, &self.tests, &["it", "test"])
    }

    fn is_binding(
        &self,
        expression: &Expression<'_>,
        names: &BTreeSet<String>,
        exports: &[&str],
    ) -> bool {
        match expression {
            Expression::Identifier(id) => names.contains(id.name.as_str()),
            Expression::StaticMemberExpression(member) => {
                if let Expression::Identifier(id) = &member.object
                    && self.namespaces.contains(id.name.as_str())
                {
                    return exports.contains(&member.property.name.as_str());
                }
                self.is_binding(&member.object, names, exports)
            }
            Expression::CallExpression(call) => self.is_binding(&call.callee, names, exports),
            Expression::TaggedTemplateExpression(tag) => self.is_binding(&tag.tag, names, exports),
            _ => false,
        }
    }
}

pub(crate) fn suites(
    statements: &[Statement<'_>],
    previous: &Suite,
    desired: &Suite,
    bindings: &Bindings,
) -> Result<BTreeMap<String, Existing>> {
    let mut result = BTreeMap::new();
    if previous.children.is_empty() && desired.children.is_empty() {
        return Ok(result);
    }
    for statement in statements {
        let Statement::ExpressionStatement(statement) = statement else {
            continue;
        };
        let Expression::CallExpression(call) = &statement.expression else {
            continue;
        };
        if !bindings.is_describe(&call.callee) {
            continue;
        }
        let Some(Argument::StringLiteral(name)) = call.arguments.first() else {
            bail!(
                "Describe names must be string literals for synchronization (offset {})",
                call.span.start
            );
        };
        if !previous.children.contains_key(name.value.as_str())
            && !desired.children.contains_key(name.value.as_str())
        {
            continue;
        }
        // Vitest accepts describe(name, options, callback) as well as (name, callback).
        let callback = if matches!(call.arguments.get(1), Some(Argument::ObjectExpression(_))) {
            call.arguments.get(2)
        } else {
            call.arguments.get(1)
        };
        let body = match callback {
            Some(Argument::ArrowFunctionExpression(function)) => match &function.body {
                ArrowFunctionBody::FunctionBody(body) => Some(&**body),
                _ => None,
            },
            Some(Argument::FunctionExpression(function)) => function.body.as_deref(),
            _ => None,
        }
        .with_context(|| {
            format!(
                "describe({:?}) must have an inline block callback",
                name.value
            )
        })?;
        let existing = Existing {
            span: statement.span,
            open: body.span.start as usize,
            close: body.span.end as usize - 1,
            pending: body.statements.iter().find_map(|statement| {
                let Statement::ExpressionStatement(statement) = statement else {
                    return None;
                };
                let Expression::CallExpression(call) = &statement.expression else {
                    return None;
                };
                (bindings.is_test(&call.callee)
                    && matches!(call.arguments.first(), Some(Argument::StringLiteral(name)) if name.value == "TODO"))
                .then_some(call.span.start)
            }),
            children: suites(
                &body.statements,
                previous
                    .children
                    .get(name.value.as_str())
                    .unwrap_or(&Suite::default()),
                desired
                    .children
                    .get(name.value.as_str())
                    .unwrap_or(&Suite::default()),
                bindings,
            )?,
        };
        ensure!(
            result.insert(name.value.to_string(), existing).is_none(),
            "Duplicate describe({:?}) in one scope; cannot synchronize unambiguously",
            name.value
        );
    }
    Ok(result)
}

#[allow(clippy::too_many_arguments)]
fn sync_level(
    source: &str,
    existing: &BTreeMap<String, Existing>,
    previous: &Suite,
    desired: &Suite,
    close: usize,
    indent: &str,
    newline: &str,
    bindings: &Bindings,
    edits: &mut Vec<Edit>,
) -> Result<()> {
    for name in previous
        .children
        .keys()
        .filter(|name| !desired.children.contains_key(*name))
    {
        if let Some(old) = existing.get(name) {
            let (start, end) = whole_line(source, old.span.start as usize, old.span.end as usize);
            edits.push(Edit {
                start,
                end,
                text: String::new(),
            });
        }
    }
    let mut additions = String::new();
    for (name, child) in &desired.children {
        if let Some(old) = existing.get(name) {
            let node_indent = line_indent(source, old.span.start as usize);
            let child_indent = format!("{node_indent}  ");
            sync_level(
                source,
                &old.children,
                previous.children.get(name).unwrap_or(&Suite::default()),
                child,
                old.close,
                &child_indent,
                newline,
                bindings,
                edits,
            )?;
        } else {
            additions.push_str(&render(name, child, indent, newline, bindings));
        }
    }
    if !additions.is_empty() {
        let line_start = source[..close].rfind('\n').map_or(0, |pos| pos + 1);
        let pos = if source[line_start..close].trim().is_empty() {
            line_start
        } else {
            close
        };
        let mut text = String::new();
        if pos > 0 && !source[..pos].ends_with('\n') {
            text.push_str(newline);
        }
        text.push_str(&additions);
        if pos == close && close < source.len() {
            text.push_str(indent.strip_suffix("  ").unwrap_or(""));
        }
        edits.push(Edit {
            start: pos,
            end: pos,
            text,
        });
    }
    Ok(())
}

fn render(name: &str, suite: &Suite, indent: &str, newline: &str, bindings: &Bindings) -> String {
    let name = serde_json::to_string(name).expect("string serialization");
    let mut text = format!("{indent}{}({name}, () => {{{newline}", bindings.describe);
    let child_indent = format!("{indent}  ");
    if suite.callable {
        text.push_str(&format!("{child_indent}{}(\"TODO\", () => {{{newline}{child_indent}  // TODO{newline}{child_indent}}});{newline}", bindings.it));
    }
    for (name, child) in &suite.children {
        text.push_str(&render(name, child, &child_indent, newline, bindings));
    }
    text.push_str(&format!("{indent}}});{newline}"));
    text
}

fn line_indent(source: &str, pos: usize) -> &str {
    let start = source[..pos].rfind('\n').map_or(0, |pos| pos + 1);
    let line = &source[start..pos];
    &line[..line.len() - line.trim_start_matches([' ', '\t']).len()]
}

fn whole_line(source: &str, start: usize, end: usize) -> (usize, usize) {
    let line_start = source[..start].rfind('\n').map_or(0, |pos| pos + 1);
    let line_end = source[end..]
        .find('\n')
        .map_or(source.len(), |pos| end + pos + 1);
    if source[line_start..start].trim().is_empty() && source[end..line_end].trim().is_empty() {
        (line_start, line_end)
    } else {
        (start, end)
    }
}
