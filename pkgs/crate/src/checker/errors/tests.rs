use super::*;
use tempfile::{TempDir, tempdir};

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
            assert!(module.diagnostics.is_empty(), "{:?}", module.diagnostics);
        }
    }
    project.check_files().await.unwrap();
    (dir, project)
}

fn result(project: &EscProject) -> &EscProjectStateChecked {
    let EscProjectState::Checked(checked) = &project.state else {
        panic!("Expected checked state")
    };
    checked
}

fn errors(project: &EscProject, module: &str, name: &str) -> Types {
    let checked = result(project);
    let graph = &checked.call_graph.graph;
    let node = graph
        .node_indices()
        .find(|&node| {
            graph[node].module_id.as_str() == module && graph[node].name.as_deref() == Some(name)
        })
        .unwrap();
    checked.errors[&EscFnId::new(graph[node].module_id.clone(), node)].clone()
}

fn builtins(names: &[&'static str]) -> Types {
    names
        .iter()
        .map(|&name| EscErrorType::builtin(name))
        .collect()
}

#[tokio::test]
async fn applies_paired_declarations_to_esm_and_cjs_implementations() {
    let (_dir, project) = checked(&[
        ("entry.ts", "import { hash } from 'paired'; const cjs = require('paired'); export function call(input: Uint8Array) { return hash(input); }"),
        ("node_modules/paired/package.json", r#"{"name":"paired","type":"module","exports":{"import":{"types":"./types/index.d.ts","default":"./esm/index.js"},"require":{"types":"./types/index.d.cts","default":"./cjs/index.cjs"}}}"#),
        ("node_modules/paired/esm/index.js", "export { implementation as hash } from './body.js';"),
        ("node_modules/paired/esm/body.js", "export function implementation(bytes, salt = 0) { const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength); const length = bytes.length; return view.getUint32(0) + salt + bytes[length - 1]; } export function unrelated(value) { return value >>> 0; }"),
        ("node_modules/paired/cjs/index.cjs", "const body = require('./body.cjs'); exports.hash = body.implementation;"),
        ("node_modules/paired/cjs/body.cjs", "function implementation(bytes, salt = 0) { const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength); return view.getUint32(0) + salt + bytes[0]; } exports.implementation = implementation;"),
        ("node_modules/paired/types/index.d.ts", "export { signature as hash } from './body.js';"),
        ("node_modules/paired/types/body.d.ts", "export declare function signature(input: Uint8Array, seed?: number): number;"),
        ("node_modules/paired/types/index.d.cts", "export { signature as hash } from './body.cjs';"),
        ("node_modules/paired/types/body.d.cts", "export declare function signature(input: Uint8Array, seed?: number): number;"),
    ]).await;
    for module in [
        "node_modules/paired/esm/body.js",
        "node_modules/paired/cjs/body.cjs",
    ] {
        assert_eq!(
            errors(&project, module, "implementation"),
            builtins(&["RangeError", "TypeError"]),
            "{module}"
        );
    }
    assert_eq!(
        errors(&project, "entry.ts", "call"),
        builtins(&["RangeError", "TypeError"])
    );
    assert_eq!(
        errors(&project, "node_modules/paired/esm/body.js", "unrelated"),
        builtins(&["unknown"])
    );
}

#[tokio::test]
async fn declaration_types_use_the_declaration_module_and_union_overloads() {
    let (_dir, project) = checked(&[
        ("entry.ts", "import { read, overloaded } from 'paired'; export function entry() { return read(1); }"),
        ("node_modules/paired/package.json", r#"{"name":"paired","type":"module","main":"index.js","types":"index.d.ts"}"#),
        ("node_modules/paired/index.js", "export function read(value = 1) { throw value; } export function overloaded(value) { throw value; }"),
        ("node_modules/paired/index.d.ts", "import type { Numeric } from './types.js'; export declare function read(input?: Numeric): never; export declare function overloaded(input: number): never; export declare function overloaded(input: string): never;"),
        ("node_modules/paired/types.d.ts", "export type Numeric = number;"),
    ]).await;
    assert_eq!(
        errors(&project, "node_modules/paired/index.js", "read"),
        builtins(&["number"])
    );
    assert_eq!(
        errors(&project, "node_modules/paired/index.js", "overloaded"),
        builtins(&["number", "string"])
    );
}

#[tokio::test]
async fn infers_local_arguments_and_preserves_open_world_calls() {
    let (_dir, project) = checked(&[("entry.ts", r#"
        const prime = 2246822519;
        export function entry(n: number) { return mix(n >>> 0); }
        function mix(hash) { hash ^= hash >>> 15; hash = Math.imul(hash, prime) >>> 0; return rotate(hash, 13); }
        function rotate(value, shift) { return (value << shift) | (value >>> (32 - shift)); }
        export function another() { return mix(123); }
        export function unknownEntry(value: unknown) { return uncertain(value); }
        function uncertain(value) { return value >>> 0; }
        export function escaped(value) { return value >>> 0; }
        export const callback = saved;
        function saved(value) { return value >>> 0; }
        export function numericSaved() { return saved(1); }
        export function recurEntry(n: number) { return recur(n); }
        function recur(n) { if (n <= 0) return 0; return recur(n - 1); }
        export function omitted() { return defaults(); }
        function defaults(n = 1) { return n >>> 0; }
        export function mixedEntry(value: unknown) { both(1); return both(value); }
        function both(n) { return n >>> 0; }
    "#)]).await;
    for name in [
        "entry",
        "mix",
        "rotate",
        "another",
        "recur",
        "recurEntry",
        "defaults",
        "omitted",
    ] {
        assert!(errors(&project, "entry.ts", name).is_empty(), "{name}");
    }
    for name in [
        "unknownEntry",
        "uncertain",
        "escaped",
        "saved",
        "numericSaved",
        "mixedEntry",
        "both",
    ] {
        assert!(
            errors(&project, "entry.ts", name).contains(&EscErrorType::UNKNOWN),
            "{name}"
        );
    }
}

#[tokio::test]
async fn distinguishes_unresolved_names_from_undefined_values() {
    let (_dir, project) = checked(&[(
        "entry.ts",
        r#"
        const prime = 2246822519;
        const absentValue = undefined;
        function known() { return Math.imul(1, prime); }
        function undefinedValue() { return Math.imul(1, absentValue); }
        function missing() { return xxh32PrimeMissing; }
        function missingCoercion() { return Math.imul(1, xxh32PrimeMissing); }
        function probe() { return typeof xxh32PrimeMissing; }
        function caught() { try { return xxh32PrimeMissing; } catch {} }
    "#,
    )])
    .await;
    for name in ["known", "undefinedValue", "probe", "caught"] {
        assert!(errors(&project, "entry.ts", name).is_empty(), "{name}");
    }
    assert_eq!(
        errors(&project, "entry.ts", "missing"),
        builtins(&["ReferenceError"])
    );
    assert_eq!(
        errors(&project, "entry.ts", "missingCoercion"),
        builtins(&["ReferenceError", "unknown"])
    );
}

#[tokio::test]
async fn follows_global_constructor_instances_to_prototype_methods() {
    let (_dir, project) = checked(&[("entry.ts", r#"
        function direct(buffer: unknown) { const view = new DataView(buffer); return view.getUint32(0, true); }
        function make(buffer: unknown) { return new DataView(buffer); }
        function returned(buffer: unknown) { const view = make(buffer); const alias = view; return alias.getUint32(0); }
        function caught(buffer: unknown) { try { const view = new DataView(buffer); view.getUint32(0); } catch {} }
        function shadow(DataView: unknown) { const view = new DataView(); view.getUint32(0); }
        function mixed(buffer: unknown, flag: boolean, other: unknown) { const view = flag ? new DataView(buffer) : other; view.getUint32(0); }
        function customOffset(buffer: unknown, offset: unknown) { new DataView(buffer).getUint32(offset); }
        function typed(view: DataView) { return view.getUint32(0); }
        function returnType(view: DataView) { try { throw view.getUint32(0); } catch (e) { throw e; } }
    "#)]).await;
    for name in ["direct", "make", "returned", "typed"] {
        assert_eq!(
            errors(&project, "entry.ts", name),
            builtins(&["RangeError", "TypeError"])
        );
    }
    assert!(errors(&project, "entry.ts", "caught").is_empty());
    assert_eq!(
        errors(&project, "entry.ts", "returnType"),
        builtins(&["RangeError", "TypeError", "number"])
    );
    assert_eq!(
        errors(&project, "entry.ts", "shadow"),
        builtins(&["unknown"])
    );
    for name in ["mixed", "customOffset"] {
        assert_eq!(
            errors(&project, "entry.ts", name),
            builtins(&["RangeError", "TypeError", "unknown"])
        );
    }
}

#[tokio::test]
async fn resolves_global_effects_and_shadowing() {
    let (_dir, project) = checked(&[(
        "entry.ts",
        r#"
        function log() { console.log('ok'); }
        function parse() { const json = JSON; const parse = json.parse; parse('{}'); }
        function uri() { globalThis.decodeURIComponent('%'); }
        function shadow(console: unknown) { console.log('ok'); }
        function missing() { console.notKnown(); }
        function inspect(value: unknown) { console.log(value); }
        function construct() { new console.log(); }
        function caught() { try { JSON.parse('{}'); } catch {} }
    "#,
    )])
    .await;
    for name in ["log", "caught"] {
        assert!(errors(&project, "entry.ts", name).is_empty());
    }
    assert_eq!(
        errors(&project, "entry.ts", "parse"),
        builtins(&["SyntaxError"])
    );
    assert_eq!(errors(&project, "entry.ts", "uri"), builtins(&["URIError"]));
    assert_eq!(
        errors(&project, "entry.ts", "construct"),
        builtins(&["TypeError"])
    );
    for name in ["shadow", "missing", "inspect"] {
        assert_eq!(errors(&project, "entry.ts", name), builtins(&["unknown"]));
    }
    let (_dir, project) = checked(&[(
        "entry.ts",
        "console.log = replacement; function log() { console.log('ok'); }",
    )])
    .await;
    assert_eq!(errors(&project, "entry.ts", "log"), builtins(&["unknown"]));
}

#[tokio::test]
async fn resolves_recursive_interface_properties() {
    let (_dir, project) = checked(&[(
        "entry.ts",
        include_str!("../../../examples/recursion/src/bin.ts"),
    )])
    .await;
    for name in ["visitEvenLevel", "visitOddLevel"] {
        assert!(errors(&project, "entry.ts", name).is_empty());
    }
}

#[tokio::test]
async fn resolves_recursive_and_transitive_error_unions() {
    let (_dir, project) = checked(&[
        ("entry.ts", "import { b } from './b'; export function a(flag) { if (flag) throw new TypeError(); b(flag); } function c() { d(); } function d() { e(); } function e() { throw new RangeError(); } function empty() {}"),
        ("b.ts", "import { a } from './entry'; export function b(flag) { if (flag) throw new SyntaxError(); a(flag); }"),
    ]).await;
    assert_eq!(
        errors(&project, "entry.ts", "a"),
        builtins(&["TypeError", "SyntaxError"])
    );
    assert_eq!(
        errors(&project, "b.ts", "b"),
        builtins(&["TypeError", "SyntaxError"])
    );
    for name in ["c", "d", "e"] {
        assert_eq!(
            errors(&project, "entry.ts", name),
            builtins(&["RangeError"])
        );
    }
    assert!(errors(&project, "entry.ts", "empty").is_empty());
}

#[tokio::test]
async fn catches_can_give_recursive_members_different_summaries() {
    let (_dir, project) = checked(&[("entry.ts", r#"
        function a(flag) { if (flag) throw new TypeError(); try { b(flag); } catch {} }
        function b(flag) { if (flag) throw new RangeError(); a(flag); }
        function rethrow() { try { throw new SyntaxError(); } catch (e) { const alias = e; throw alias; } }
        function replace() { try { throw new Error(); } catch { throw new URIError(); } }
        function unreachableCatch() { try {} catch { throw new Error(); } }
    "#)]).await;
    assert_eq!(errors(&project, "entry.ts", "a"), builtins(&["TypeError"]));
    assert_eq!(
        errors(&project, "entry.ts", "b"),
        builtins(&["TypeError", "RangeError"])
    );
    assert_eq!(
        errors(&project, "entry.ts", "rethrow"),
        builtins(&["SyntaxError"])
    );
    assert_eq!(
        errors(&project, "entry.ts", "replace"),
        builtins(&["URIError"])
    );
    assert!(errors(&project, "entry.ts", "unreachableCatch").is_empty());
}

#[tokio::test]
async fn finally_preserves_or_replaces_each_pending_completion() {
    let (_dir, project) = checked(&[("entry.ts", r#"
        function suppress() { try { throw new Error(); } finally { return; } }
        function replace() { try { throw new Error(); } finally { throw new TypeError(); } }
        function preserve() { try { throw new RangeError(); } finally {} }
        function conditional(flag) { try { throw new SyntaxError(); } finally { if (flag) return; } }
        function returnThenThrow() { try { return; } finally { throw new URIError(); } }
        function catchThrows() { try { throw new Error(); } catch { throw new TypeError(); } finally {} }
        function breakSuppresses() { while (true) { try { throw new Error(); } finally { break; } } }
        function unreachable() { return; throw new Error(); }
        function diverge() { diverge(); throw new Error(); }
    "#)]).await;
    for name in ["suppress", "breakSuppresses", "unreachable", "diverge"] {
        assert!(errors(&project, "entry.ts", name).is_empty(), "{name}");
    }
    for (name, ty) in [
        ("replace", "TypeError"),
        ("preserve", "RangeError"),
        ("conditional", "SyntaxError"),
        ("returnThenThrow", "URIError"),
        ("catchThrows", "TypeError"),
    ] {
        assert_eq!(
            errors(&project, "entry.ts", name),
            builtins(&[ty]),
            "{name}"
        );
    }
}

#[tokio::test]
async fn resolves_cross_module_custom_errors_and_instanceof_filters() {
    let (_dir, project) = checked(&[
        ("entry.ts", r#"
            import { Custom as Renamed } from './barrel';
            function throwsCustom() { throw new Renamed(); }
            function filtered(flag) {
                try { if (flag) throw new TypeError(); throw new RangeError(); }
                catch (e) { if (e instanceof TypeError) return; throw e; }
            }
            function catchesBase() { try { throwsCustom(); } catch (e) { if (e instanceof Error) return; throw e; } }
            function catchesCustom() { try { throwsCustom(); } catch (e) { if (!(e instanceof Renamed)) throw e; } }
        "#),
        ("barrel.ts", "export { Custom } from './custom';"),
        ("custom.ts", "export class Custom extends Error {}"),
    ]).await;
    let types = errors(&project, "entry.ts", "throwsCustom");
    assert_eq!(types.len(), 1);
    let EscErrorType::Node(id) = types.iter().next().unwrap() else {
        panic!("Expected custom error")
    };
    assert_eq!(id.module_id.as_str(), "custom.ts");
    result(&project).checked_files[&id.module_id].with_semantic(|result| {
        assert!(matches!(
            result.semantic.nodes().kind(id.node),
            AstKind::Class(_)
        ));
    });
    assert_eq!(
        errors(&project, "entry.ts", "filtered"),
        builtins(&["RangeError"])
    );
    assert!(errors(&project, "entry.ts", "catchesBase").is_empty());
    assert!(errors(&project, "entry.ts", "catchesCustom").is_empty());
}

#[tokio::test]
async fn evaluates_values_arguments_defaults_and_constructor_bodies() {
    let (_dir, project) = checked(&[(
        "entry.ts",
        r#"
        function make() { return new TypeError(); }
        function returned() { throw make(); }
        function argument() { throw new RangeError(); }
        function consume(value) {}
        function argumentsFirst() { consume(argument()); throw new Error(); }
        function defaults(value = argument()) {}
        class Failing extends Error { constructor() { super(); throw new SyntaxError(); } }
        function constructing() { throw new Failing(); }
        function literals(flag) { if (flag) throw 'bad'; throw 42; }
        function nested() { function unused() { throw new Error(); } }
        function dynamic(callback) { callback(); }
        function shadow(Error) { throw new Error(); }
    "#,
    )])
    .await;
    for (name, types) in [
        ("returned", &["TypeError"][..]),
        ("argumentsFirst", &["RangeError"]),
        ("defaults", &["RangeError"]),
        ("constructing", &["SyntaxError"]),
        ("literals", &["string", "number"]),
        ("dynamic", &["unknown"]),
        ("shadow", &["unknown"]),
    ] {
        assert_eq!(
            errors(&project, "entry.ts", name),
            builtins(types),
            "{name}"
        );
    }
    assert!(errors(&project, "entry.ts", "nested").is_empty());
}

#[tokio::test]
async fn distinguishes_async_rejections_from_synchronous_calls() {
    let (_dir, project) = checked(&[(
        "entry.ts",
        r#"
        async function rejects() { throw new TypeError(); }
        function invoke() { rejects(); }
        async function awaited() { await rejects(); }
        async function swallowed() { try { await rejects(); } catch {} }
        async function returned() { return rejects(); }
        async function uncaughtReturn() { try { return rejects(); } catch { return; } }
    "#,
    )])
    .await;
    for name in ["rejects", "awaited", "returned", "uncaughtReturn"] {
        assert_eq!(
            errors(&project, "entry.ts", name),
            builtins(&["TypeError"]),
            "{name}"
        );
    }
    for name in ["invoke", "swallowed"] {
        assert!(errors(&project, "entry.ts", name).is_empty(), "{name}");
    }
}

#[tokio::test]
async fn preserves_loop_completions_and_reanalyzes_changed_value_types() {
    let (_dir, project) = checked(&[("entry.ts", r#"
        function neverEnter() { while (false) { throw new Error(); } }
        function doOnce() { do { throw new TypeError(); } while (false); }
        function continueSuppresses() { outer: while (true) { try { throw new Error(); } finally { continue outer; } } }
        function breakOverridden() { while (true) { try { break; } finally { throw new RangeError(); } } }
        function iterate(flag: boolean) {
            let error = new TypeError();
            while (flag) { error = new RangeError(); }
            throw error;
        }
        function finalValue() {
            let error = new TypeError();
            try { return; } finally { error = new RangeError(); throw error; }
        }
        function choices(value: number) { switch (value) { case 0: throw new TypeError(); default: throw new RangeError(); } }
        function counter(n: number) { for (let i = 0; i < n; i++) { if (i === 0) throw new SyntaxError(); } }
        function shortCircuit() { false && fail(); true || fail(); }
        function fail() { throw new Error(); }
    "#)]).await;
    for name in ["neverEnter", "continueSuppresses", "shortCircuit"] {
        assert!(errors(&project, "entry.ts", name).is_empty(), "{name}");
    }
    for (name, types) in [
        ("doOnce", &["TypeError"][..]),
        ("breakOverridden", &["RangeError"]),
        ("iterate", &["TypeError", "RangeError"]),
        ("finalValue", &["RangeError"]),
        ("choices", &["TypeError", "RangeError"]),
        ("counter", &["SyntaxError"]),
    ] {
        assert_eq!(
            errors(&project, "entry.ts", name),
            builtins(types),
            "{name}"
        );
    }
}

#[tokio::test]
async fn uses_parameter_annotations_and_retains_uncertainty_for_side_effects() {
    let (_dir, project) = checked(&[(
        "entry.ts",
        r#"
        class Custom extends Error {}
        function typed(error: Custom) { throw error; }
        function primitive(error: string | number) { throw error; }
        function numeric(n: number) { if (n <= 0) throw new TypeError(); numeric(n - 1); }
        function effects() {
            let error = new TypeError();
            function mutate() { error = new RangeError(); }
            mutate();
            throw error;
        }
        function getter(object) { object.value; }
    "#,
    )])
    .await;
    let types = errors(&project, "entry.ts", "typed");
    assert_eq!(types.len(), 1);
    assert!(matches!(types.iter().next(), Some(EscErrorType::Node(_))));
    assert_eq!(
        errors(&project, "entry.ts", "primitive"),
        builtins(&["string", "number"])
    );
    assert_eq!(
        errors(&project, "entry.ts", "numeric"),
        builtins(&["TypeError"])
    );
    assert!(errors(&project, "entry.ts", "effects").contains(&EscErrorType::UNKNOWN));
    assert!(errors(&project, "entry.ts", "getter").contains(&EscErrorType::UNKNOWN));
}

#[tokio::test]
async fn does_not_assume_constructor_coercions_or_fields_are_error_free() {
    let (_dir, project) = checked(&[(
        "entry.ts",
        r#"
        function coercion(message) { throw new Error(message); }
        function aggregate() { new AggregateError(); }
        class WithField extends Error { field = external(); }
        function fields() { throw new WithField(); }
        function Factory() { return new SyntaxError(); }
        function returnedInstance() { throw new Factory(); }
    "#,
    )])
    .await;
    assert_eq!(
        errors(&project, "entry.ts", "coercion"),
        builtins(&["Error", "unknown"])
    );
    assert_eq!(
        errors(&project, "entry.ts", "aggregate"),
        builtins(&["TypeError"])
    );
    assert!(errors(&project, "entry.ts", "fields").contains(&EscErrorType::UNKNOWN));
    assert_eq!(
        errors(&project, "entry.ts", "returnedInstance"),
        builtins(&["SyntaxError"])
    );
}
