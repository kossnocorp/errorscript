use std::{fs, path::Path};

use errorscript_lib::{
    Options, PendingOptions, Suite, generate, next_pending, parse_declarations, synchronize,
};

fn api(source: &str) -> Suite {
    parse_declarations(source, false).unwrap()
}

fn at<'a>(tree: &'a Suite, path: &[&str]) -> &'a Suite {
    path.iter().fold(tree, |node, name| {
        node.children
            .get(*name)
            .unwrap_or_else(|| panic!("Missing {path:?}"))
    })
}

#[test]
fn declaration_inventory_handles_merges_overloads_namespaces_and_callable_shapes() {
    let tree = api(r#"
        interface TransitionEvent { init(): void; init(value: string): void; }
        declare var TransitionEvent: {
            prototype: TransitionEvent;
            new(type: string): TransitionEvent;
            new(type: string, init: object): TransitionEvent;
        };
        type Handler = (value: string) => void;
        type Alias = Handler;
        interface Factory { new(): object; }
        declare const factory: Factory;
        declare namespace Intl {
            function parse(value: string): void;
            interface Locale { [Symbol.iterator](): Iterator<string>; "quoted-key"(): void; onChange: Alias; }
        }
        declare class Example {
            constructor();
            static run(): void;
            run(): void;
            get value(): number;
            set value(value: number);
            private secret(): void;
            protected hidden(): void;
            #private(): void;
            callback: () => void;
        }
        declare function outer(callback: () => void): () => void;
        interface Data { number: number; text: string; }
        type CycleA = CycleB;
        type CycleB = CycleA;
    "#);
    assert_eq!(at(&tree, &["TransitionEvent"]).children.len(), 2);
    assert!(at(&tree, &["TransitionEvent", "new"]).callable);
    assert!(at(&tree, &["factory", "new"]).callable);
    assert!(at(&tree, &["Alias"]).callable);
    assert!(at(&tree, &["Intl", "Locale", "onChange"]).callable);
    assert!(at(&tree, &["Intl", "Locale", "[Symbol.iterator]"]).callable);
    assert!(at(&tree, &["Intl", "Locale", "quoted-key"]).callable);
    for name in [
        "new",
        "static run",
        "run",
        "get value",
        "set value",
        "callback",
    ] {
        assert!(at(&tree, &["Example", name]).callable);
    }
    assert_eq!(at(&tree, &["Example"]).children.len(), 6);
    assert!(at(&tree, &["outer"]).children.is_empty());
    assert!(!tree.children.contains_key("Data"));
    assert!(!tree.children.contains_key("CycleA"));
}

#[test]
fn node_modules_and_global_blocks_keep_their_hierarchy() {
    let tree = parse_declarations(
        r#"
        declare module "console" { export * from "node:console"; }
        declare module "node:console" {
            global {
                interface Console { log(...data: any[]): void; }
                namespace console { interface ConsoleConstructor { new(): Console; } }
            }
        }
        declare module "fs/promises" { function open(path: string): Promise<void>; }
    "#,
        true,
    )
    .unwrap();
    assert_eq!(tree.children.len(), 2);
    assert!(at(&tree, &["node:console", "global", "Console", "log"]).callable);
    assert!(
        at(
            &tree,
            &[
                "node:console",
                "global",
                "console",
                "ConsoleConstructor",
                "new"
            ]
        )
        .callable
    );
    assert!(at(&tree, &["node:fs/promises", "open"]).callable);
}

#[test]
fn sync_preserves_real_tests_hooks_helpers_comments_and_custom_suites() {
    let old = api("interface Example { keep(): void; removed(): void; }");
    let new = api("interface Example { keep(value: string): void; added(): void; }");
    let keep = r#"describe.skip('keep', function () {
    beforeEach(() => reset());
    // This test is the source of truth: punctuation, Unicode, and quotes matter.
    it('throws TypeError 🦀', () => {
      expect(() => new TransitionEvent(undefined)).toThrow(TypeError);
    });
    it.each([1, 2])('case %s', value => expect(value).toBeTruthy());
    describe("custom cases", () => { it("works", () => {}); });
  });"#;
    let original = format!(
        r#"import {{ describe, it, expect, beforeEach }} from "vitest";
const reset = () => {{}};
describe("Example", () => {{
  {keep}
  describe("removed", () => {{ it("retired", () => {{}}); }});
  describe("custom", () => {{ it("helper test", () => {{}}); }});
}});
"#
    );
    let result = synchronize(Some(&original), &old, &new).unwrap();
    assert!(result.contains(keep));
    assert!(result.contains("const reset = () => {};"));
    assert!(result.contains("helper test"));
    assert!(!result.contains("retired"));
    assert!(result.contains("describe(\"added\", () => {\n    it(\"TODO\""));
    assert_eq!(result, synchronize(Some(&result), &new, &new).unwrap());
}

