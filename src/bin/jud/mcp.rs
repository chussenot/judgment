//! `jud mcp [DIR]`: a Model Context Protocol server on stdio. Every `.jud`
//! Rubric under DIR is one tool, named by the rubric (`metadata.name`);
//! calling it with `{state, options?}` runs what `jud RUBRIC` runs
//! ([`run::decide`]) and returns the same verdict text as the tool's one
//! content block, so an agent and a pipeline read the same bytes. A failed
//! call is a tool error (`isError`), never a protocol error: the agent can
//! read it and retry.
//!
//! stdout is the protocol channel. Nothing in this path prints to it; the
//! diagnostics (a file skipped, a variable ignored) go to stderr, as they do
//! everywhere in the binary.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use clap::{Args, ValueHint};
use judgment::jud::{Document, Rubric, Supplied};
use judgment::{Assets, Question};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool,
};
use rmcp::schemars::{self, JsonSchema};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler, ServiceExt};
use serde_json::Value;

use crate::backend::{Backend, Failure};
use crate::batch::base_of;
use crate::run::{self, AssetsArg};

/// Serve every Rubric under a directory as an MCP tool, over stdio.
///
/// Each rubric becomes a tool named by its `metadata.name`; its input is
/// `{state, options?}`, and its result is the verdict map `jud RUBRIC`
/// prints, as text. The backend is chosen as for `jud RUBRIC`: TypeSafe by
/// default (a missing key refuses to start, as there), or the recordings
/// under --replay with no key and no network. stdout carries the protocol
/// and nothing else.
#[derive(Args)]
pub(crate) struct Mcp {
    /// The directory searched, recursively, for Rubric documents.
    #[arg(value_name = "DIR", default_value = ".", value_hint = ValueHint::DirPath)]
    dir: PathBuf,
    /// Answer from the recordings in this directory instead of a server.
    #[arg(long, env = "JUD_REPLAY", value_name = "DIR", value_hint = ValueHint::DirPath)]
    replay: Option<PathBuf>,
    /// Ask for long context, as `jud --long-context` does.
    #[arg(long)]
    long_context: bool,
}

/// What a tool takes: the JSON state the questions are asked about, and the
/// options of the Choices the rubric marks `options_from: request`, by
/// question id then option key, asked in the order given.
#[derive(JsonSchema)]
#[allow(
    dead_code,
    reason = "read through the schema; the arguments are parsed by hand"
)]
struct Input {
    /// The JSON state to evaluate: any JSON value.
    state: Value,
    /// Options per question id, for Choices that take them per request.
    options: Option<std::collections::BTreeMap<String, std::collections::BTreeMap<String, Value>>>,
}

/// One rubric, read once at start, with the file it came from (its images
/// are read from that file's directory on each call).
struct Entry {
    path: String,
    rubric: Rubric,
    tool: Tool,
}

struct Server {
    entries: Vec<Arc<Entry>>,
    backend: Arc<Backend>,
}

pub(crate) fn run(args: &Mcp) -> ExitCode {
    match serve(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(failure) => failure.report(),
    }
}

fn serve(args: &Mcp) -> Result<(), Failure> {
    let entries = scan(&args.dir)?;
    let backend = Arc::new(Backend::open(
        args.replay.as_deref(),
        args.long_context.then_some(true),
    )?);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| Failure::Backend(format!("cannot start the runtime: {e}")))?;
    let server = Server {
        entries,
        // The backend owns a runtime, and a runtime cannot be dropped from
        // inside another; this clone is the one that outlives `block_on`.
        backend: Arc::clone(&backend),
    };
    let result = runtime.block_on(async move {
        let running = server
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| Failure::Backend(format!("cannot start the MCP server: {e}")))?;
        running
            .waiting()
            .await
            .map(|_| ())
            .map_err(|e| Failure::Backend(format!("the MCP server stopped: {e}")))
    });
    drop(runtime);
    drop(backend);
    result
}

