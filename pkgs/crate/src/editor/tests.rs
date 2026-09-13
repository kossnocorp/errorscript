use super::*;
use tempfile::{TempDir, tempdir};

async fn fixture(files: &[(&str, &str)]) -> (TempDir, EscEditor) {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("errconfig.toml"), "files = ['*.ts']").unwrap();
    for (path, source) in files {
        std::fs::write(dir.path().join(path), source).unwrap();
    }
    let mut editor = EscEditor::default();
    editor
        .update(&dir.path().to_path_buf(), &HashMap::new())
        .await
        .unwrap();
    (dir, editor)
}

fn path(dir: &TempDir, file: &str) -> EscModulePath {
    EscModulePath::try_new(dir.path().join(file)).unwrap()
}

#[tokio::test]
async fn rotate32_edit_keeps_standard_error_identity_with_test_runner_globals() {
    let source = r#"
        import './runner';
        export function hash(value: number) { return rotate32(value, 13); }
        function rotate32(value: number, shift: number): number {
            return ((value << (shift % 32)) | (value >>> (32 - (shift % 32)))) >>> 0;
        }
    "#;
    let (dir, mut editor) = fixture(&[
        ("entry.ts", source),
        (
            "runner.ts",
            r#"
            Error.stackTraceLimit = Infinity;
            const SAFE_TIMERS_SYMBOL = Symbol("vitest:SAFE_TIMERS");
            function setSafeTimers() { globalThis[SAFE_TIMERS_SYMBOL] = {}; }
        "#,
        ),
    ])
    .await;
    let entry = path(&dir, "entry.ts");
    assert!(
        calls(&editor, &entry, "rotate32(value, 13)")[0]
            .errors
            .is_empty()
    );
    let edited = source.replace(
        "return ((value",
        "if (value === 3) throw new Error(\"Wut\"); return ((value",
    );
    editor
        .update(
            &dir.path().to_path_buf(),
            &HashMap::from([(entry.clone(), edited)]),
        )
        .await
        .unwrap();
    assert_eq!(
        calls(&editor, &entry, "rotate32(value, 13)")[0].errors,
        ["Error"]
    );
    editor
        .update(&dir.path().to_path_buf(), &HashMap::new())
        .await
        .unwrap();
    assert!(
        calls(&editor, &entry, "rotate32(value, 13)")[0]
            .errors
            .is_empty()
    );
}

#[tokio::test]
async fn actual_global_replacements_still_invalidate_models() {
    for mutation in [
        "globalThis.Error = external;",
        "const host = globalThis; host.Error = external;",
        "globalThis[key] = external;",
        "const Symbol = () => 'Error'; const key = Symbol(); globalThis[key] = external;",
        "let key = Symbol(); key = external; globalThis[key] = external;",
    ] {
        let (dir, editor) = fixture(&[(
            "entry.ts",
            &format!("{mutation} export function fail() {{ throw new Error('Wut'); }} fail();"),
        )])
        .await;
        assert!(
            calls(&editor, &path(&dir, "entry.ts"), "fail()")[0]
                .errors
                .contains(&"unknown".into()),
            "{mutation}"
        );
    }
    let (dir, editor) = fixture(&[(
        "entry.ts",
        "JSON.parse = external; JSON.stringify(1); JSON.parse('{}');",
    )])
    .await;
    let entry = path(&dir, "entry.ts");
    assert!(
        calls(&editor, &entry, "JSON.parse('{}')")[0]
            .errors
            .contains(&"unknown".into())
    );
    assert!(
        !calls(&editor, &entry, "JSON.stringify(1)")[0]
            .errors
            .contains(&"unknown".into())
    );
}

fn calls<'a>(editor: &'a EscEditor, path: &EscModulePath, source: &str) -> Vec<&'a EscCallReport> {
    let text = editor.source(path).unwrap();
    editor.reports[path]
        .iter()
        .filter(|report| &text[report.start as usize..report.end as usize] == source)
        .collect()
}

