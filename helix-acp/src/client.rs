//! `AgentClient` — manages one ACP agent subprocess and its sessions.

use crate::{
    transport::{AgentClientIdRaw, Payload, Transport},
    Call, Error, Result,
};
use helix_acp_types::{
    jsonrpc::{self, Id, Params, Version},
    *,
};
use log::error;
use parking_lot::Mutex;
use serde_json::Value;
use slotmap::{new_key_type, Key};
use std::{
    path::PathBuf,
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::{
    io::{BufReader, BufWriter},
    process::{Child, Command},
    sync::{
        mpsc::{channel, UnboundedSender},
        Notify, OnceCell,
    },
    time::{timeout, Duration},
};

new_key_type! {
    /// Stable identifier for an `AgentClient` within the `Registry`.
    pub struct AgentClientId;
}

/// Configuration for launching one ACP agent.
#[derive(Debug, Clone)]
pub struct AgentClientConfig {
    /// Human-readable name (from `[[agent.servers]]` in `config.toml`).
    pub name: String,
    /// Path to the agent executable.
    pub command: PathBuf,
    /// Arguments to pass to the agent.
    pub args: Vec<String>,
    /// Environment variables to set.
    pub env: Vec<(String, String)>,
    /// Request timeout in seconds.
    pub timeout_secs: u64,
}

impl Default for AgentClientConfig {
    fn default() -> Self {
        Self {
            name: String::new(),
            command: PathBuf::new(),
            args: Vec::new(),
            env: Vec::new(),
            timeout_secs: 60,
        }
    }
}

/// Active state of an ACP session within this client.
#[derive(Debug)]
pub struct SessionState {
    pub session_id: SessionId,
    /// True while a `session/prompt` request is in flight.
    pub running: bool,
}

/// A cheaply cloneable handle that allows async operations on an agent
/// from background tasks without holding a reference to the `AgentClient`.
#[derive(Clone)]
pub struct ClientHandle {
    server_tx: UnboundedSender<Payload>,
    request_counter: Arc<AtomicU64>,
    pub(crate) capabilities: Arc<OnceCell<AgentCapabilities>>,
    initialize_notify: Arc<Notify>,
    sessions: Arc<Mutex<Vec<SessionState>>>,
    pub timeout_secs: u64,
}

impl ClientHandle {
    fn next_id(&self) -> Id {
        Id::Num(self.request_counter.fetch_add(1, Ordering::Relaxed))
    }

    async fn request_timeout_inner(
        &self,
        method: &str,
        params: impl serde::Serialize,
    ) -> Result<Value> {
        let id = self.next_id();
        let (tx, mut rx) = channel(1);
        let value = jsonrpc::MethodCall {
            jsonrpc: Some(Version::V2),
            method: method.to_string(),
            params: Params::Map(
                serde_json::to_value(params)?
                    .as_object()
                    .cloned()
                    .unwrap_or_default(),
            ),
            id: id.clone(),
        };
        self.server_tx
            .send(Payload::Request { chan: tx, value })
            .map_err(|_| Error::StreamClosed)?;

        let dur = Duration::from_secs(self.timeout_secs);
        match timeout(dur, rx.recv()).await {
            Ok(Some(result)) => result,
            Ok(None) => Err(Error::StreamClosed),
            Err(_elapsed) => Err(Error::Timeout(id)),
        }
    }

    /// Perform the `initialize` handshake. Consumes `self`.
    pub async fn initialize(self) -> Result<InitializeResult> {
        let params = InitializeParams {
            protocol_version: PROTOCOL_VERSION,
            client_capabilities: ClientCapabilities {
                fs: Some(FsCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                }),
                terminal: None,
            },
            client_info: Some(ImplementationInfo {
                name: "helix".to_string(),
                title: Some("Helix Editor".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
        };
        let result_value = self.request_timeout_inner("initialize", &params).await?;
        let result: InitializeResult = serde_json::from_value(result_value)?;
        let _ = self.capabilities.set(result.agent_capabilities.clone());
        self.initialize_notify.notify_waiters();
        Ok(result)
    }

    /// Create a new session and return the session ID.
    pub async fn new_session(
        &self,
        cwd: PathBuf,
        mcp_servers: Vec<McpServerConfig>,
    ) -> Result<SessionId> {
        let params = SessionNewParams { cwd, mcp_servers };
        let result_value = self.request_timeout_inner("session/new", &params).await?;
        let result: SessionNewResult = serde_json::from_value(result_value)?;
        self.sessions.lock().push(SessionState {
            session_id: result.session_id.clone(),
            running: false,
        });
        Ok(result.session_id)
    }

    /// Send a prompt and wait for the agent's `StopReason`.
    pub async fn prompt(
        &self,
        session_id: SessionId,
        prompt: Vec<ContentBlock>,
    ) -> Result<StopReason> {
        {
            let mut sessions = self.sessions.lock();
            if let Some(s) = sessions.iter_mut().find(|s| s.session_id == session_id) {
                s.running = true;
            }
        }
        let params = SessionPromptParams {
            session_id: session_id.clone(),
            prompt,
        };
        let result_value = self
            .request_timeout_inner("session/prompt", &params)
            .await?;
        let result: SessionPromptResult = serde_json::from_value(result_value)?;
        {
            let mut sessions = self.sessions.lock();
            if let Some(s) = sessions.iter_mut().find(|s| s.session_id == session_id) {
                s.running = false;
            }
        }
        Ok(result.stop_reason)
    }
}

pub struct AgentClient {
    pub id: AgentClientId,
    pub name: String,
    _process: Child,
    server_tx: UnboundedSender<Payload>,
    request_counter: Arc<AtomicU64>,
    pub capabilities: Arc<OnceCell<AgentCapabilities>>,
    initialize_notify: Arc<Notify>,
    sessions: Arc<Mutex<Vec<SessionState>>>,
    timeout_secs: u64,
}

impl AgentClient {
    /// Spawn the agent subprocess and start the transport tasks.
    pub fn start(
        id: AgentClientId,
        config: &AgentClientConfig,
        call_tx: UnboundedSender<(AgentClientId, Call)>,
    ) -> Result<Self> {
        let mut cmd = Command::new(&config.command);
        cmd.args(&config.args)
            .envs(config.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut process = cmd.spawn()?;

        let stdout = BufReader::new(process.stdout.take().expect("stdout pipe"));
        let stdin = BufWriter::new(process.stdin.take().expect("stdin pipe"));
        let stderr = BufReader::new(process.stderr.take().expect("stderr pipe"));

        // The transport uses a raw u64 key to stay free of SlotMap dependency.
        // We extract the key's index bits here as a unique ID.
        let raw_id: AgentClientIdRaw = id.data().as_ffi();

        // Bridge from (AgentClientIdRaw, Call) → (AgentClientId, Call).
        let (raw_rx, server_tx, initialize_notify) =
            Transport::start(raw_id, config.name.clone(), stdout, stdin, stderr);

        // Spawn a task that re-tags raw IDs with the typed AgentClientId.
        tokio::spawn(async move {
            let mut rx = raw_rx;
            while let Some((_raw, call)) = rx.recv().await {
                if call_tx.send((id, call)).is_err() {
                    break;
                }
            }
        });

        Ok(Self {
            id,
            name: config.name.clone(),
            _process: process,
            server_tx,
            request_counter: Arc::new(AtomicU64::new(0)),
            capabilities: Arc::new(OnceCell::new()),
            initialize_notify,
            sessions: Arc::new(Mutex::new(Vec::new())),
            timeout_secs: config.timeout_secs,
        })
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Return a cheaply cloneable async handle to this client.
    pub fn handle(&self) -> ClientHandle {
        ClientHandle {
            server_tx: self.server_tx.clone(),
            request_counter: Arc::clone(&self.request_counter),
            capabilities: Arc::clone(&self.capabilities),
            initialize_notify: Arc::clone(&self.initialize_notify),
            sessions: Arc::clone(&self.sessions),
            timeout_secs: self.timeout_secs,
        }
    }

    fn next_id(&self) -> Id {
        Id::Num(self.request_counter.fetch_add(1, Ordering::Relaxed))
    }

    /// Send a request and wait for the response JSON value.
    async fn request_timeout(
        &self,
        method: &str,
        params: impl serde::Serialize,
    ) -> Result<Value> {
        let id = self.next_id();
        let (tx, mut rx) = channel(1);
        let value = jsonrpc::MethodCall {
            jsonrpc: Some(Version::V2),
            method: method.to_string(),
            params: Params::Map(serde_json::to_value(params)?.as_object().cloned().unwrap_or_default()),
            id: id.clone(),
        };
        self.server_tx
            .send(Payload::Request { chan: tx, value })
            .map_err(|_| Error::StreamClosed)?;

        let dur = Duration::from_secs(self.timeout_secs);
        match timeout(dur, rx.recv()).await {
            Ok(Some(result)) => result,
            Ok(None) => Err(Error::StreamClosed),
            Err(_elapsed) => Err(Error::Timeout(id)),
        }
    }

    fn notify(&self, method: &str, params: impl serde::Serialize) -> Result<()> {
        let params_value = serde_json::to_value(params)?;
        let params = if let Some(map) = params_value.as_object() {
            Params::Map(map.clone())
        } else {
            Params::None
        };
        let notif = jsonrpc::Notification {
            jsonrpc: Some(Version::V2),
            method: method.to_string(),
            params,
        };
        self.server_tx
            .send(Payload::Notification(notif))
            .map_err(|_| Error::StreamClosed)
    }

    fn respond_ok(&self, id: Id, result: impl serde::Serialize) -> Result<()> {
        let value = serde_json::to_value(result)?;
        let output = jsonrpc::Output::Success(jsonrpc::Success {
            jsonrpc: Some(Version::V2),
            result: value,
            id,
        });
        self.server_tx
            .send(Payload::Response(output))
            .map_err(|_| Error::StreamClosed)
    }

    fn respond_err(&self, id: Id, error: jsonrpc::Error) -> Result<()> {
        let output = jsonrpc::Output::Failure(jsonrpc::Failure {
            jsonrpc: Some(Version::V2),
            error,
            id,
        });
        self.server_tx
            .send(Payload::Response(output))
            .map_err(|_| Error::StreamClosed)
    }

    // -----------------------------------------------------------------------
    // Protocol methods (Client → Agent)
    // -----------------------------------------------------------------------

    /// Perform the `initialize` handshake.
    ///
    /// Must be called once before any session methods.
    pub async fn initialize(&self) -> Result<InitializeResult> {
        let params = InitializeParams {
            protocol_version: PROTOCOL_VERSION,
            client_capabilities: ClientCapabilities {
                fs: Some(FsCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                }),
                terminal: None,
            },
            client_info: Some(ImplementationInfo {
                name: "helix".to_string(),
                title: Some("Helix Editor".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }),
        };

        let result_value = self.request_timeout("initialize", &params).await?;
        let result: InitializeResult = serde_json::from_value(result_value)?;

        // Store capabilities and signal transport that initialization is done.
        let _ = self.capabilities.set(result.agent_capabilities.clone());
        self.initialize_notify.notify_waiters();

        Ok(result)
    }

    /// Create a new session and return the session ID.
    pub async fn new_session(
        &self,
        cwd: PathBuf,
        mcp_servers: Vec<McpServerConfig>,
    ) -> Result<SessionId> {
        let params = SessionNewParams { cwd, mcp_servers };
        let result_value = self.request_timeout("session/new", &params).await?;
        let result: SessionNewResult = serde_json::from_value(result_value)?;

        self.sessions.lock().push(SessionState {
            session_id: result.session_id.clone(),
            running: false,
        });

        Ok(result.session_id)
    }

    /// Send a prompt and wait for the agent's `StopReason`.
    ///
    /// The agent will stream `session/update` notifications over the call
    /// channel while this future is pending.
    pub async fn prompt(
        &self,
        session_id: SessionId,
        prompt: Vec<ContentBlock>,
    ) -> Result<StopReason> {
        {
            let mut sessions = self.sessions.lock();
            if let Some(s) = sessions.iter_mut().find(|s| s.session_id == session_id) {
                s.running = true;
            }
        }

        let params = SessionPromptParams {
            session_id: session_id.clone(),
            prompt,
        };
        let result_value = self.request_timeout("session/prompt", &params).await?;
        let result: SessionPromptResult = serde_json::from_value(result_value)?;

        {
            let mut sessions = self.sessions.lock();
            if let Some(s) = sessions.iter_mut().find(|s| s.session_id == session_id) {
                s.running = false;
            }
        }

        Ok(result.stop_reason)
    }

    /// Cancel the current prompt turn for a session.
    pub fn cancel(&self, session_id: &SessionId) {
        let params = SessionCancelParams {
            session_id: session_id.clone(),
        };
        if let Err(e) = self.notify("session/cancel", &params) {
            error!("ACP {}: failed to send cancel: {e}", self.name);
        }
    }

    // -----------------------------------------------------------------------
    // Responses to agent→client requests
    // -----------------------------------------------------------------------

    /// Respond to a `session/request_permission` request.
    pub fn reply_permission(&self, id: Id, outcome: PermissionOutcome) {
        let result = RequestPermissionResult { outcome };
        if let Err(e) = self.respond_ok(id, result) {
            error!("ACP {}: failed to send permission reply: {e}", self.name);
        }
    }

    /// Respond to a `session/request_permission` with cancellation.
    pub fn reply_permission_cancelled(&self, id: Id) {
        self.reply_permission(id, PermissionOutcome::Cancelled);
    }

    /// Respond to a `fs/read_text_file` request with the file's content.
    pub fn reply_read_file(&self, id: Id, content: String) {
        let result = FsReadTextFileResult { content };
        if let Err(e) = self.respond_ok(id, result) {
            error!("ACP {}: failed to send read_file reply: {e}", self.name);
        }
    }

    /// Respond to a `fs/read_text_file` with an error (e.g. file not found).
    pub fn reply_read_file_error(&self, id: Id, message: String) {
        let err = jsonrpc::Error {
            code: helix_acp_types::jsonrpc::ErrorCode::InternalError,
            message,
            data: None,
        };
        if let Err(e) = self.respond_err(id, err) {
            error!("ACP {}: failed to send read_file error: {e}", self.name);
        }
    }

    /// Respond to a `fs/write_text_file` request with success (null result).
    pub fn reply_write_file_ok(&self, id: Id) {
        if let Err(e) = self.respond_ok(id, serde_json::Value::Null) {
            error!("ACP {}: failed to send write_file reply: {e}", self.name);
        }
    }

    /// Respond to a `fs/write_text_file` with an error.
    pub fn reply_write_file_error(&self, id: Id, message: String) {
        let err = jsonrpc::Error {
            code: helix_acp_types::jsonrpc::ErrorCode::InternalError,
            message,
            data: None,
        };
        if let Err(e) = self.respond_err(id, err) {
            error!("ACP {}: failed to send write_file error: {e}", self.name);
        }
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    pub fn capabilities(&self) -> Option<&AgentCapabilities> {
        self.capabilities.get()
    }

    pub fn is_initialized(&self) -> bool {
        self.capabilities.initialized()
    }
}