#[test]
fn first_sync_adopts_existing_suites_and_retains_their_empty_bodies() {
    let desired = api("interface Event { new(): Event; other(): void; }");
    let existing =
        "describe(\"Event\", () => { describe(\"new\", () => { /* written later */ }); });\n";
    let result = synchronize(Some(existing), &Suite::default(), &desired).unwrap();
    assert!(result.contains("describe(\"new\", () => { /* written later */ });"));
    assert_eq!(result.matches("it(\"TODO\"").count(), 1);
    assert_eq!(
        result,
        synchronize(Some(&result), &desired, &desired).unwrap()
    );
}

#[test]
fn additions_and_deletions_preserve_crlf_and_overload_edits_are_noops() {
    let old = api("declare function keep(): void; declare function remove(): void;");
    let new = api(
        "declare function keep(): void; declare function keep(x: string): void; declare function add(): void;",
    );
    let original = synchronize(None, &Suite::default(), &old)
        .unwrap()
        .replace('\n', "\r\n");
    let result = synchronize(Some(&original), &old, &new).unwrap();
    assert!(!result.replace("\r\n", "").contains('\n'));
    assert!(!result.contains("\"remove\""));
    assert_eq!(result, synchronize(Some(&result), &new, &new).unwrap());
}

#[test]
fn parser_errors_and_ambiguous_suites_are_not_overwritten() {
    let desired = api("declare function api(): void;");
    for source in [
        "describe(",
        "describe('api', () => {}); describe('api', () => {});",
    ] {
        assert!(synchronize(Some(source), &Suite::default(), &desired).is_err());
    }
    assert!(parse_declarations("declare function (", false).is_err());
}

#[test]
fn vitest_aliases_namespaces_and_parameterized_suites_keep_existing_tests() {
    let old = api("interface Example { keep(): void; }");
    let new = api("interface Example { keep(): void; added(): void; }");
    for source in [
        "import { describe as group, test as check } from 'vitest';\ngroup.each([1, 2])('Example', () => { group('keep', () => { check('custom', () => {}); }); });\n",
        "import * as v from 'vitest';\nv.describe.skipIf(false)('Example', () => { v.describe('keep', () => { v.it('custom', () => {}); }); });\n",
        "import { describe, it } from 'vitest';\ndescribe('Example', { tags: ['browser-only'] }, () => { describe('keep', () => { it('custom', () => {}); }); });\n",
    ] {
        let result = synchronize(Some(source), &old, &new).unwrap();
        assert!(result.contains("'custom'"));
        assert_eq!(result.matches("'Example'").count(), 1);
        assert!(!result.contains("describe(\"Example\""));
        assert_eq!(result, synchronize(Some(&result), &new, &new).unwrap());
    }
}