#[tokio::test]
async fn hover_and_sync_async_handlers() {
    let (dir, editor) = fixture(&[(
        "entry.ts",
        r#"
        export function sync() { throw new TypeError(); }
        async function task() { throw new RangeError(); }
        sync();
        try { sync(); } catch (error) {}
        try {} catch (error) { sync(); }
        try {} finally { sync(); }
        function nested() { sync(); }
        task();
        try { task(); } catch (error) {}
        try { await task(); } catch (error) {}
        await task();
        task().catch(() => {});
        task().then(undefined, () => {});
        task().finally(() => {});
        task().catch(() => { throw new SyntaxError(); });
    "#,
    )])
    .await;
    let path = path(&dir, "entry.ts");
    let sync = calls(&editor, &path, "sync()");
    assert_eq!(sync.len(), 5);
    assert!(sync.iter().all(|call| call.errors == ["TypeError"]));
    assert_eq!(
        sync.iter()
            .map(|call| !call.uncaught.is_empty())
            .collect::<Vec<_>>(),
        [true, false, true, true, false]
    );
    let task = calls(&editor, &path, "task()");
    assert!(task.iter().all(|call| call.errors == ["RangeError"]));
    assert_eq!(
        task.iter()
            .map(|call| !call.uncaught.is_empty())
            .collect::<Vec<_>>(),
        [true, true, false, true, false, false, true, false]
    );
    assert!(
        calls(&editor, &path, "task().catch(() => {})")[0]
            .uncaught
            .is_empty()
    );
    assert!(
        calls(&editor, &path, "task().then(undefined, () => {})")[0]
            .uncaught
            .is_empty()
    );
    assert_eq!(
        calls(
            &editor,
            &path,
            "task().catch(() => { throw new SyntaxError(); })"
        )[0]
        .uncaught,
        ["SyntaxError"]
    );
}

#[tokio::test]
async fn edits_invalidate_dependencies_and_keep_unrelated_arenas_and_reports() {
    let (dir, mut editor) = fixture(&[
        ("entry.ts", "import { fail } from './bridge'; fail();"),
        ("bridge.ts", "export { fail } from './leaf';"),
        (
            "leaf.ts",
            "export function fail() { throw new TypeError(); }",
        ),
        (
            "other.ts",
            "export function other() { throw new RangeError(); } other();",
        ),
    ])
    .await;
    let entry = path(&dir, "entry.ts");
    let leaf = path(&dir, "leaf.ts");
    let other = path(&dir, "other.ts");
    let arena = editor.source(&other).unwrap().as_ptr();
    let entry_arena = editor.source(&entry).unwrap().as_ptr();
    let report = editor.reports[&other].as_ptr();
    let overlays = HashMap::from([(
        leaf.clone(),
        "export function fail() { throw new SyntaxError(); }".into(),
    )]);
    let changed = editor
        .update(&dir.path().to_path_buf(), &overlays)
        .await
        .unwrap();
    assert!(changed.contains(&entry));
    assert!(!changed.contains(&other));
    assert_eq!(editor.source(&other).unwrap().as_ptr(), arena);
    assert_eq!(editor.source(&entry).unwrap().as_ptr(), entry_arena);
    assert_eq!(editor.reports[&other].as_ptr(), report);
    assert_eq!(calls(&editor, &entry, "fail()")[0].errors, ["SyntaxError"]);
    assert!(
        editor
            .update(&dir.path().to_path_buf(), &overlays)
            .await
            .unwrap()
            .is_empty()
    );
    editor
        .update(&dir.path().to_path_buf(), &HashMap::new())
        .await
        .unwrap();
    assert_eq!(calls(&editor, &entry, "fail()")[0].errors, ["TypeError"]);
    std::fs::write(leaf.as_path(), "export function fail() {}").unwrap();
    editor
        .update(&dir.path().to_path_buf(), &HashMap::new())
        .await
        .unwrap();
    assert!(calls(&editor, &entry, "fail()")[0].uncaught.is_empty());
}

#[tokio::test]
async fn resolves_created_dependencies_and_drops_deleted_modules() {
    let (dir, mut editor) =
        fixture(&[("entry.ts", "import { fail } from './leaf'; fail();")]).await;
    let entry = path(&dir, "entry.ts");
    assert!(
        calls(&editor, &entry, "fail()")[0]
            .errors
            .contains(&"unknown".into())
    );
    std::fs::write(
        dir.path().join("leaf.ts"),
        "export function fail() { throw new RangeError(); }",
    )
    .unwrap();
    let leaf = path(&dir, "leaf.ts");
    editor
        .update(&dir.path().to_path_buf(), &HashMap::new())
        .await
        .unwrap();
    assert_eq!(calls(&editor, &entry, "fail()")[0].errors, ["RangeError"]);
    std::fs::remove_file(leaf.as_path()).unwrap();
    editor
        .update(&dir.path().to_path_buf(), &HashMap::new())
        .await
        .unwrap();
    assert!(editor.source(&leaf).is_none());
    assert!(!editor.reports.contains_key(&leaf));
    assert!(
        calls(&editor, &entry, "fail()")[0]
            .errors
            .contains(&"unknown".into())
    );
}

