//! ACP protocol type definitions.
//!
//! Covers initialization, session management, the prompt turn lifecycle,
//! content blocks, tool calls, and file-system methods.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Common
// ---------------------------------------------------------------------------

/// An opaque session identifier returned by `session/new` or `session/load`.
pub type SessionId = String;

/// A unique identifier for a tool call within a session.
pub type ToolCallId = String;

// ---------------------------------------------------------------------------
// Initialization
// ---------------------------------------------------------------------------

/// Protocol version integer (currently 1).
pub const PROTOCOL_VERSION: u32 = 1;

/// `initialize` request params (Client → Agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_capabilities: ClientCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_info: Option<ImplementationInfo>,
}

/// Capabilities advertised by the client.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fs: Option<FsCapabilities>,
    /// All `terminal/*` methods are available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal: Option<bool>,
}

/// File-system capabilities the client exposes to the agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCapabilities {
    /// `fs/read_text_file` is available.
    #[serde(default)]
    pub read_text_file: bool,
    /// `fs/write_text_file` is available.
    #[serde(default)]
    pub write_text_file: bool,
}

/// `initialize` response (Agent → Client).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub agent_capabilities: AgentCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_info: Option<ImplementationInfo>,
    #[serde(default)]
    pub auth_methods: Vec<Value>,
}

/// Capabilities advertised by the agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    /// Agent supports `session/load`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_session: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt_capabilities: Option<PromptCapabilities>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mcp_capabilities: Option<McpCapabilities>,
}

/// Content types supported in `session/prompt`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptCapabilities {
    #[serde(default)]
    pub image: bool,
    #[serde(default)]
    pub audio: bool,
    #[serde(rename = "embeddedContext", default)]
    pub embedded_context: bool,
}

/// MCP server transport capabilities of the agent.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct McpCapabilities {
    #[serde(default)]
    pub http: bool,
    #[serde(default)]
    pub sse: bool,
}

/// Name / version of an implementation (client or agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImplementationInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub version: String,
}

// ---------------------------------------------------------------------------
// Session setup
// ---------------------------------------------------------------------------

/// `session/new` request params.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewParams {
    /// Absolute path to the working directory.
    pub cwd: PathBuf,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// `session/new` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewResult {
    pub session_id: SessionId,
}

/// `session/load` request params.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionLoadParams {
    pub session_id: SessionId,
    pub cwd: PathBuf,
    #[serde(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Configuration for an MCP server connection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum McpServerConfig {
    #[serde(rename = "stdio")]
    Stdio(McpStdioConfig),
    #[serde(rename = "http")]
    Http(McpHttpConfig),
}

/// An MCP server accessed via stdio subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpStdioConfig {
    pub name: String,
    pub command: PathBuf,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvVariable>,
}

/// An MCP server accessed over HTTP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpHttpConfig {
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVariable {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

// ---------------------------------------------------------------------------
// Content blocks
// ---------------------------------------------------------------------------

/// A content block that can appear in prompts and agent messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    Image {
        /// Base64-encoded image bytes.
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// Embedded resource (requires `embeddedContext` agent capability).
    Resource {
        resource: EmbeddedResource,
    },
    /// A link to a resource (always supported).
    #[serde(rename = "resource_link")]
    ResourceLink {
        resource: ResourceLink,
    },
}

/// An embedded resource (file / document content inline).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddedResource {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Text content, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Base64-encoded binary content, if not text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob: Option<String>,
}

/// A reference to a resource by URI.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLink {
    pub uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

// ---------------------------------------------------------------------------
// Prompt turn
// ---------------------------------------------------------------------------

/// `session/prompt` request params (Client → Agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptParams {
    pub session_id: SessionId,
    pub prompt: Vec<ContentBlock>,
}

/// `session/prompt` result — the reason the agent stopped this turn.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionPromptResult {
    pub stop_reason: StopReason,
}

/// Why an agent ended a prompt turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
    Cancelled,
}

