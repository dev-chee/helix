//! Tool-related types for ACP

use serde::{Deserialize, Serialize};

/// Tool call request from agent to client
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    /// Unique ID for this tool call
    pub id: String,
    /// Name of the tool to call
    pub name: String,
    /// Arguments for the tool call
    #[serde(default)]
    pub arguments: serde_json::Value,
}

/// Tool result from client to agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    /// ID of the tool call this is responding to
    pub tool_call_id: String,
    /// The result content
    pub content: Vec<super::ContentBlock>,
    /// Whether the tool call was successful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

// ============================================================================
// Client-side request/response types (editor implements these)
// ============================================================================

/// Request permission from user
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionRequest {
    /// Permission being requested
    pub permission: String,
    /// Human-readable description
    pub description: String,
    /// Optional details about the request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Response to permission request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionResponse {
    /// Whether permission was granted
    pub granted: bool,
    /// Optional reason for denial
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Read text file request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTextFileRequest {
    /// Path to the file
    pub path: std::path::PathBuf,
    /// Encoding to use (defaults to utf-8)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}

/// Response to read text file request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTextFileResponse {
    /// File content
    pub content: String,
    /// Encoding used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
}

/// Write text file request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteTextFileRequest {
    /// Path to the file
    pub path: std::path::PathBuf,
    /// Content to write
    pub content: String,
    /// Whether to create parent directories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_directories: Option<bool>,
    /// Whether to overwrite existing file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overwrite: Option<bool>,
}

/// Response to write text file request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteTextFileResponse {}

/// Create terminal request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTerminalRequest {
    /// Command to run
    pub command: String,
    /// Arguments for the command
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<std::path::PathBuf>,
    /// Environment variables
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
    /// Terminal dimensions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<TerminalDimensions>,
}

/// Terminal dimensions
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalDimensions {
    /// Number of columns
    pub cols: u16,
    /// Number of rows
    pub rows: u16,
}

/// Response to create terminal request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTerminalResponse {
    /// Terminal ID
    pub terminal_id: String,
}

/// Terminal output request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputRequest {
    /// Terminal ID
    pub terminal_id: String,
}

/// Response to terminal output request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutputResponse {
    /// Output content
    pub output: String,
    /// Whether the terminal has exited
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exited: Option<bool>,
    /// Exit code if exited
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
}

/// Release terminal request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseTerminalRequest {
    /// Terminal ID
    pub terminal_id: String,
}

/// Response to release terminal request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseTerminalResponse {}

/// Wait for terminal exit request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitForTerminalExitRequest {
    /// Terminal ID
    pub terminal_id: String,
    /// Timeout in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

/// Response to wait for terminal exit request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitForTerminalExitResponse {
    /// Exit code
    pub exit_code: i32,
}

/// Kill terminal command request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillTerminalCommandRequest {
    /// Terminal ID
    pub terminal_id: String,
}

/// Response to kill terminal command request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KillTerminalCommandResponse {}

// ============================================================================
// Extension methods for future extensibility
// ============================================================================

/// Extension method request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtRequest {
    /// Extension method name
    pub method: String,
    /// Extension method params
    #[serde(default)]
    pub params: serde_json::Value,
}

/// Extension method response
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtResponse {
    /// Response content
    pub result: serde_json::Value,
}

/// Extension notification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtNotification {
    /// Extension method name
    pub method: String,
    /// Extension method params
    #[serde(default)]
    pub params: serde_json::Value,
}