#[tokio::test]
async fn handles_stored_promises_and_retains_escapes() {
    let (dir, editor) = fixture(&[(
        "entry.ts",
        r#"
        export async function task() { throw new RangeError(); }
        const handled = task();
        handled.catch(() => {});
        const awaited = task();
        try { await awaited; } catch {}
        const escaped = task();
        function later() { escaped.catch(() => {}); }
        const conditional = task();
        if (flag) conditional.catch(() => {});
        try { await consume(task()); } catch {}
    "#,
    )])
    .await;
    let path = path(&dir, "entry.ts");
    assert_eq!(
        calls(&editor, &path, "task()")
            .iter()
            .map(|call| !call.uncaught.is_empty())
            .collect::<Vec<_>>(),
        [false, false, true, true, true]
    );
}

#[tokio::test]
async fn unsaved_files_and_configuration_recovery() {
    let (dir, mut editor) = fixture(&[]).await;
    let file = EscModulePath::for_document(dir.path().join("unsaved.ts")).unwrap();
    let overlays = HashMap::from([(
        file.clone(),
        "function fail() { throw new Error(); } fail();".into(),
    )]);
    editor
        .update(&dir.path().to_path_buf(), &overlays)
        .await
        .unwrap();
    assert_eq!(calls(&editor, &file, "fail()")[0].uncaught, ["Error"]);
    std::fs::write(dir.path().join("errconfig.toml"), "invalid [").unwrap();
    assert!(
        editor
            .update(&dir.path().to_path_buf(), &overlays)
            .await
            .is_err()
    );
    std::fs::write(dir.path().join("errconfig.toml"), "files = []").unwrap();
    editor.reports.clear();
    editor
        .update(&dir.path().to_path_buf(), &overlays)
        .await
        .unwrap();
    assert_eq!(calls(&editor, &file, "fail()")[0].uncaught, ["Error"]);
}

#[tokio::test]
async fn imported_promise_handlers_update_after_edits() {
    let (dir, mut editor) = fixture(&[
        ("entry.ts", "import { handle } from './handler'; async function task() { throw new TypeError(); } task().catch(handle);"),
        ("handler.ts", "export function handle() {}"),
    ]).await;
    let entry = path(&dir, "entry.ts");
    assert!(
        editor.reports[&entry]
            .iter()
            .all(|call| call.uncaught.is_empty())
    );
    let overlays = HashMap::from([(
        path(&dir, "handler.ts"),
        "export function handle() { throw new SyntaxError(); }".into(),
    )]);
    editor
        .update(&dir.path().to_path_buf(), &overlays)
        .await
        .unwrap();
    assert_eq!(
        calls(&editor, &entry, "task().catch(handle)")[0].uncaught,
        ["SyntaxError"]
    );
}

#[tokio::test]
async fn global_mutations_invalidate_disconnected_modules() {
    let (dir, mut editor) = fixture(&[
        ("entry.ts", "export {}; JSON.parse('{}');"),
        ("globals.ts", "export {};"),
    ])
    .await;
    let entry = path(&dir, "entry.ts");
    let original = calls(&editor, &entry, "JSON.parse('{}')")[0].errors.clone();
    let overlays = HashMap::from([(
        path(&dir, "globals.ts"),
        "export {}; JSON.parse = external;".into(),
    )]);
    let changed = editor
        .update(&dir.path().to_path_buf(), &overlays)
        .await
        .unwrap();
    assert!(changed.contains(&entry));
    assert!(
        calls(&editor, &entry, "JSON.parse('{}')")[0]
            .errors
            .contains(&"unknown".into())
    );
    editor
        .update(&dir.path().to_path_buf(), &HashMap::new())
        .await
        .unwrap();
    assert_eq!(
        calls(&editor, &entry, "JSON.parse('{}')")[0].errors,
        original
    );
}

#[tokio::test]
async fn call_reports_use_typed_environments_and_include_constructors() {
    let (dir, editor) = fixture(&[(
        "entry.ts",
        r#"
        export function parse(value: string) { JSON.parse(value); }
        class Boom { constructor() { throw new RangeError(); } }
        new Boom();
        try { new Boom(); } catch {}
    "#,
    )])
    .await;
    let path = path(&dir, "entry.ts");
    assert_eq!(
        calls(&editor, &path, "JSON.parse(value)")[0].errors,
        ["SyntaxError"]
    );
    assert_eq!(
        calls(&editor, &path, "new Boom()")
            .iter()
            .map(|call| !call.uncaught.is_empty())
            .collect::<Vec<_>>(),
        [true, false]
    );
}
