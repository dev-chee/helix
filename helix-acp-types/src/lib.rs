//! Types for interaction with AI coding agents using the Agent Client Protocol (ACP).
//!
//! ACP standardizes communication between code editors and AI coding agents.
//! See: https://agentclientprotocol.com/

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Re-export all modules
mod capabilities;
mod content;
mod protocol;
mod session;
mod tool;

pub use capabilities::*;
pub use content::*;
pub use protocol::*;
pub use session::*;
pub use tool::*;

/// Empty parameters type
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyParams {}

/// Empty result type
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmptyResult {}

/// Exit parameters
pub type ExitParams = EmptyParams;

/// Protocol version
pub const PROTOCOL_VERSION: &str = "1.0.0";

/// Unique identifier type used throughout ACP
pub type Id = String;

/// Implementation information for client or agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    /// Name/identifier of the implementation
    pub name: String,
    /// Human-readable title
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Version string
    pub version: String,
}

/// Generic result wrapper for responses
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Result<T> {
    Ok(T),
    Err(Error),
}

/// Error type for ACP operations
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Error {
    pub fn method_not_found() -> Self {
        Self {
            code: -32601,
            message: "Method not found".to_string(),
            data: None,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: None,
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: message.into(),
            data: None,
        }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Meta information that can be attached to requests
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Map<String, serde_json::Value>>,
}
