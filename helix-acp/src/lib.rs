//! ACP (Agent Client Protocol) client implementation for Helix.
//!
//! This crate provides the client-side implementation for communicating with
//! AI coding agents using the Agent Client Protocol.

mod client;
mod registry;
pub mod transport;

pub use client::Client;
pub use helix_acp_types as types;
pub use registry::{AgentConfiguration, AgentId, Registry};
pub use transport::{Call, Payload};

use thiserror::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("protocol error: {0}")]
    Rpc(#[from] RpcError),
    #[error("failed to parse: {0}")]
    Parse(Box<dyn std::error::Error + Send + Sync>),
    #[error("IO Error: {0}")]
    IO(#[from] std::io::Error),
    #[error("request {0} timed out")]
    Timeout(u64),
    #[error("server closed the stream")]
    StreamClosed,
    #[error("Unhandled")]
    Unhandled,
    #[error(transparent)]
    ExecutableNotFound(#[from] helix_stdx::env::ExecutableNotFoundError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Parse(Box::new(value))
    }
}

impl From<sonic_rs::Error> for Error {
    fn from(value: sonic_rs::Error) -> Self {
        Self::Parse(Box::new(value))
    }
}

#[derive(Error, Debug)]
pub enum RpcError {
    #[error("JSON-RPC error: code={code}, message={message}")]
    JsonRpc {
        code: i64,
        message: String,
        data: Option<serde_json::Value>,
    },
}

impl From<helix_acp_types::Error> for RpcError {
    fn from(err: helix_acp_types::Error) -> Self {
        RpcError::JsonRpc {
            code: err.code,
            message: err.message,
            data: err.data,
        }
    }
}

/// Error during client startup
#[derive(Debug)]
pub enum StartupError {
    /// The required root for the server was not found
    NoRequiredRootFound,
    /// Other error during startup
    Error(Error),
}

impl From<Error> for StartupError {
    fn from(err: Error) -> Self {
        StartupError::Error(err)
    }
}

impl From<anyhow::Error> for StartupError {
    fn from(err: anyhow::Error) -> Self {
        StartupError::Error(Error::Other(err))
    }
}
