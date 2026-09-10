use crate::prelude::*;

impl EscProject {
    pub async fn check_files(&mut self) -> Result<()> {
        let parsed = match &self.state {
            EscProjectState::Resolved => bail!("Project files must be parsed before checking"),
            EscProjectState::Parsed(parsed) => parsed,
            EscProjectState::Checked(_) => return Ok(()),
        };
        let call_graph = EscCallGraph::build(parsed, &self.repo_path)?;

        // Build successfully before moving the modules so errors preserve Parsed state.
        let EscProjectState::Parsed(parsed) =
            std::mem::replace(&mut self.state, EscProjectState::Resolved)
        else {
            unreachable!();
        };
        self.state = EscProjectState::Checked(EscProjectStateChecked {
            checked_files: parsed.parsed_files,
            call_graph,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxc_ast::AstKind;
    use petgraph::visit::EdgeRef;
    use tempfile::{TempDir, tempdir};

    #[tokio::test]
    async fn groups_cross_module_recursion_and_orders_dependency_components() {
        let (_dir, mut project) = checked(&[
            ("entry.ts", "import { b } from './bridge'; export function a() { b(); } function c() { d(); } function d() { e(); } function e() {} function unused() {} function self() { self(); }"),
            ("bridge.ts", "export { b } from './b';"),
            ("b.ts", "import { a } from './entry'; export function b() { a(); }"),
        ]).await;
        let graph = &result(&project).call_graph;
        assert_eq!(graph.graph.node_count(), 7);
        assert_eq!(graph.sccs.len(), 6);
        assert!(graph.calls.iter().all(|call| !call.unresolved));
        let a = function(graph, "entry.ts", "a");
        let b = function(graph, "b.ts", "b");
        let component = graph.component_of[&a];
        assert_eq!(component, graph.component_of[&b]);
        assert_eq!(
            graph.sccs[component]
                .iter()
                .cloned()
                .collect::<HashSet<_>>(),
            HashSet::from([a, b])
        );
        let c = function(graph, "entry.ts", "c");
        let d = function(graph, "entry.ts", "d");
        let e = function(graph, "entry.ts", "e");
        assert!(graph.component_of[&e] < graph.component_of[&d]);
        assert!(graph.component_of[&d] < graph.component_of[&c]);
        let recursive = function(graph, "entry.ts", "self");
        assert!(
            graph
                .graph
                .find_edge(recursive.node(), recursive.node())
                .is_some()
        );

        // Every retained AST address remains valid in Checked state.
        for function in graph.graph.node_weights() {
            result(&project).checked_files[&function.module_id].with_semantic(|result| {
                assert!(matches!(
                    result.semantic.nodes().kind(function.node_id),
                    AstKind::Function(_) | AstKind::ArrowFunctionExpression(_)
                ));
            });
        }
        for edge in graph.graph.edge_references() {
            let caller = EscFnId::new(graph.graph[edge.source()].module_id.clone(), edge.source());
            let callee = EscFnId::new(graph.graph[edge.target()].module_id.clone(), edge.target());
            assert!(graph.component_of[&callee] <= graph.component_of[&caller]);
        }
        // Checking twice retains the same graph rather than dropping the modules.
        project.check_files().await.unwrap();
        assert_eq!(result(&project).call_graph.graph.node_count(), 7);
    }

    #[tokio::test]
    async fn resolves_aliases_namespaces_default_exports_and_cyclic_barrels() {
        let (_dir, project) = checked(&[
            ("entry.ts", "import run, * as api from './barrel'; const alias = run; export const start = () => { alias(); api['other'](); };"),
            ("barrel.ts", "export { default } from './worker'; export * from './cycle';"),
            ("cycle.ts", "export * from './barrel'; export { other } from './worker';"),
            ("worker.ts", "import { start } from './entry'; export default function run() { start(); } export function other() {}"),
        ]).await;
        let graph = &result(&project).call_graph;
        let start = function(graph, "entry.ts", "start");
        let run = function(graph, "worker.ts", "run");
        let other = function(graph, "worker.ts", "other");
        assert_eq!(graph.component_of[&start], graph.component_of[&run]);
        assert!(graph.component_of[&other] < graph.component_of[&start]);
        assert!(graph.graph.find_edge(start.node(), other.node()).is_some());
        assert!(graph.calls.iter().all(|call| !call.unresolved));
    }

    #[tokio::test]
    async fn keeps_nested_bodies_separate_and_records_dynamic_calls() {
        let (_dir, project) = checked(&[(
            "entry.ts",
            r#"
            function target() {}
            function outer(target) {
                function inner() { outer(target); }
                target();
                unknownObject.method();
            }
            const left = () => {};
            const right = function named() {};
            const choice = flag ? left : right;
            function choose() { choice(); }
            function invoke() { (function immediate() { left(); })(); }
            class Example { method() {} }
            const object = { method() {} };
        "#,
        )])
        .await;
        let graph = &result(&project).call_graph;
        let outer = function(graph, "entry.ts", "outer");
        let inner = function(graph, "entry.ts", "inner");
        let target = function(graph, "entry.ts", "target");
        assert!(graph.graph.find_edge(inner.node(), outer.node()).is_some());
        assert!(graph.graph.find_edge(outer.node(), inner.node()).is_none());
        assert!(graph.graph.find_edge(outer.node(), target.node()).is_none());
        assert_eq!(
            graph
                .calls
                .iter()
                .filter(|call| call.caller.as_ref() == Some(&outer) && call.unresolved)
                .count(),
            2
        );
        let choose = function(graph, "entry.ts", "choose");
        let choice = graph
            .calls
            .iter()
            .find(|call| call.caller.as_ref() == Some(&choose))
            .unwrap();
        assert_eq!(choice.targets.len(), 2);
        assert!(!choice.unresolved);
        let invoke = function(graph, "entry.ts", "invoke");
        let immediate = function(graph, "entry.ts", "immediate");
        assert!(
            graph
                .graph
                .find_edge(invoke.node(), immediate.node())
                .is_some()
        );
        // Includes both otherwise uncalled method bodies.
        assert_eq!(graph.graph.node_count(), 10);
    }

    #[tokio::test]
    async fn preserves_handler_context_without_collapsing_functions() {
        let (_dir, project) = checked(&[(
            "entry.ts",
            "function a() { try { b(); } catch (err) {} } function b() { a(); }",
        )])
        .await;
        let checked = result(&project);
        let graph = &checked.call_graph;
        let a = function(graph, "entry.ts", "a");
        let b = function(graph, "entry.ts", "b");
        assert_eq!(graph.sccs.len(), 1);
        assert_eq!(graph.sccs[0].len(), 2);
        let edge = graph.graph.find_edge(a.node(), b.node()).unwrap();
        let site = &graph.graph[edge];
        checked.checked_files[&site.module_id].with_semantic(|result| {
            assert!(result.semantic.nodes().ancestor_kinds(site.node_id)
                .any(|kind| matches!(kind, AstKind::TryStatement(statement) if statement.handler.is_some())));
        });
    }

    #[tokio::test]
    async fn retains_uncertainty_for_mutations_external_calls_and_declarations() {
        let (_dir, project) = checked(&[(
            "entry.ts",
            r#"
            import { readFile } from 'node:fs';
            declare function ambient(): void;
            function original() {}
            let alias = original;
            alias = external;
            function caller() { alias(); readFile(); ambient(); }
        "#,
        )])
        .await;
        let graph = &result(&project).call_graph;
        assert_eq!(graph.graph.node_count(), 2);
        let original = function(graph, "entry.ts", "original");
        let caller = function(graph, "entry.ts", "caller");
        assert!(
            graph
                .graph
                .find_edge(caller.node(), original.node())
                .is_some()
        );
        assert_eq!(graph.calls.len(), 3);
        assert!(graph.calls.iter().all(|call| call.unresolved));
    }

    #[tokio::test]
    async fn handles_empty_graphs_and_requires_parsing() {
        let (dir, project) = checked(&[("entry.ts", "export const value = 1;")]).await;
        assert!(result(&project).call_graph.sccs.is_empty());

        let mut project = EscProject::resolve(Some(&dir.path().to_path_buf()))
            .await
            .unwrap();
        assert!(project.check_files().await.is_err());
        assert!(matches!(project.state, EscProjectState::Resolved));
    }

    #[tokio::test]
    async fn includes_function_assignments_and_keeps_namespace_exports_local() {
        let (_dir, project) = checked(&[
            (
                "entry.ts",
                r#"
                import { target } from './exports';
                let a;
                let b;
                a = function aBody() { b(); };
                b = function bBody() { a(); };
                function invoke() { target(); }
            "#,
            ),
            (
                "exports.ts",
                r#"
                export function target() {}
                namespace Nested { export function target() { missing(); } }
            "#,
            ),
        ])
        .await;
        let checked = result(&project);
        let graph = &checked.call_graph;
        let a = function(graph, "entry.ts", "aBody");
        let b = function(graph, "entry.ts", "bBody");
        assert_eq!(graph.component_of[&a], graph.component_of[&b]);
        let invoke = function(graph, "entry.ts", "invoke");
        let call = graph
            .calls
            .iter()
            .find(|call| call.caller.as_ref() == Some(&invoke))
            .unwrap();
        assert!(!call.unresolved);
        assert_eq!(call.targets.len(), 1);
        let target = &graph.graph[call.targets[0].node()];
        checked.checked_files[&target.module_id].with_semantic(|result| {
            let parent = result.semantic.nodes().parent_id(target.node_id);
            assert!(matches!(
                result.semantic.nodes().parent_kind(parent),
                AstKind::Program(_)
            ));
        });
    }

    async fn checked(fixtures: &[(&str, &str)]) -> (TempDir, EscProject) {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::write(dir.path().join("errconfig.toml"), "files = [\"entry.ts\"]").unwrap();
        for (path, source) in fixtures {
            let path = dir.path().join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, source).unwrap();
        }
        let mut project = EscProject::resolve(Some(&dir.path().to_path_buf()))
            .await
            .unwrap();
        project.parse_files().await.unwrap();
        if let EscProjectState::Parsed(parsed) = &project.state {
            for module in parsed.parsed_files.values() {
                assert!(!module.panicked);
                assert!(module.diagnostics.is_empty(), "{:?}", module.diagnostics);
            }
        }
        project.check_files().await.unwrap();
        (dir, project)
    }

    fn result(project: &EscProject) -> &EscProjectStateChecked {
        let EscProjectState::Checked(checked) = &project.state else {
            panic!("Expected checked state");
        };
        checked
    }

    fn function(graph: &EscCallGraph, module: &str, name: &str) -> EscFnId {
        let index = graph
            .graph
            .node_indices()
            .find(|&index| {
                let function = &graph.graph[index];
                function.module_id.as_str() == module && function.name.as_deref() == Some(name)
            })
            .unwrap_or_else(|| panic!("Missing function {module}:{name}"));
        EscFnId::new(graph.graph[index].module_id.clone(), index)
    }
}
