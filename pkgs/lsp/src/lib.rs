use errorscript::{EscCallReport, EscCheckDiagnostic, EscEditor, EscModulePath};
use napi_derive::napi;
use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};
use tokio::sync::{Mutex, Notify};
use tower::ServiceBuilder;
use tower_lsp::jsonrpc::{Request, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
    workspace: Arc<Mutex<Workspace>>,
    ready: Arc<Notify>,
}

#[derive(Default)]
struct Workspace {
    root: Option<PathBuf>,
    documents: HashMap<Url, (i32, String)>,
    editor: EscEditor,
    snapshots: HashMap<EscModulePath, Snapshot>,
    revision: u64,
    completed: u64,
    building: bool,
}

struct Snapshot {
    diagnostics: Vec<EscCheckDiagnostic>,
    text: String,
    reports: Vec<EscCallReport>,
}

enum ThrowingCalls {}

impl notification::Notification for ThrowingCalls {
    type Params = ThrowingCallsParams;
    const METHOD: &'static str = "errorscript/throwingCalls";
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ThrowingCallsParams {
    uri: Url,
    version: i32,
    ranges: Vec<Range>,
}

impl Backend {
    /// Notifications only update inputs. A single worker coalesces edits and
    /// publishes the newest completed revision without occupying LSP requests.
    fn rebuild(&self, workspace: &mut Workspace) {
        workspace.revision += 1;
        if workspace.building {
            return;
        }
        workspace.building = true;
        let mut editor = std::mem::take(&mut workspace.editor);
        let workspace = self.workspace.clone();
        let client = self.client.clone();
        let ready = self.ready.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                let (revision, root, overlays) = {
                    let workspace = workspace.lock().await;
                    (
                        workspace.revision,
                        workspace.root.clone(),
                        workspace
                            .documents
                            .iter()
                            .filter_map(|(uri, (_, text))| Some((module_path(uri)?, text.clone())))
                            .collect::<HashMap<_, _>>(),
                    )
                };
                let runtime = tokio::runtime::Handle::current();
                let result = tokio::task::spawn_blocking(move || {
                    let result = root
                        .map(|root| runtime.block_on(editor.update(&root, &overlays)))
                        .transpose();
                    let error = result.err().map(|error| format!("ErrorScript: {error:#}"));
                    if error.is_some() {
                        editor.reports.clear();
                        editor.diagnostics.clear();
                    }
                    let snapshots = overlays
                        .keys()
                        .filter_map(|path| {
                            Some((
                                path.clone(),
                                Snapshot {
                                    diagnostics: editor
                                        .diagnostics
                                        .get(path)
                                        .cloned()
                                        .unwrap_or_default(),
                                    text: editor.source(path)?.to_owned(),
                                    reports: editor.reports.get(path).cloned().unwrap_or_default(),
                                },
                            ))
                        })
                        .collect();
                    (editor, snapshots, error)
                })
                .await;
                let (next_editor, snapshots, error) = match result {
                    Ok(result) => result,
                    Err(error) => (
                        EscEditor::default(),
                        HashMap::new(),
                        Some(error.to_string()),
                    ),
                };
                editor = next_editor;
                let mut state = workspace.lock().await;
                if state.revision != revision {
                    continue;
                }
                state.snapshots = snapshots;
                let diagnostics = publications(&state);
                // Serialize publication with document notifications, including
                // close, so old diagnostics cannot reappear after being cleared.
                if let Some(error) = error {
                    client.log_message(MessageType::ERROR, error).await;
                }
                for (uri, version, diagnostics) in diagnostics {
                    let ranges = module_path(&uri)
                        .and_then(|path| state.snapshots.get(&path))
                        .map(|snapshot| {
                            let lines = LineIndex::new(&snapshot.text);
                            snapshot
                                .reports
                                .iter()
                                .filter(|report| !report.errors.is_empty())
                                .map(|report| {
                                    lines.range(
                                        &snapshot.text,
                                        report.highlight_start,
                                        report.highlight_end,
                                    )
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    client
                        .send_notification::<ThrowingCalls>(ThrowingCallsParams {
                            uri: uri.clone(),
                            version,
                            ranges,
                        })
                        .await;
                    client
                        .publish_diagnostics(uri, diagnostics, Some(version))
                        .await;
                }
                state.completed = revision;
                state.editor = editor;
                state.building = false;
                drop(state);
                ready.notify_waiters();
                break;
            }
        });
    }
}

fn publications(workspace: &Workspace) -> Vec<(Url, i32, Vec<Diagnostic>)> {
    workspace
        .documents
        .iter()
        .map(|(uri, (version, text))| {
            let lines = LineIndex::new(text);
            let mut diagnostics: Vec<Diagnostic> = module_path(uri)
                .and_then(|path| workspace.snapshots.get(&path))
                .into_iter()
                .flat_map(|snapshot| &snapshot.reports)
                .filter(|report| !report.uncaught.is_empty())
                .map(|report| Diagnostic {
                    range: lines.range(text, report.start, report.end),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("uncaught-call".into())),
                    source: Some("ErrorScript".into()),
                    message: format!(
                        "Unhandled top-level call errors: {}. Add a catch handler.",
                        report.uncaught.join(", ")
                    ),
                    ..Default::default()
                })
                .collect();
            if let Some(snapshot) = module_path(uri).and_then(|path| workspace.snapshots.get(&path))
            {
                diagnostics.extend(snapshot.diagnostics.iter().map(|report| Diagnostic {
                    range: lines.range(text, report.start, report.end),
                    severity: Some(DiagnosticSeverity::ERROR),
                    code: Some(NumberOrString::String("cast-catch".into())),
                    source: Some("ErrorScript".into()),
                    message: report.message.clone(),
                    ..Default::default()
                }));
            }
            (uri.clone(), *version, diagnostics)
        })
        .collect()
}

fn module_path(uri: &Url) -> Option<EscModulePath> {
    EscModulePath::for_document(uri.to_file_path().ok()?).ok()
}

struct LineIndex(Vec<usize>);

impl LineIndex {
    fn new(text: &str) -> Self {
        Self(
            std::iter::once(0)
                .chain(
                    text.bytes()
                        .enumerate()
                        .filter_map(|(index, byte)| (byte == b'\n').then_some(index + 1)),
                )
                .collect(),
        )
    }