#[test]
fn custom_nested_test_suites_are_opaque_and_sentinel_text_inside_tests_is_preserved() {
    let old = api("declare function keep(): void;");
    let new = api("declare function keep(): void; declare function added(): void;");
    let source = r#"import { describe, it } from 'vitest';
describe('keep', () => {
  describe('repeated', () => {});
  describe('repeated', () => {});
  describe(dynamicName, () => {});
  const fixture = `// @errs-lib:empty
it.todo("No callable declarations in this file");
`;
  it('keeps fixture', () => {});
});
"#;
    let result = synchronize(Some(source), &old, &new).unwrap();
    assert!(result.starts_with(source));
    assert_eq!(result, synchronize(Some(&result), &new, &new).unwrap());
}

#[test]
fn callable_container_can_gain_children_without_replacing_its_own_tests() {
    let old = api("declare function parse(): void;");
    let new =
        api("declare function parse(): void; declare namespace parse { function strict(): void; }");
    let source = synchronize(None, &Suite::default(), &old)
        .unwrap()
        .replace("\"TODO\"", "\"my behavior\"");
    let result = synchronize(Some(&source), &old, &new).unwrap();
    assert!(result.contains("it(\"my behavior\""));
    assert!(result.contains("describe(\"strict\""));
    let restored = synchronize(Some(&result), &new, &old).unwrap();
    assert_eq!(restored, source);
}

