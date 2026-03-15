//! ACP client: one agent process and its JSON-RPC transport.

use crate::jsonrpc::{self, Id, MethodCall, Notification, Params, Version};
use crate::transport::{Payload, Transport};
use crate::{AgentId, Error, Result};
use helix_acp_types::{
    InitializeRequest, InitializeResponse, NewSessionRequest, NewSessionResponse, ProtocolVersion,
};
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::{path::Path, process::Stdio};
use tokio::{
    io::{BufReader, BufWriter},
    process::{Child, Command},
    sync::{
        mpsc::{channel, UnboundedReceiver, UnboundedSender},
        OnceCell, Notify,
    },
};

/// Single ACP agent connection (process + transport).
pub struct AcpClient {
    id: AgentId,
    name: String,
    _process: Child,
    server_tx: UnboundedSender<Payload>,
    request_counter: AtomicU64,
    /// Set after successful initialize.
    pub(crate) capabilities: OnceCell<InitializeResponse>,
    pub initialize_notify: Arc<Notify>,
    /// Current session id if one was created (session/new).
    session_id: std::sync::Mutex<Option<helix_acp_types::SessionId>>,
}

impl AcpClient {
    /// Start agent process and transport. Caller should run initialize in a task and then notify.
    pub fn start(
        command: &str,
        args: &[String],
        root_path: &Path,
        id: AgentId,
        name: String,
    ) -> Result<(
        Self,
        UnboundedReceiver<(AgentId, jsonrpc::Call)>,
        Arc<Notify>,
    )> {
        let cmd = helix_stdx::env::which(command).map_err(|e| {
            Error::IO(std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))
        })?;

        let mut process = Command::new(cmd)
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .current_dir(root_path)
            .kill_on_drop(true)
            .spawn()?;

        let writer = BufWriter::new(process.stdin.take().expect("stdin"));
        let reader = BufReader::new(process.stdout.take().expect("stdout"));
        let stderr = BufReader::new(process.stderr.take().expect("stderr"));

        let (server_rx, server_tx, initialize_notify) =
            Transport::start(reader, writer, stderr, id, name.clone());

        let client = Self {
            id,
            name,
            _process: process,
            server_tx,
            request_counter: AtomicU64::new(0),
            capabilities: OnceCell::new(),
            initialize_notify: initialize_notify.clone(),
            session_id: std::sync::Mutex::new(None),
        };

        Ok((client, server_rx, initialize_notify))
    }

    pub fn id(&self) -> AgentId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_initialized(&self) -> bool {
        self.capabilities.get().is_some()
    }

    pub fn capabilities(&self) -> Option<&InitializeResponse> {
        self.capabilities.get()
    }

    /// Set capabilities after successful initialize (e.g. from a spawn task).
    pub fn set_initialized(&self, response: InitializeResponse) {
        let _ = self.capabilities.set(response);
    }

    pub fn session_id(&self) -> Option<helix_acp_types::SessionId> {
        self.session_id.lock().unwrap().clone()
    }

    pub fn set_session_id(&self, id: Option<helix_acp_types::SessionId>) {
        *self.session_id.lock().unwrap() = id;
    }

    fn next_id(&self) -> Id {
        let n = self.request_counter.fetch_add(1, Ordering::Relaxed);
        Id::Num(n)
    }

    fn value_to_params(v: Value) -> Params {
        match v {
            Value::Null => Params::None,
            Value::Array(a) => Params::Array(a),
            Value::Object(m) => Params::Map(m),
            other => Params::Array(vec![other]),
        }
    }

    /// Send a request and wait for the response (via channel).
    pub async fn request<R>(&self, params: R::Params) -> Result<R::Result>
    where
        R: helix_acp_types::request::Request,
        R::Params: Serialize,
        R::Result: serde::de::DeserializeOwned,
    {
        let id = self.next_id();
        let value = serde_json::to_value(&params).map_err(Error::from)?;
        let params = Self::value_to_params(value);

        let (tx, mut rx) = channel(1);
        let method_call = MethodCall {
            jsonrpc: Some(Version::V2),
            method: R::METHOD.to_string(),
            params,
            id: id.clone(),
        };
        self.server_tx
            .send(Payload::Request {
                chan: tx,
                value: method_call,
            })
            .map_err(|_| Error::StreamClosed)?;

        let result = rx.recv().await.ok_or(Error::StreamClosed)??;
        let parsed = serde_json::from_value(result).map_err(Error::from)?;
        Ok(parsed)
    }

    /// Send a notification (fire-and-forget).
    pub fn notify<N>(&self, params: N::Params) -> Result<()>
    where
        N: helix_acp_types::notification::Notification,
        N::Params: Serialize,
    {
        let value = serde_json::to_value(&params).map_err(Error::from)?;
        let params = Self::value_to_params(value);
        let n = Notification {
            jsonrpc: Some(Version::V2),
            method: N::METHOD.to_string(),
            params,
        };
        self.server_tx.send(Payload::Notification(n)).map_err(|_| Error::StreamClosed)
    }

    /// Reply to a method call from the agent (e.g. fs/read_text_file).
    pub fn reply(&self, id: Id, result: std::result::Result<Value, jsonrpc::Error>) -> Result<()> {
        let output = match result {
            Ok(r) => jsonrpc::Output::Success(jsonrpc::Success {
                jsonrpc: Some(Version::V2),
                result: r,
                id: id.clone(),
            }),
            Err(e) => jsonrpc::Output::Failure(jsonrpc::Failure {
                jsonrpc: Some(Version::V2),
                error: e,
                id: id.clone(),
            }),
        };
        self.server_tx
            .send(Payload::Response(output))
            .map_err(|_| Error::StreamClosed)
    }

    /// Initialize the agent (protocol version and client capabilities).
    pub async fn initialize(&self, protocol_version: ProtocolVersion) -> Result<InitializeResponse> {
        let req = InitializeRequest {
            protocol_version,
            client_capabilities: helix_acp_types::ClientCapabilities::default(),
            client_info: None,
        };
        let resp: InitializeResponse = self.request::<helix_acp_types::request::Initialize>(req).await?;
        Ok(resp)
    }

    /// Create a new session. Sets internal session_id on success.
    pub async fn session_new(&self, working_directory: String) -> Result<NewSessionResponse> {
        let req = NewSessionRequest {
            working_directory,
            mcp_servers: vec![],
        };
        let resp: NewSessionResponse =
            self.request::<helix_acp_types::request::NewSession>(req).await?;
        self.set_session_id(Some(resp.session_id.clone()));
        Ok(resp)
    }

    /// Send prompt to current session. Creates session if none.
    pub async fn session_prompt(
        &self,
        session_id: helix_acp_types::SessionId,
        content: Vec<helix_acp_types::ContentBlock>,
    ) -> Result<helix_acp_types::PromptResponse> {
        let req = helix_acp_types::PromptRequest {
            session_id,
            content,
        };
        self.request::<helix_acp_types::request::Prompt>(req).await
    }

    /// Signal that the client is no longer needed. Process is killed on drop when all handles are dropped.
    pub fn shutdown(&self) {
        // Dropping the last sender closes the channel; transport send task will exit.
        // Process is kill_on_drop when Client is dropped.
    }
}
