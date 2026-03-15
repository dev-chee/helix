//! Agent Client Protocol (ACP) type definitions.
//!
//! Schema reference: <https://agentclientprotocol.com/protocol/schema>

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod request;
pub mod notification;

pub use request::Request;
pub use notification::Notification;

/// Protocol version (integer). ACP uses a single integer for major version.
pub type ProtocolVersion = u32;

/// Unique identifier for a session. Used in session/new, session/prompt, etc.
pub type SessionId = String;

// ---------------------------------------------------------------------------
// Initialize
// ---------------------------------------------------------------------------

/// Client capabilities advertised during initialize.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    #[serde(default)]
    pub fs: FsCapabilities,
    #[serde(default)]
    pub terminal: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCapabilities {
    #[serde(default)]
    pub read_text_file: bool,
    #[serde(default)]
    pub write_text_file: bool,
}

/// Implementation info (name, version) for client or agent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

/// Request params for `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeRequest {
    pub protocol_version: ProtocolVersion,
    #[serde(default)]
    pub client_capabilities: ClientCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_info: Option<Implementation>,
}

/// Agent capabilities returned from initialize.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(default)]
    pub load_session: bool,
    #[serde(default)]
    pub prompt_capabilities: PromptCapabilities,
    #[serde(default)]
    pub mcp_capabilities: McpCapabilities,
    #[serde(default)]
    pub session_capabilities: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptCapabilities {
    #[serde(default)]
    pub image: bool,
    #[serde(default)]
    pub audio: bool,
    #[serde(default)]
    pub embedded_context: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpCapabilities {
    #[serde(default)]
    pub http: bool,
    #[serde(default)]
    pub sse: bool,
}

/// Authentication method identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthMethod {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

/// Response for `initialize`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResponse {
    pub protocol_version: ProtocolVersion,
    #[serde(default)]
    pub agent_capabilities: AgentCapabilities,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_info: Option<Implementation>,
    #[serde(default)]
    pub auth_methods: Vec<AuthMethod>,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// MCP server entry for session/new or session/load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
}

/// Request params for `session/new`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionRequest {
    pub working_directory: String,
    pub mcp_servers: Vec<McpServer>,
}

/// Response for `session/new`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionResponse {
    pub session_id: SessionId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_config_options: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_mode_state: Option<serde_json::Value>,
}

/// Content block in a prompt (text, resource link, etc.). Schema: ContentBlock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ResourceLink { uri: String },
    #[serde(other)]
    Other,
}

/// Request params for `session/prompt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptRequest {
    pub session_id: SessionId,
    pub content: Vec<ContentBlock>,
}

/// Stop reason for prompt turn.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Completed,
    Cancelled,
    Error,
    #[serde(other)]
    Other,
}

/// Response for `session/prompt`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptResponse {
    pub stop_reason: StopReason,
}

/// Params for `session/cancel` notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelNotification {
    pub session_id: SessionId,
}

// ---------------------------------------------------------------------------
// Client methods (Agent calls Client)
// ---------------------------------------------------------------------------

/// Request for `fs/read_text_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTextFileRequest {
    pub session_id: SessionId,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_lines: Option<u32>,
}

/// Response for `fs/read_text_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadTextFileResponse {
    pub content: String,
}

/// Request for `fs/write_text_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteTextFileRequest {
    pub session_id: SessionId,
    pub path: String,
    pub content: String,
}

/// Response for `fs/write_text_file`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteTextFileResponse {}

/// Session update payload (message chunks, tool calls, etc.). Schema: SessionUpdate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNotification {
    pub session_id: SessionId,
    pub update: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use notification::{Cancel, SessionUpdate};
    use request::{Initialize, NewSession, Prompt};

    #[test]
    fn request_method_constants() {
        assert_eq!(Initialize::METHOD, "initialize");
        assert_eq!(NewSession::METHOD, "session/new");
        assert_eq!(Prompt::METHOD, "session/prompt");
    }

    #[test]
    fn notification_method_constants() {
        assert_eq!(Cancel::METHOD, "session/cancel");
        assert_eq!(SessionUpdate::METHOD, "session/update");
    }

    #[test]
    fn initialize_request_roundtrip() {
        let req = InitializeRequest {
            protocol_version: 1,
            client_capabilities: ClientCapabilities::default(),
            client_info: Some(Implementation {
                name: "helix".into(),
                title: Some("Helix".into()),
                version: Some("1.0".into()),
            }),
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: InitializeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.protocol_version, 1);
        assert_eq!(back.client_info.as_ref().unwrap().name, "helix");
    }

    #[test]
    fn initialize_response_roundtrip() {
        let resp = InitializeResponse {
            protocol_version: 1,
            agent_capabilities: AgentCapabilities::default(),
            agent_info: None,
            auth_methods: vec![],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: InitializeResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back.protocol_version, 1);
        assert!(back.auth_methods.is_empty());
    }

    #[test]
    fn content_block_text_roundtrip() {
        let block = ContentBlock::Text {
            text: "hello".to_string(),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("text") && json.contains("hello"));
        let back: ContentBlock = serde_json::from_str(&json).unwrap();
        match &back {
            ContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected Text variant"),
        }
    }

    #[test]
    fn prompt_request_roundtrip() {
        let req = PromptRequest {
            session_id: "sess-1".to_string(),
            content: vec![ContentBlock::Text {
                text: "hi".to_string(),
            }],
        };
        let json = serde_json::to_string(&req).unwrap();
        let back: PromptRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(back.session_id, "sess-1");
        assert_eq!(back.content.len(), 1);
    }
}