    fn position(&self, text: &str, offset: u32) -> Position {
        let mut end = (offset as usize).min(text.len());
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        let line = self.0.partition_point(|start| *start <= end) - 1;
        Position::new(
            line as u32,
            text[self.0[line]..end].encode_utf16().count() as u32,
        )
    }

    fn offset(&self, text: &str, position: Position) -> Option<u32> {
        let start = *self.0.get(position.line as usize)?;
        let mut units = 0;
        for (offset, character) in text[start..].char_indices() {
            if units == position.character {
                return Some((start + offset) as u32);
            }
            if character == '\n' {
                return None;
            }
            units += character.len_utf16() as u32;
            if units > position.character {
                return None;
            }
        }
        (units == position.character).then_some(text.len() as u32)
    }

    fn range(&self, text: &str, start: u32, end: u32) -> Range {
        Range::new(self.position(text, start), self.position(text, end))
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        self.workspace.lock().await.root = params
            .workspace_folders
            .and_then(|folders| folders.into_iter().next())
            .map(|folder| folder.uri)
            .or(params.root_uri)
            .and_then(|uri| uri.to_file_path().ok());
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                hover_provider: Some(HoverProviderCapability::Simple(true)),
                ..Default::default()
            },
            server_info: Some(ServerInfo {
                name: "ErrorScript".into(),
                version: Some(env!("CARGO_PKG_VERSION").into()),
            }),
        })
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn initialized(&self, _: InitializedParams) {
        let mut workspace = self.workspace.lock().await;
        self.rebuild(&mut workspace);
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        let mut workspace = self.workspace.lock().await;
        if workspace.root.is_none() {
            workspace.root = document
                .uri
                .to_file_path()
                .ok()
                .and_then(|path| path.parent().map(PathBuf::from));
        }
        workspace
            .documents
            .insert(document.uri, (document.version, document.text));
        self.rebuild(&mut workspace);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.last() {
            let mut workspace = self.workspace.lock().await;
            if let Some(document) = workspace.documents.get_mut(&params.text_document.uri) {
                if params.text_document.version <= document.0 {
                    return;
                }
                *document = (params.text_document.version, change.text.clone());
                self.rebuild(&mut workspace);
            }
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let mut workspace = self.workspace.lock().await;
        workspace.documents.remove(&params.text_document.uri);
        self.rebuild(&mut workspace);
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }

    async fn did_change_watched_files(&self, params: DidChangeWatchedFilesParams) {
        let mut workspace = self.workspace.lock().await;
        // Saving an open document does not change the authoritative buffer.
        // Creation/deletion can still change import resolution.
        if params.changes.iter().all(|change| {
            change.typ == FileChangeType::CHANGED && workspace.documents.contains_key(&change.uri)
        }) {
            return;
        }
        self.rebuild(&mut workspace);
    }

    async fn hover(&self, params: HoverParams) -> Result<Option<Hover>> {
        let workspace = loop {
            let notified = self.ready.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            let workspace = self.workspace.lock().await;
            if workspace.completed == workspace.revision {
                break workspace;
            }
            drop(workspace);
            notified.await;
        };
        let document = params.text_document_position_params;
        let Some(path) = module_path(&document.text_document.uri) else {
            return Ok(None);
        };
        let Some(snapshot) = workspace.snapshots.get(&path) else {
            return Ok(None);
        };
        let text = &snapshot.text;
        let lines = LineIndex::new(text);
        let Some(offset) = lines.offset(text, document.position) else {
            return Ok(None);
        };
        let Some(report) = snapshot
            .reports
            .iter()
            .filter(|report| report.start <= offset && offset < report.end)
            .min_by_key(|report| report.end - report.start)
        else {
            return Ok(None);
        };
        let value = if report.errors.is_empty() {
            "**ErrorScript**\n\nNo known thrown errors.".into()
        } else {
            format!(
                "**ErrorScript — may throw / reject**\n\n{}",
                report
                    .errors
                    .iter()
                    .map(|error| format!("- `{error}`"))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        };
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value,
            }),
            range: Some(lines.range(text, report.start, report.end)),
        }))
    }
}

/// Run the language server in a dedicated Node.js process using LSP-framed stdio.
#[napi]
pub async fn start_server() -> napi::Result<()> {
    let (service, socket) = LspService::new(|client| Backend {
        client,
        workspace: Arc::new(Mutex::new(Workspace::default())),
        ready: Arc::new(Notify::new()),
    });
    let exit = std::sync::Arc::new(tokio::sync::Notify::new());
    let signal = exit.clone();
    let service = ServiceBuilder::new()
        .map_request(move |request: Request| {
            if request.method() == "exit" && request.id().is_none() {
                signal.notify_one();
            }
            request
        })
        .service(service);
    // tower-lsp 0.20 otherwise waits for stdin EOF even after an exit notification.
    tokio::select! {
        _ = Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service) => {},
        _ = exit.notified() => {},
    }
    Ok(())
}
