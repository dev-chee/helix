//! ACP Client implementation

use crate::registry::{AgentId, TransportType};
use crate::transport::{Call, Id, Notification, Output, Params, Payload, Request, Transport};
use crate::{Error, Result};
use helix_acp_types as acp;
use serde_json::Value;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::{future::Future, process::Stdio};
use tokio::io::{BufReader, BufWriter};
use tokio::process::{Child, Command};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::sync::{mpsc::UnboundedSender, Notify, OnceCell};

/// A connected AI agent
#[derive(Debug)]
pub struct Client {
    id: AgentId,
    name: String,
    _process: Child,
    server_tx: UnboundedSender<Payload>,
    request_counter: AtomicU64,
    capabilities: OnceCell<acp::AgentCapabilities>,
    config: Option<Value>,
    root_path: PathBuf,
    initialize_notify: Arc<Notify>,
    req_timeout: u64,
}

impl Client {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        cmd: &str,
        args: &[String],
        config: Option<Value>,
        server_environment: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
        root_path: PathBuf,
        id: AgentId,
        name: String,
        req_timeout: u64,
        transport: TransportType,
    ) -> Result<(Self, UnboundedReceiver<Call>, Arc<Notify>)> {
        let cmd = helix_stdx::env::which(cmd)?;

        let process = Command::new(cmd)
            .envs(server_environment)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(&root_path)
            .kill_on_drop(true)
            .spawn();

        let mut process = process?;

        let writer = BufWriter::new(process.stdin.take().expect("Failed to open stdin"));
        let reader = BufReader::new(process.stdout.take().expect("Failed to open stdout"));
        let stderr = BufReader::new(process.stderr.take().expect("Failed to open stderr"));

        let (server_rx, server_tx, initialize_notify) =
            Transport::start(reader, writer, stderr, &name, transport);

        let client = Self {
            id,
            name,
            _process: process,
            server_tx,
            request_counter: AtomicU64::new(0),
            capabilities: OnceCell::new(),
            config,
            req_timeout,
            root_path,
            initialize_notify: initialize_notify.clone(),
        };

        Ok((client, server_rx, initialize_notify))
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn id(&self) -> AgentId {
        self.id
    }

    fn next_request_id(&self) -> Id {
        let id = self.request_counter.fetch_add(1, Ordering::Relaxed);
        Id::Num(id)
    }

    fn value_into_params(value: Value) -> Params {
        match value {
            Value::Null => Params::None,
            Value::Bool(_) | Value::Number(_) | Value::String(_) => Params::Array(vec![value]),
            Value::Array(vec) => Params::Array(vec),
            Value::Object(map) => Params::Map(map),
        }
    }

    pub fn is_initialized(&self) -> bool {
        self.capabilities.get().is_some()
    }

    pub fn capabilities(&self) -> &acp::AgentCapabilities {
        self.capabilities
            .get()
            .expect("agent not yet initialized!")
    }

    pub fn config(&self) -> Option<&Value> {
        self.config.as_ref()
    }

    /// Execute a RPC request on the agent
    fn call<R: acp::Request>(&self, params: R::Params) -> impl Future<Output = Result<R::Result>>
    where
        R::Params: serde::Serialize,
    {
        let server_tx = self.server_tx.clone();
        let id = self.next_request_id();
        let timeout_secs = self.req_timeout;

        let rx = serde_json::to_value(&params)
            .map_err(Error::from)
            .and_then(|params| {
                let request = Request {
                    jsonrpc: Some(crate::transport::Version::V2),
                    id: id.clone(),
                    method: R::METHOD.to_string(),
                    params: Self::value_into_params(params),
                };
                let (tx, rx) = tokio::sync::mpsc::channel::<Result<Value>>(1);
                server_tx
                    .send(Payload::Request {
                        chan: tx,
                        value: request,
                    })
                    .map_err(|e| Error::Other(e.into()))?;
                Ok(rx)
            });

        async move {
            use std::time::Duration;
            use tokio::time::timeout;

            let id_num = match &id {
                Id::Num(n) => *n,
                _ => 0,
            };

            timeout(Duration::from_secs(timeout_secs), rx?.recv())
                .await
                .map_err(|_| Error::Timeout(id_num))?
                .ok_or(Error::StreamClosed)?
                .and_then(|value| serde_json::from_value(value).map_err(Into::into))
        }
    }

    /// Send a RPC notification to the agent
    pub fn notify<N: acp::Notification>(&self, params: N::Params)
    where
        N::Params: serde::Serialize,
    {
        let server_tx = self.server_tx.clone();

        let params = match serde_json::to_value(&params) {
            Ok(params) => params,
            Err(err) => {
                log::error!(
                    "Failed to serialize params for notification '{}' for agent '{}': {err}",
                    N::METHOD,
                    self.name,
                );
                return;
            }
        };

        let notification = Notification {
            jsonrpc: Some(crate::transport::Version::V2),
            method: N::METHOD.to_string(),
            params: Self::value_into_params(params),
        };

        if let Err(err) = server_tx.send(Payload::Notification(notification)) {
            log::error!(
                "Failed to send notification '{}' to agent '{}': {err}",
                N::METHOD,
                self.name
            );
        }
    }

    /// Reply to an agent request
    pub fn reply(&self, id: Id, result: core::result::Result<Value, crate::RpcError>) -> Result<()> {
        let server_tx = self.server_tx.clone();

        let output = match result {
            Ok(result) => Output::Success(crate::transport::Success {
                jsonrpc: Some(crate::transport::Version::V2),
                id,
                result,
            }),
            Err(error) => Output::Failure(crate::transport::Failure {
                jsonrpc: Some(crate::transport::Version::V2),
                id,
                error: crate::transport::JsonRpcError {
                    code: match &error {
                        crate::RpcError::JsonRpc { code, .. } => *code,
                    },
                    message: match &error {
                        crate::RpcError::JsonRpc { message, .. } => message.clone(),
                    },
                    data: match &error {
                        crate::RpcError::JsonRpc { data, .. } => data.clone(),
                    },
                },
            }),
        };

        server_tx
            .send(Payload::Response(output))
            .map_err(|e| Error::Other(e.into()))?;

        Ok(())
    }

    // =========================================================================
    // ACP Methods
    // =========================================================================

    /// Initialize the connection with the agent
    pub async fn initialize(&self, _enable_tools: bool) -> Result<acp::InitializeResponse> {
        let params = acp::InitializeRequest {
            protocol_version: acp::PROTOCOL_VERSION.to_string(),
            client_capabilities: acp::ClientCapabilities {
                file_operations: Some(acp::FileOperationCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                    create_directory: Some(true),
                    delete_file: Some(true),
                    list_directory: Some(true),
                    watch_files: Some(true),
                }),
                terminal: Some(acp::TerminalCapabilities {
                    create_terminal: true,
                    terminal_output: true,
                    release_terminal: true,
                    wait_for_terminal_exit: true,
                    kill_terminal_command: true,
                }),
                permissions: Some(acp::PermissionCapabilities {
                    request_permission: true,
                    supported_permissions: vec![
                        "file_read".to_string(),
                        "file_write".to_string(),
                        "execute".to_string(),
                    ],
                }),
                diff_preview: Some(acp::DiffPreviewCapabilities {
                    inline_preview: true,
                    side_by_side: Some(false),
                    unified_diff: Some(true),
                }),
            },
            client_info: Some(acp::Implementation {
                name: "helix".to_string(),
                title: Some("Helix Editor".to_string()),
                version: helix_loader::VERSION_AND_GIT_HASH.to_string(),
            }),
            meta: None,
        };

        let response = self.call::<acp::requests::Initialize>(params).await?;

        if let Err(_) = self.capabilities.set(response.agent_capabilities.clone()) {
            log::warn!("Agent capabilities already set");
        }

        self.initialize_notify.notify_waiters();

        Ok(response)
    }

    /// Create a new session with the agent
    pub async fn new_session(&self, cwd: PathBuf) -> Result<acp::NewSessionResponse> {
        let params = acp::NewSessionRequest {
            mcp_servers: Vec::new(),
            cwd,
            meta: None,
        };

        self.call::<acp::requests::NewSession>(params).await
    }

    /// Send a prompt to the agent
    pub async fn prompt(
        &self,
        session_id: &acp::SessionId,
        prompt: Vec<acp::ContentBlock>,
    ) -> Result<acp::PromptResponse> {
        let params = acp::PromptRequest {
            session_id: session_id.clone(),
            prompt,
            meta: None,
        };

        self.call::<acp::requests::Prompt>(params).await
    }

    /// Cancel a session
    pub async fn cancel_session(
        &self,
        session_id: &acp::SessionId,
        reason: Option<String>,
    ) -> Result<acp::CancelSessionResponse> {
        let params = acp::CancelSessionRequest {
            session_id: session_id.clone(),
            reason,
            meta: None,
        };

        self.call::<acp::requests::CancelSession>(params).await
    }

    /// Force shutdown the agent
    pub async fn force_shutdown(&self) -> Result<()> {
        // Send exit notification
        self.notify::<acp::notifications::Exit>(acp::ExitParams {});
        Ok(())
    }
}

