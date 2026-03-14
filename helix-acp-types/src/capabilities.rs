//! Capability types for ACP

use serde::{Deserialize, Serialize};

/// Client capabilities - what the client supports
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// File operations the client supports
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_operations: Option<FileOperationCapabilities>,
    /// Terminal capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<TerminalCapabilities>,
    /// Permission handling capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PermissionCapabilities>,
    /// Diff preview capabilities
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diff_preview: Option<DiffPreviewCapabilities>,
}

/// File operation capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOperationCapabilities {
    /// Can read text files
    pub read_text_file: bool,
    /// Can write text files
    pub write_text_file: bool,
    /// Can create directories
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_directory: Option<bool>,
    /// Can delete files
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete_file: Option<bool>,
    /// Can list directory contents
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_directory: Option<bool>,
    /// Supports file watching
    #[serde(skip_serializing_if = "Option::is_none")]
    pub watch_files: Option<bool>,
}

/// Terminal capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalCapabilities {
    /// Can create terminals
    pub create_terminal: bool,
    /// Can read terminal output
    pub terminal_output: bool,
    /// Can release/destroy terminals
    pub release_terminal: bool,
    /// Can wait for terminal exit
    pub wait_for_terminal_exit: bool,
    /// Can kill terminal commands
    pub kill_terminal_command: bool,
}

/// Permission capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionCapabilities {
    /// Can request permissions interactively
    pub request_permission: bool,
    /// Supported permission types
    #[serde(default)]
    pub supported_permissions: Vec<String>,
}

/// Diff preview capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffPreviewCapabilities {
    /// Supports inline diff preview
    pub inline_preview: bool,
    /// Supports side-by-side diff
    #[serde(skip_serializing_if = "Option::is_none")]
    pub side_by_side: Option<bool>,
    /// Supports unified diff format
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unified_diff: Option<bool>,
}

/// Agent capabilities - what the agent supports
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    /// Supported content types
    #[serde(default)]
    pub content_types: Vec<ContentTypeCapability>,
    /// Streaming support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub streaming: Option<StreamingCapabilities>,
    /// Tool support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<ToolCapabilities>,
    /// Session support
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sessions: Option<SessionCapabilities>,
}

/// Content type capability
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentTypeCapability {
    Text,
    Image,
    Audio,
    Resource,
}

/// Streaming capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamingCapabilities {
    /// Supports streaming responses
    pub supports_streaming: bool,
    /// Supports cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_cancellation: Option<bool>,
}

/// Tool capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCapabilities {
    /// Supports tool calls
    pub supports_tools: bool,
    /// Available tools
    #[serde(default)]
    pub available_tools: Vec<ToolInfo>,
}

/// Information about an available tool
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    /// Tool name/identifier
    pub name: String,
    /// Human-readable description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Input schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_schema: Option<serde_json::Value>,
}

/// Session capabilities
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCapabilities {
    /// Supports multiple concurrent sessions
    pub supports_multiple_sessions: bool,
    /// Supports session persistence
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_persistence: Option<bool>,
    /// Maximum number of sessions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_sessions: Option<usize>,
}