fn write(path: &Path, source: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn setup() -> (tempfile::TempDir, Options) {
    let directory = tempfile::tempdir().unwrap();
    let options = Options {
        ts_libs: directory.path().join("libs"),
        node_types: directory.path().join("node-types"),
        node_version: "24".into(),
        output: directory.path().join("tests"),
        check: false,
    };
    write(
        &options.ts_libs.join("lib.dom.d.ts"),
        "declare var TransitionEvent: { new(type: string): TransitionEvent; };",
    );
    write(
        &options.ts_libs.join("lib.esnext.d.ts"),
        "/// <reference lib=\"es2025\" />\n",
    );
    write(
        &options.node_types.join("package.json"),
        r#"{"name":"@types/node","version":"24.19.2"}"#,
    );
    write(
        &options.node_types.join("console.d.ts"),
        "declare module 'node:console' { global { interface Console { log(): void; } } }",
    );
    write(
        &options.node_types.join("fs/promises.d.ts"),
        "declare module 'node:fs/promises' { function open(): void; }",
    );
    write(
        &options.node_types.join("ts5.6/console.d.ts"),
        "INVALID TYPESCRIPT",
    );
    (directory, options)
}

#[test]
fn full_generation_is_idempotent_and_check_is_read_only() {
    let (_directory, mut options) = setup();
    let first = generate(&options).unwrap();
    assert_eq!(first.files, 4);
    assert_eq!(first.changed.len(), 6);
    assert!(options.output.join("ts/esnext.test.ts").is_file());
    assert!(options.output.join("node/24/fs/promises.test.ts").is_file());
    assert!(!options.output.join("node/24/ts5.6").exists());
    assert!(generate(&options).unwrap().changed.is_empty());

    let test_path = options.output.join("ts/dom.test.ts");
    let original = fs::read_to_string(&test_path).unwrap();
    let written = original
        .replace("it(\"TODO\"", "it(\"throws TypeError\"")
        .replace(
            "// TODO",
            "expect(() => new TransitionEvent(undefined)).toThrow(TypeError);",
        );
    fs::write(&test_path, &written).unwrap();
    write(
        &options.ts_libs.join("lib.dom.d.ts"),
        "declare var TransitionEvent: { new(type: string): TransitionEvent; extra(): void; };",
    );
    options.check = true;
    assert!(!generate(&options).unwrap().changed.is_empty());
    assert_eq!(fs::read_to_string(&test_path).unwrap(), written);
    options.check = false;
    generate(&options).unwrap();
    let result = fs::read_to_string(test_path).unwrap();
    assert!(result.contains("throws TypeError"));
    assert!(result.contains("describe(\"extra\""));
    assert!(generate(&options).unwrap().changed.is_empty());
}

#[test]
fn library_wrappers_migrate_flat_tests_and_preserve_literal_contents() {
    let (_directory, options) = setup();
    generate(&options).unwrap();
    let path = options.output.join("ts/dom.test.ts");
    let flat = r#"import { describe, it, expect } from "vitest";
export const helper = 123;

describe("TransitionEvent", () => {
  describe("new", () => {
    it("real behavior", () => {
      const text = `first
  exact whitespace
last`;
      expect(() => new TransitionEvent(undefined)).toThrow(TypeError);
    });
  });
});
"#;
    fs::write(&path, flat).unwrap();
    write(
        &options.ts_libs.join("lib.es2020.bigint.d.ts"),
        "interface BigInt { toString(): string; }",
    );
    generate(&options).unwrap();
    let result = fs::read_to_string(&path).unwrap();
    assert!(result.contains("describe(\"DOM\", () => {\n  describe(\"TransitionEvent\""));
    assert!(result.contains("it(\"real behavior\""));
    assert!(result.contains("`first\n  exact whitespace\nlast`"));
    assert!(result.find("export const helper").unwrap() < result.find("describe(\"DOM\"").unwrap());
    let bigint = fs::read_to_string(options.output.join("ts/es2020.bigint.test.ts")).unwrap();
    assert!(bigint.contains("describe(\"ES2020.BigInt\", () => {\n  describe(\"BigInt\""));
    assert!(generate(&options).unwrap().changed.is_empty());
}

#[test]
fn reference_only_library_placeholders_stay_inside_wrapper_and_follow_api_changes() {
    let (_directory, options) = setup();
    generate(&options).unwrap();
    let path = options.output.join("ts/esnext.test.ts");
    let original = fs::read_to_string(&path).unwrap();
    assert!(original.contains("describe(\"ESNext\", () => {\n  // @errs-lib:empty\n  it.todo("));
    write(
        &options.ts_libs.join("lib.esnext.d.ts"),
        "declare function added(): void;",
    );
    generate(&options).unwrap();
    let added = fs::read_to_string(&path).unwrap();
    assert!(!added.contains("@errs-lib:empty"));
    assert!(added.contains("describe(\"added\""));
    write(&options.ts_libs.join("lib.esnext.d.ts"), "");
    generate(&options).unwrap();
    let removed = fs::read_to_string(&path).unwrap();
    assert!(!removed.contains("describe(\"added\""));
    assert!(removed.contains("it.todo("));
    assert!(generate(&options).unwrap().changed.is_empty());
}

#[test]
fn retired_files_remove_managed_apis_but_retain_unrelated_test_source() {
    let (_directory, options) = setup();
    generate(&options).unwrap();
    let test_path = options.output.join("node/24/console.test.ts");
    let source = fs::read_to_string(&test_path).unwrap();
    fs::write(
        &test_path,
        format!("{source}\nconst helper = 123;\nit('custom', () => {{}});\n"),
    )
    .unwrap();
    fs::remove_file(options.node_types.join("console.d.ts")).unwrap();
    generate(&options).unwrap();
    let result = fs::read_to_string(test_path).unwrap();
    assert!(!result.contains("describe(\"node:console\""));
    assert!(result.contains("const helper = 123;\nit('custom', () => {});"));
    assert!(generate(&options).unwrap().changed.is_empty());
}

#[test]
fn invalid_inputs_leave_every_output_untouched_and_versions_are_checked() {
    let (_directory, mut options) = setup();
    generate(&options).unwrap();
    let dom = options.output.join("ts/dom.test.ts");
    let original = fs::read_to_string(&dom).unwrap();
    write(
        &options.ts_libs.join("lib.dom.d.ts"),
        "declare function added(): void;",
    );
    let console = options.output.join("node/24/console.test.ts");
    write(&console, "describe(");
    assert!(generate(&options).is_err());
    assert_eq!(fs::read_to_string(dom).unwrap(), original);
    assert_eq!(fs::read_to_string(console).unwrap(), "describe(");
    options.node_version = "22".into();
    assert!(
        generate(&options)
            .unwrap_err()
            .to_string()
            .contains("Expected @types/node 22.x")
    );
}

#[test]
fn pending_queue_parses_owned_tests_instead_of_comments_strings_or_custom_suites() {
    let (_directory, options) = setup();
    write(
        &options.ts_libs.join("lib.es2015.reflect.d.ts"),
        "declare namespace Reflect { function apply(): void; function construct(): void; function get(): void; }",
    );
    generate(&options).unwrap();
    let file = options.output.join("ts/es2015.reflect.test.ts");
    let source = r#"import { describe as group, test as check } from "vitest";
import * as v from "vitest";
// it("TODO", () => {}); 🦀
group("ES2015.Reflect", { tags: ["browser-only"] }, () => {
  group("Reflect", () => {
    group("apply", () => {
      check.skipIf(false)("TODO", { timeout: 100 }, () => {});
      v.it.todo("TODO");
    });

    group("construct", () => {
      const text = 'it("TODO", () => {})';
      check("throws TypeError", () => {
        const fixture = () => { it("TODO", () => {}); };
      });
      group("custom", () => { check("TODO", () => {}); });
    });

    v.describe.skip("get", () => {
      v.test.each([1, 2])("TODO", () => {});
    });
  });
});
group("unmanaged", () => { check("TODO", () => {}); });
"#
    .replace('\n', "\r\n");
    write(&file, &source);
    let filter = PendingOptions {
        limit: 20,
        kind: Some("ts".into()),
        lib: Some("es2015.reflect".into()),
        ..PendingOptions::default()
    };
    let tasks = next_pending(&options, &filter).unwrap();
    assert_eq!(tasks.len(), 2);
    assert_eq!(
        tasks[0].describe_path,
        ["ES2015.Reflect", "Reflect", "apply"]
    );
    assert_eq!(tasks[0].line, 7);
    assert_eq!(tasks[0].evidence_file, "ts/es2015.reflect/reflect#apply.md");
    assert_eq!(tasks[1].describe_path.last().unwrap(), "get");
    assert_eq!(tasks[0].node_version, None);
    assert_eq!(fs::read_to_string(file).unwrap(), source);
    assert!(
        generate(&Options {
            check: true,
            ..options
        })
        .unwrap()
        .changed
        .is_empty()
    );
}

#[test]
fn pending_cursor_survives_completion_and_partial_repros_stay_pending() {
    let (_directory, options) = setup();
    generate(&options).unwrap();
    let mut filter = PendingOptions {
        limit: 20,
        ..PendingOptions::default()
    };
    let all = next_pending(&options, &filter).unwrap();
    assert_eq!(all.len(), 3); // reference-only esnext is not a callable
    assert!(all.windows(2).all(|pair| pair[0].id < pair[1].id));
    assert_eq!(all[0].node_version.as_deref(), Some("24"));
    assert!(all[1].evidence_file.starts_with("node/24/fs/promises/"));
    assert!(
        all[1]
            .evidence_file
            .contains("node%3Afs%2Fpromises#open.md")
    );
    filter.limit = 1;
    assert_eq!(next_pending(&options, &filter).unwrap()[0].id, all[0].id);

    let first = &all[0].test_file;
    let source = fs::read_to_string(first).unwrap();
    write(
        Path::new(first),
        &source.replace("it(\"TODO\"", "it.skip(\"never throws\""),
    );
    filter.after = Some(all[0].id.clone());
    assert_eq!(next_pending(&options, &filter).unwrap()[0].id, all[1].id);
    filter.after = Some(all[2].id.clone());
    assert!(next_pending(&options, &filter).unwrap().is_empty());

    write(
        Path::new(first),
        &source.replace(
            "// TODO",
            "// partial repro\n      it('throws Error', () => {});",
        ),
    );
    filter.after = None;
    assert_eq!(next_pending(&options, &filter).unwrap()[0].id, all[0].id);
    filter.kind = Some("ts".into());
    assert_eq!(next_pending(&options, &filter).unwrap()[0].id, all[2].id);
    filter.lib = Some("absent".into());
    assert!(next_pending(&options, &filter).unwrap().is_empty());
}

#[test]
fn pending_evidence_names_distinguish_case_collisions_and_unsafe_characters() {
    let (_directory, mut options) = setup();
    write(
        &options.node_types.join("shapes.d.ts"),
        r#"declare module 'node:shapes' {
            interface Console { log(): void; }
            interface console { log(): void; }
            interface Special { "../#%": () => void; [Symbol.iterator](): void; }
        }"#,
    );
    generate(&options).unwrap();
    let filter = PendingOptions {
        limit: 20,
        kind: Some("node".into()),
        lib: Some("shapes".into()),
        ..PendingOptions::default()
    };
    let tasks = next_pending(&options, &filter).unwrap();
    assert_eq!(tasks.len(), 4);
    let mut paths: Vec<_> = tasks
        .iter()
        .map(|task| task.evidence_file.clone())
        .collect();
    paths.sort();
    paths.dedup();
    assert_eq!(paths.len(), 4);
    assert!(paths.iter().any(|path| path.contains("..%2F%23%25")));
    assert!(
        paths
            .iter()
            .any(|path| path.contains("%5Bsymbol.iterator%5D"))
    );
    assert_eq!(
        paths
            .iter()
            .filter(|path| path.contains("console#log~"))
            .count(),
        2
    );

    let completed = fs::read_to_string(&tasks[0].test_file).unwrap().replacen(
        "it(\"TODO\"",
        "it.skip(\"never throws\"",
        1,
    );
    write(Path::new(&tasks[0].test_file), &completed);
    let remaining = next_pending(&options, &filter).unwrap();
    assert_eq!(remaining[0].evidence_file, tasks[1].evidence_file);

    options.node_version = "26".into();
    write(
        &options.node_types.join("package.json"),
        r#"{"name":"@types/node","version":"26.0.0"}"#,
    );
    generate(&options).unwrap();
    let node26 = next_pending(&options, &filter).unwrap();
    assert!(
        node26
            .iter()
            .all(|task| task.evidence_file.starts_with("node/26/")
                && task.node_version.as_deref() == Some("26"))
    );
}

