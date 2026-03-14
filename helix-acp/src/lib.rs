//! ACP (Agent Client Protocol) client library.
//!
//! Implements the client side of ACP: spawning agent sub-processes,
//! performing the initialization handshake, managing sessions, and routing
//! bidirectional JSON-RPC messages.

mod client;
mod transport;
pub mod registry;

pub use client::{AgentClient, AgentClientConfig, AgentClientId, ClientHandle};
pub use helix_acp_types as types;
pub use registry::Registry;
pub use transport::Payload;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("protocol error: {0}")]
    Rpc(#[from] helix_acp_types::jsonrpc::Error),
    #[error("failed to parse: {0}")]
    Parse(Box<dyn std::error::Error + Send + Sync>),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("request {0} timed out")]
    Timeout(helix_acp_types::jsonrpc::Id),
    #[error("agent closed the stream")]
    StreamClosed,
    #[error("unhandled ACP method")]
    Unhandled,
    #[error(transparent)]
    ExecutableNotFound(#[from] helix_stdx::env::ExecutableNotFoundError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Parse(Box::new(e))
    }
}

pub type Result<T, E = Error> = core::result::Result<T, E>;

// ---------------------------------------------------------------------------
// Public call types — what we expose to helix-view
// ---------------------------------------------------------------------------

/// A parsed notification from the agent (no response needed).
#[derive(Debug)]
pub enum Notification {
    /// `session/update` – the main streaming update from the agent.
    SessionUpdate(helix_acp_types::SessionUpdateParams),
    /// Agent exited (injected by the transport on stream close).
    Exit,
}

/// A parsed method-call from the agent (a response is required).
#[derive(Debug)]
pub enum MethodCall {
    /// `session/request_permission` – agent asks the user to approve a tool.
    RequestPermission {
        id: helix_acp_types::jsonrpc::Id,
        params: helix_acp_types::RequestPermissionParams,
    },
    /// `fs/read_text_file` – agent reads from the editor's file state.
    ReadTextFile {
        id: helix_acp_types::jsonrpc::Id,
        params: helix_acp_types::FsReadTextFileParams,
    },
    /// `fs/write_text_file` – agent writes via the editor.
    WriteTextFile {
        id: helix_acp_types::jsonrpc::Id,
        params: helix_acp_types::FsWriteTextFileParams,
    },
}

/// A message from the agent forwarded to helix-view.
#[derive(Debug)]
pub enum Call {
    Notification(Notification),
    MethodCall(MethodCall),
}

impl Call {
    /// Parse a raw JSON-RPC call into a typed `Call`.
    pub(crate) fn parse(
        call: helix_acp_types::jsonrpc::Call,
    ) -> core::result::Result<Self, Error> {
        use helix_acp_types::jsonrpc::Call as RawCall;

        match call {
            RawCall::MethodCall(mc) => {
                let id = mc.id.clone();
                let params = mc.params;
                match mc.method.as_str() {
                    "session/request_permission" => {
                        let p: helix_acp_types::RequestPermissionParams = params.parse()?;
                        Ok(Call::MethodCall(MethodCall::RequestPermission { id, params: p }))
                    }
                    "fs/read_text_file" => {
                        let p: helix_acp_types::FsReadTextFileParams = params.parse()?;
                        Ok(Call::MethodCall(MethodCall::ReadTextFile { id, params: p }))
                    }
                    "fs/write_text_file" => {
                        let p: helix_acp_types::FsWriteTextFileParams = params.parse()?;
                        Ok(Call::MethodCall(MethodCall::WriteTextFile { id, params: p }))
                    }
                    _ => Err(Error::Unhandled),
                }
            }
            RawCall::Notification(n) => {
                match n.method.as_str() {
                    "session/update" => {
                        let p: helix_acp_types::SessionUpdateParams = n.params.parse()?;
                        Ok(Call::Notification(Notification::SessionUpdate(p)))
                    }
                    _ => Err(Error::Unhandled),
                }
            }
            RawCall::Invalid { .. } => Err(Error::Unhandled),
        }
    }
}
