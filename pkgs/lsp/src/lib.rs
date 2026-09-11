use napi_derive::napi;
use tower::ServiceBuilder;
use tower_lsp::jsonrpc::{Request, Result};
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};

struct Backend {
    client: Client,
}

impl Backend {
    async fn publish(&self, uri: Url, version: i32, text: &str) {
        let diagnostics = text
            .lines()
            .enumerate()
            .filter_map(|(line, text)| {
                let offset = text.find("errorscript-dummy")?;
                let start = text[..offset].encode_utf16().count() as u32;
                Some(Diagnostic {
                    range: Range::new(
                        Position::new(line as u32, start),
                        Position::new(line as u32, start + 17),
                    ),
                    severity: Some(DiagnosticSeverity::INFORMATION),
                    source: Some("ErrorScript".into()),
                    message: "Dummy ErrorScript diagnostic; compiler integration is not enabled."
                        .into(),
                    ..Default::default()
                })
            })
            .collect();
        self.client
            .publish_diagnostics(uri, diagnostics, Some(version))
            .await;
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
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

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let document = params.text_document;
        self.publish(document.uri, document.version, &document.text)
            .await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        if let Some(change) = params.content_changes.last() {
            self.publish(
                params.text_document.uri,
                params.text_document.version,
                &change.text,
            )
            .await;
        }
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        self.client
            .publish_diagnostics(params.text_document.uri, vec![], None)
            .await;
    }

    async fn hover(&self, _: HoverParams) -> Result<Option<Hover>> {
        Ok(Some(Hover {
            contents: HoverContents::Markup(MarkupContent {
                kind: MarkupKind::Markdown,
                value: "**ErrorScript** — dummy language server is running.".into(),
            }),
            range: None,
        }))
    }
}

/// Run the language server in a dedicated Node.js process using LSP-framed stdio.
#[napi]
pub async fn start_server() -> napi::Result<()> {
    let (service, socket) = LspService::new(|client| Backend { client });
    let exit = std::sync::Arc::new(tokio::sync::Notify::new());
    let signal = exit.clone();
    let service = ServiceBuilder::new()
        .map_request(move |request: Request| {
            if request.method() == "exit" && request.id().is_none() {
                signal.notify_one();
            }
            request
        })
        .concurrency_limit(1)
        .service(service);
    // tower-lsp 0.20 otherwise waits for stdin EOF even after an exit notification.
    tokio::select! {
        _ = Server::new(tokio::io::stdin(), tokio::io::stdout(), socket).serve(service) => {},
        _ = exit.notified() => {},
    }
    Ok(())
}