#[test]
fn pending_cli_is_json_only_read_only_and_rejects_invalid_requests() {
    let (_directory, options) = setup();
    generate(&options).unwrap();
    // Listing must be independent of declaration inputs and dependency installation.
    fs::remove_dir_all(&options.ts_libs).unwrap();
    fs::remove_dir_all(&options.node_types).unwrap();
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_errs-lib"))
            .arg("--out")
            .arg(&options.output)
            .args(args)
            .output()
            .unwrap()
    };
    let output = run(&["--next", "2"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);
    assert!(output.stderr.is_empty());
    for args in [
        vec!["--next", "0"],
        vec!["--next", "-1"],
        vec!["--next", "lots"],
        vec!["--next"],
        vec!["--next", "1", "--check"],
        vec!["--next", "1", "--ts-libs", "unused"],
        vec!["--next", "1", "--kind", "invalid"],
        vec!["--next", "1", "--node-version", "../24"],
        vec!["--lib", "dom"],
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "accepted {args:?}");
        assert!(output.stdout.is_empty(), "non-JSON stdout for {args:?}");
    }
    let file = options.output.join("ts/dom.test.ts");
    for source in ["describe(", "describe('DOM', () => {});"] {
        write(&file, source);
        let output = run(&["--next", "20", "--kind", "ts"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(fs::read_to_string(&file).unwrap(), source);
    }
    fs::remove_file(options.output.join("ts/.manifest.json")).unwrap();
    assert!(!run(&["--next", "1", "--kind", "ts"]).status.success());
}
