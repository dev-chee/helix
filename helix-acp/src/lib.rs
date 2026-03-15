//! Agent Client Protocol (ACP) client implementation.
//!
//! JSON-RPC and transport are implemented in this crate (no dependency on helix-lsp).

#![forbid(unsafe_code)]

pub mod client;
pub mod jsonrpc;
pub mod registry;
pub mod transport;

pub use client::AcpClient;
pub use registry::AgentRegistry;
pub use transport::{AgentId, Payload};

use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("protocol error: {0}")]
    Rpc(#[from] jsonrpc::Error),
    #[error("failed to parse: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("request {0:?} timed out")]
    Timeout(jsonrpc::Id),
    #[error("stream closed")]
    StreamClosed,
}