/// `session/cancel` notification params (Client → Agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionCancelParams {
    pub session_id: SessionId,
}

// ---------------------------------------------------------------------------
// session/update notification (Agent → Client)
// ---------------------------------------------------------------------------

/// Outer wrapper for `session/update` notification params.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdateParams {
    pub session_id: SessionId,
    pub update: SessionUpdate,
}

/// The discriminated payload inside a `session/update` notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
pub enum SessionUpdate {
    /// A chunk of text (or other content) from the agent's response.
    AgentMessageChunk {
        content: ContentBlock,
    },
    /// A chunk replayed from a loaded session that was originally a user message.
    UserMessageChunk {
        content: ContentBlock,
    },
    /// The agent's plan for the current turn.
    Plan {
        entries: Vec<PlanEntry>,
    },
    /// The agent is starting a new tool call.
    ToolCall {
        #[serde(rename = "toolCallId")]
        tool_call_id: ToolCallId,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        kind: Option<ToolKind>,
        #[serde(default)]
        status: ToolCallStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_input: Option<Value>,
    },
    /// An update to an existing tool call.
    ToolCallUpdate {
        #[serde(rename = "toolCallId")]
        tool_call_id: ToolCallId,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<ToolCallStatus>,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        content: Vec<ToolCallContent>,
        #[serde(skip_serializing_if = "Option::is_none")]
        raw_output: Option<Value>,
    },
}

/// A single entry in an agent plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// ---------------------------------------------------------------------------
// Tool calls
// ---------------------------------------------------------------------------

/// Broad category of a tool call, used for icon selection in UIs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Read,
    Write,
    Execute,
    Network,
    Other,
}

/// Lifecycle status of a tool call.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Failed,
    Cancelled,
}

/// Content produced by a tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolCallContent {
    /// Regular text / image / resource output.
    Content { content: ContentBlock },
    /// A file diff shown to the user.
    Diff {
        path: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        old_text: Option<String>,
        new_text: String,
    },
    /// Live terminal output; the terminal's ID links to `terminal/create`.
    Terminal {
        #[serde(rename = "terminalId")]
        terminal_id: String,
    },
}

/// File location reported by a tool call (for "follow-along" UI).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallLocation {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
}

// ---------------------------------------------------------------------------
// session/request_permission (Agent → Client, bidirectional request)
// ---------------------------------------------------------------------------

/// `session/request_permission` request params (Agent → Client).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionParams {
    pub session_id: SessionId,
    /// The pending tool call this permission applies to.
    pub tool_call: ToolCallRef,
    /// Options the user may choose from.
    pub options: Vec<PermissionOption>,
}

/// Minimal reference to the tool call being approved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRef {
    pub tool_call_id: ToolCallId,
}

/// One choice presented to the user in a permission dialog.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    pub kind: PermissionOptionKind,
}

/// The semantic meaning of a permission option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionOptionKind {
    AllowOnce,
    AllowAlways,
    RejectOnce,
    RejectAlways,
}

/// `session/request_permission` response (Client → Agent).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionResult {
    pub outcome: PermissionOutcome,
}

/// The user's decision on a permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum PermissionOutcome {
    Cancelled,
    Selected { option_id: String },
}

// ---------------------------------------------------------------------------
// fs/read_text_file  (Agent → Client, bidirectional request)
// ---------------------------------------------------------------------------

/// `fs/read_text_file` request params (Agent → Client).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsReadTextFileParams {
    pub session_id: SessionId,
    pub path: PathBuf,
    /// 1-based starting line (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Maximum number of lines to return (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

/// `fs/read_text_file` result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FsReadTextFileResult {
    pub content: String,
}

// ---------------------------------------------------------------------------
// fs/write_text_file (Agent → Client, bidirectional request)
// ---------------------------------------------------------------------------

/// `fs/write_text_file` request params (Agent → Client).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteTextFileParams {
    pub session_id: SessionId,
    pub path: PathBuf,
    pub content: String,
}
// `fs/write_text_file` result is `null` on success — use `serde_json::Value::Null`.
