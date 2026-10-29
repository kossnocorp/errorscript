use errorscript_lib::native::{Finding, analyze, clang_ast};
use std::path::Path;

/// Replay genuine, bounded Clang dumps without changing production inference.
/// The namespace filter omits dependencies: this is deliberately partial input.
#[test]
#[ignore = "requires externally captured real Node Clang AST and explicit entry"]
fn replay_real_node_ast_for_gap_analysis() -> anyhow::Result<()> {
    let input = std::env::var("ERRS_NATIVE_GAP_AST")?;
    let entry = std::env::var("ERRS_NATIVE_GAP_ENTRY")?;
    let output = std::env::var("ERRS_NATIVE_GAP_REPORT")?;
    // Clang filtering emits consecutive JSON roots, not a single JSON array.
    // Preserve the real compiler nodes; only add a container for indexing.
    let bytes = std::fs::read(input)?;
    let nodes = serde_json::Deserializer::from_slice(&bytes)
        .into_iter::<serde_json::Value>()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let ast = serde_json::json!({"kind": "TranslationUnitDecl", "inner": nodes});
    let report = analyze(
        &ast,
        &entry,
        None,
        "clang 22.1.8; externally filtered AST".into(),
        "Partial namespace-filtered input; outer namespace context and dependencies may be absent"
            .into(),
    )?;
    assert!(!report.complete && !report.sound);
    assert!(!report.functions.is_empty());
    std::fs::write(output, serde_json::to_string_pretty(&report)?)?;
    Ok(())
}

#[test]
fn cli_emits_json_with_compiler_flags_without_separator() -> anyhow::Result<()> {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tools/native-effects/node-style.cc");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_errs-native"))
        .args([
            "--source",
            fixture.to_str().unwrap(),
            "--entry",
            "node::Parse",
            "-std=c++20",
        ])
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(json["entry"], "node::Parse");
    assert_eq!(json["complete"], false);
    assert_eq!(json["functions"].as_array().unwrap().len(), 2);
    Ok(())
}

fn report(
    source: &str,
    entry: &str,
    signature: Option<&str>,
) -> anyhow::Result<errorscript_lib::native::Report> {
    let dir = tempfile::tempdir()?;
    let file = dir.path().join("fixture.cc");
    std::fs::write(&file, source)?;
    let (ast, version, diagnostics) =
        clang_ast(Path::new("clang++"), &file, &["-std=c++20".into()])?;
    analyze(&ast, entry, signature, version, diagnostics)
}

#[test]
fn resolves_overloads_definitions_and_recursive_edges() -> anyhow::Result<()> {
    let r = report(
        "namespace node { void f(int); void f(double) { throw 2; } void f(int x) { if(x) f(x-1); } void entry() { f(1); } }",
        "node::entry",
        None,
    )?;
    assert_eq!(r.functions.len(), 2);
    assert!(r.findings.is_empty()); // The uncalled overload must not contribute a throw.
    assert!(r.edges.iter().all(|e| e.signature == "void (int)"));
    assert!(!r.complete && !r.sound);
    Ok(())
}

#[test]
fn detects_node_style_exception_and_unresolved_v8_boundary() -> anyhow::Result<()> {
    let r = report(
        "struct Isolate { void ThrowException(int); }; int ERR_INVALID_URL(); void fail(Isolate* i) { i->ThrowException(ERR_INVALID_URL()); } void entry(Isolate* i) { fail(i); }",
        "entry",
        None,
    )?;
    assert!(r.findings.iter().any(|f| matches!(f,Finding::JsExceptionCandidate{payload_symbols,..} if payload_symbols.contains(&"ERR_INVALID_URL".into()))));
    assert!(
        r.unresolved
            .iter()
            .any(|b| b.operation == "Isolate::ThrowException")
    );
    Ok(())
}

#[test]
fn creation_is_not_throw_and_lambdas_are_not_eagerly_walked() -> anyhow::Result<()> {
    let r = report(
        "int ERR_BAD(); void entry() { auto error = ERR_BAD(); auto fn = [] { throw 1; }; }",
        "entry",
        None,
    )?;
    assert!(r.findings.is_empty());
    assert!(r.unresolved.iter().any(|b| b.operation == "lambda"));
    Ok(())
}

#[test]
fn reports_indirect_virtual_and_constructor_boundaries() -> anyhow::Result<()> {
    let r = report(
        "struct B { B(); virtual void run() { throw 1; } }; void entry(B* b, void (*cb)()) { B local; b->run(); cb(); }",
        "entry",
        None,
    )?;
    assert!(r.unresolved.iter().any(|b| b.reason.contains("Virtual")));
    assert!(r.unresolved.iter().any(|b| b.operation == "indirect-call"));
    assert!(
        r.unresolved
            .iter()
            .any(|b| b.reason.contains("Constructor invocation"))
    );
    Ok(())
}

#[test]
fn ambiguous_entries_require_signature_and_invalid_cpp_fails() -> anyhow::Result<()> {
    let cpp = "void entry(int) {} void entry(double) {}";
    assert!(report(cpp, "entry", None).is_err());
    assert_eq!(report(cpp, "entry", Some("void (int)"))?.functions.len(), 1);
    assert!(report("void entry() { invalid syntax }", "entry", None).is_err());
    Ok(())
}

#[test]
fn caught_throw_is_still_only_a_candidate() -> anyhow::Result<()> {
    let r = report(
        "void entry() { try { throw 1; } catch (...) {} }",
        "entry",
        None,
    )?;
    assert!(
        r.findings
            .iter()
            .any(|f| matches!(f, Finding::CppThrow { .. }))
    );
    assert!(!r.complete);
    Ok(())
}

#[test]
fn gap_local_error_payload_is_not_resolved_to_its_factory() -> anyhow::Result<()> {
    let r = report(
        "struct Isolate { void ThrowException(int); }; int ERR_INVALID_URL(); void entry(Isolate* i) { int err = ERR_INVALID_URL(); i->ThrowException(err); }",
        "entry",
        None,
    )?;
    assert!(r.findings.iter().any(|f| matches!(f,
        Finding::JsExceptionCandidate { payload_symbols, .. }
        if payload_symbols == &vec![String::from("err")])));
    // This records the limitation; it does not certify exception identity.
    Ok(())
}

#[test]
fn gap_method_name_heuristic_can_misclassify_unrelated_methods() -> anyhow::Result<()> {
    let r = report(
        "struct NotV8 { void ThrowException(int) {} }; void entry(NotV8* x) { x->ThrowException(1); }",
        "entry",
        None,
    )?;
    assert!(
        r.findings
            .iter()
            .any(|f| matches!(f, Finding::JsExceptionCandidate { .. }))
    );
    // No exception is thrown. The candidate is a known heuristic false positive.
    assert!(!r.sound && !r.complete);
    Ok(())
}