/// Every Rubric under `dir`, in path order. Another kind of document is not
/// a tool and is passed over; a file that is not a document at all is said
/// on stderr and passed over too, one bad file never hides the rest.
fn scan(dir: &Path) -> Result<Vec<Arc<Entry>>, Failure> {
    let mut paths = Vec::new();
    collect(dir, &mut paths)
        .map_err(|e| Failure::Usage(format!("cannot read {}: {e}", dir.display())))?;
    paths.sort();
    let mut entries: Vec<Arc<Entry>> = Vec::new();
    for path in paths {
        let shown = path.display().to_string();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) => {
                crate::out::note!("jud: skipping {shown}: {e}");
                continue;
            }
        };
        match judgment::jud::parse(&text) {
            Ok(Document::Rubric(rubric)) => {
                if let Some(first) = entries.iter().find(|e| e.rubric.name == rubric.name) {
                    return Err(Failure::Usage(format!(
                        "two rubrics are named `{}`: {} and {shown}; a tool is named by its rubric, so rename one",
                        rubric.name, first.path
                    )));
                }
                entries.push(Arc::new(Entry {
                    tool: tool_of(&rubric),
                    path: shown,
                    rubric,
                }));
            }
            Ok(_) => {}
            Err(e) => crate::out::note!("jud: skipping {shown}: {e}"),
        }
    }
    if entries.is_empty() {
        return Err(Failure::Usage(format!(
            "no Rubric document under {}: `jud mcp` serves one tool per rubric",
            dir.display()
        )));
    }
    Ok(entries)
}

/// The `.jud` regular files under `dir`; links are not followed.
fn collect(dir: &Path, into: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        let kind = path.symlink_metadata()?.file_type();
        if kind.is_dir() {
            collect(&path, into)?;
        } else if kind.is_file()
            && path.extension().and_then(|e| e.to_str()) == Some(judgment::jud::EXTENSION)
        {
            into.push(path);
        }
    }
    Ok(())
}

fn tool_of(rubric: &Rubric) -> Tool {
    let questions: Vec<String> = rubric
        .questions
        .iter()
        .map(|(id, q)| {
            let kind = match q.question {
                Question::Noul { .. } => "noul",
                Question::Choice { .. } => "choice",
                Question::Score { .. } => "score",
                Question::Box { .. } => "box",
            };
            format!("{id} ({kind})")
        })
        .collect();
    let what = rubric.description.as_deref().map_or_else(
        || format!("Evaluate a state with the rubric `{}`.", rubric.name),
        str::to_owned,
    );
    Tool::new(
        rubric.name.clone(),
        format!(
            "{what} Returns one verdict per question as JSON: {}.",
            questions.join(", ")
        ),
        rmcp::handler::server::common::schema_for_type::<Input>(),
    )
}

/// One call, on a blocking thread: the backend runs its requests on a
/// runtime of its own, which cannot be entered from an async task.
fn evaluate(
    entry: &Entry,
    state: &Value,
    supplied: &Supplied,
    backend: &Arc<Backend>,
) -> Result<String, Failure> {
    let mut assets = Assets::new();
    entry
        .rubric
        .load_assets(&base_of(&entry.path), &mut assets)
        .map_err(|e| {
            Failure::Usage(format!(
                "{}: the rubric's images cannot be loaded: {e}",
                entry.path
            ))
        })?;
    let size = AssetsArg {
        pairs: &[],
        pixels: None,
    }
    .size(&assets)?;
    run::decide(&entry.rubric, state, supplied, &assets, size, || {
        Ok(backend.clone())
    })
}

impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("jud", env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult::with_all_items(
            self.entries.iter().map(|e| e.tool.clone()).collect(),
        ))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let Some(entry) = self.entries.iter().find(|e| e.rubric.name == request.name) else {
            return Err(McpError::invalid_params(
                format!("no tool named `{}`", request.name),
                None,
            ));
        };
        let arguments = request.arguments.unwrap_or_default();
        let state = arguments
            .get("state")
            .cloned()
            .ok_or_else(|| McpError::invalid_params("`state` is required", None))?;
        let supplied: Supplied = match arguments.get("options") {
            None | Some(Value::Null) => Supplied::default(),
            Some(options) => serde_json::from_value(options.clone()).map_err(|e| {
                McpError::invalid_params(
                    format!(
                        "`options` is a map from question id to option key to description: {e}"
                    ),
                    None,
                )
            })?,
        };
        let (entry, backend) = (Arc::clone(entry), Arc::clone(&self.backend));
        let outcome =
            tokio::task::spawn_blocking(move || evaluate(&entry, &state, &supplied, &backend))
                .await
                .map_err(|e| {
                    McpError::internal_error(format!("the call did not finish: {e}"), None)
                })?;
        let result = match outcome {
            Ok(verdicts) => CallToolResult::success(vec![ContentBlock::text(verdicts)]),
            Err(failure) => CallToolResult::error(vec![ContentBlock::text(failure.message())]),
        };
        Ok(result.into())
    }
}
