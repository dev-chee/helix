//! Session-related types for ACP

use super::*;
use serde::{Deserialize, Serialize};

/// Unique identifier for a session
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// MCP Server configuration
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    /// Name of the MCP server
    pub name: String,
    /// Configuration for the MCP server
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

/// Request to initialize the ACP connection
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    /// Protocol version
    pub protocol_version: String,
    /// Client capabilities
    pub client_capabilities: super::ClientCapabilities,
    /// Information about the client implementation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_info: Option<Implementation>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Response to initialize request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    /// Protocol version
    pub protocol_version: String,
    /// Agent capabilities
    pub agent_capabilities: super::AgentCapabilities,
    /// Information about the agent implementation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_info: Option<Implementation>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Request to create a new session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionRequest {
    /// MCP servers to use for this session
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
    /// Current working directory for the session
    pub cwd: PathBuf,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Response to new session request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionResponse {
    /// ID of the created session
    pub session_id: SessionId,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Request to send a prompt to the agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptRequest {
    /// Session ID to send the prompt to
    pub session_id: SessionId,
    /// Prompt content blocks
    pub prompt: Vec<super::ContentBlock>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Response to prompt request (empty for streaming)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptResponse {}

/// Request to cancel a session
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelSessionRequest {
    /// Session ID to cancel
    pub session_id: SessionId,
    /// Optional reason for cancellation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<Meta>,
}

/// Response to cancel session request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelSessionResponse {}

/// Notification sent from agent to client about session updates
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNotification {
    /// Session ID this update is for
    pub session_id: SessionId,
    /// The update content
    pub update: SessionUpdate,
}

/// Types of session updates
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SessionUpdate {
    /// Agent is sending a message chunk
    AgentMessageChunk(ContentChunk),
    /// Agent has finished a turn
    AgentTurnEnd(AgentTurnEnd),
    /// Task status update
    TaskStatusUpdate(TaskStatusUpdate),
    /// Agent is requesting permission
    PermissionRequest(PermissionRequest),
    /// Agent wants to show a diff
    ShowDiff(ShowDiff),
    /// Session has ended
    SessionEnded(SessionEnded),
    /// Error occurred
    Error(ErrorNotification),
}

/// A chunk of content from the agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentChunk {
    /// The content block
    pub content: super::ContentBlock,
    /// Optional position in the message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
}

/// Signals that the agent has finished a turn
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentTurnEnd {
    /// Whether the agent is waiting for user input
    pub waiting_for_input: bool,
    /// Optional summary of what was done
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Task status update
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskStatusUpdate {
    /// Status message
    pub status: String,
    /// Whether the task is in progress
    pub in_progress: bool,
    /// Optional progress percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u8>,
}

/// Permission request from agent
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    /// Unique ID for this permission request
    pub id: String,
    /// What permission is being requested
    pub permission: String,
    /// Human-readable description
    pub description: String,
    /// Optional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

/// Show diff notification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShowDiff {
    /// File path
    pub path: PathBuf,
    /// The diff content
    pub diff: String,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Session ended notification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionEnded {
    /// Reason for ending
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Error notification
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorNotification {
    /// Error message
    pub message: String,
    /// Optional error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<i64>,
}