/// Trait for types that can handle ACP requests
#[async_trait::async_trait(?Send)]
pub trait ClientHandler: Send + Sync {
    async fn request_permission(
        &self,
        args: acp::RequestPermissionRequest,
    ) -> Result<acp::RequestPermissionResponse>;

    async fn read_text_file(
        &self,
        args: acp::ReadTextFileRequest,
    ) -> Result<acp::ReadTextFileResponse>;

    async fn write_text_file(
        &self,
        args: acp::WriteTextFileRequest,
    ) -> Result<acp::WriteTextFileResponse>;

    async fn create_terminal(
        &self,
        args: acp::CreateTerminalRequest,
    ) -> Result<acp::CreateTerminalResponse>;

    async fn terminal_output(
        &self,
        args: acp::TerminalOutputRequest,
    ) -> Result<acp::TerminalOutputResponse>;

    async fn release_terminal(
        &self,
        args: acp::ReleaseTerminalRequest,
    ) -> Result<acp::ReleaseTerminalResponse>;

    async fn wait_for_terminal_exit(
        &self,
        args: acp::WaitForTerminalExitRequest,
    ) -> Result<acp::WaitForTerminalExitResponse>;

    async fn kill_terminal_command(
        &self,
        args: acp::KillTerminalCommandRequest,
    ) -> Result<acp::KillTerminalCommandResponse>;

    async fn session_notification(&self, args: acp::SessionNotification) -> Result<()>;

    async fn ext_method(&self, args: acp::ExtRequest) -> Result<acp::ExtResponse>;

    async fn ext_notification(&self, args: acp::ExtNotification) -> Result<()>;
}
