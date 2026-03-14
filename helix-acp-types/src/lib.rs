//! ACP (Agent Client Protocol) type definitions.
//!
//! ACP standardizes communication between code editors and AI coding agents
//! using JSON-RPC 2.0 over stdio, similar to LSP and DAP.
//!
//! See <https://agentclientprotocol.com> for the specification.

pub mod jsonrpc;
pub mod types;

pub use types::*;
