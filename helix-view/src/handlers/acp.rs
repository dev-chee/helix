//! Handlers for ACP (Agent Client Protocol) events received from agent sub-processes.
//!
//! Two categories of messages arrive here:
//!
//! 1. **Notifications** (`session/update`): the agent streams text chunks, tool-call
//!    progress, and plan entries. We accumulate these in `AgentSessionState` and
//!    request a redraw.
//!
//! 2. **Method calls** (agent → client requests): `session/request_permission`,
//!    `fs/read_text_file`, and `fs/write_text_file`.
//!    File-system requests that don't require user interaction are resolved here;
//!    permission dialogs are forwarded to helix-term's event loop via `AgentUiEvent`.

use crate::Editor;
use helix_acp::{AgentClientId, MethodCall, Notification};
use helix_acp::types::{ContentBlock, SessionId, SessionUpdate, ToolCallId, ToolCallStatus};
use helix_event::request_redraw;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Per-session runtime state, stored in `Editor::acp_sessions`
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct AgentSessionState {
    pub client_id: Option<AgentClientId>,
    pub session_id: SessionId,
    /// Accumulated agent text reply (rendered in the output panel).
    pub accumulated_text: String,
    /// Tool calls reported by the agent this turn.
    pub tool_calls: std::collections::BTreeMap<ToolCallId, AgentToolCall>,
    /// True while `session/prompt` is in flight.
    pub running: bool,
}

#[derive(Debug)]
pub struct AgentToolCall {
    pub title: String,
    pub status: ToolCallStatus,
    /// Diffs produced by this tool call (old_text, new_text, path).
    pub diffs: Vec<(PathBuf, Option<String>, String)>,
    pub text_output: String,
}

// ---------------------------------------------------------------------------
// Events dispatched back to helix-term (require Compositor access)
// ---------------------------------------------------------------------------

/// Events that need the Compositor (UI) to handle and can't be resolved inside
/// helix-view alone.  Sent via `Editor::acp_ui_events` channel.
#[derive(Debug)]
pub enum AgentUiEvent {
    /// Agent requested permission for a tool call.
    RequestPermission {
        client_id: AgentClientId,
        request_id: helix_acp::types::jsonrpc::Id,
        session_id: SessionId,
        tool_call_id: ToolCallId,
        options: Vec<helix_acp::types::PermissionOption>,
    },
    /// Agent wants to write a file — needs user confirmation when auto-approve
    /// is disabled.
    WriteFile {
        client_id: AgentClientId,
        request_id: helix_acp::types::jsonrpc::Id,
        session_id: SessionId,
        path: PathBuf,
        content: String,
    },
}

// ---------------------------------------------------------------------------
// Notification handler
// ---------------------------------------------------------------------------

/// Process a `session/update` notification and update local state.
pub fn handle_notification(
    editor: &mut Editor,
    client_id: AgentClientId,
    notif: Notification,
) {
    match notif {
        Notification::SessionUpdate(params) => {
            let session_id = params.session_id.clone();
            let state = editor
                .acp_sessions
                .entry(session_id.clone())
                .or_insert_with(|| AgentSessionState {
                    client_id: Some(client_id),
                    session_id: session_id.clone(),
                    ..Default::default()
                });

            match params.update {
                SessionUpdate::AgentMessageChunk { content } => {
                    if let ContentBlock::Text { text } = content {
                        state.accumulated_text.push_str(&text);
                        request_redraw();
                    }
                }
                SessionUpdate::Plan { .. } => {
                    // Plans are informational; a future UI panel can display them.
                    request_redraw();
                }
                SessionUpdate::ToolCall {
                    tool_call_id,
                    title,
                    status,
                    ..
                } => {
                    state.tool_calls.insert(
                        tool_call_id,
                        AgentToolCall {
                            title,
                            status,
                            diffs: Vec::new(),
                            text_output: String::new(),
                        },
                    );
                    request_redraw();
                }
                SessionUpdate::ToolCallUpdate {
                    tool_call_id,
                    status,
                    content,
                    ..
                } => {
                    if let Some(tc) = state.tool_calls.get_mut(&tool_call_id) {
                        if let Some(s) = status {
                            tc.status = s;
                        }
                        for item in content {
                            use helix_acp::types::ToolCallContent;
                            match item {
                                ToolCallContent::Content { content } => {
                                    if let ContentBlock::Text { text } = content {
                                        tc.text_output.push_str(&text);
                                    }
                                }
                                ToolCallContent::Diff {
                                    path,
                                    old_text,
                                    new_text,
                                } => {
                                    tc.diffs.push((PathBuf::from(path), old_text, new_text));
                                }
                                ToolCallContent::Terminal { .. } => {
                                    // Terminal embedding: future work.
                                }
                            }
                        }
                    }
                    request_redraw();
                }
                SessionUpdate::UserMessageChunk { .. } => {
                    // Replayed history — ignored for now.
                }
            }
        }
        Notification::Exit => {
            // Agent process exited. Clean up sessions associated with this client.
            editor
                .acp_sessions
                .retain(|_, s| s.client_id != Some(client_id));
            editor.agent_clients.remove(client_id);
            request_redraw();
        }
    }
}

// ---------------------------------------------------------------------------
// Method-call handler (agent → client requests)
// ---------------------------------------------------------------------------

/// Handle an agent-originated request, either resolving it immediately or
/// returning an `AgentUiEvent` that helix-term must handle.
pub fn handle_method_call(
    editor: &mut Editor,
    client_id: AgentClientId,
    call: MethodCall,
) -> Option<AgentUiEvent> {
    let _auto_approve_reads = editor.config().agent.auto_approve_reads;

    match call {
        MethodCall::ReadTextFile { id, params } => {
            // Resolve from editor state (includes unsaved buffer content).
            let content = read_file_content(editor, &params.path, params.line, params.limit);
            if let Some(client) = editor.agent_clients.get(client_id) {
                match content {
                    Ok(text) => client.reply_read_file(id, text),
                    Err(e) => client.reply_read_file_error(id, e),
                }
            }
            None
        }

        MethodCall::WriteTextFile { id, params } => {
            let auto_approve = editor.config().agent.auto_approve_reads; // reuse flag for now
            if auto_approve {
                apply_write(editor, client_id, id, params.path.clone(), params.content.clone());
                None
            } else {
                // Need user confirmation — surface to helix-term.
                Some(AgentUiEvent::WriteFile {
                    client_id,
                    request_id: id,
                    session_id: params.session_id,
                    path: params.path,
                    content: params.content,
                })
            }
        }

        MethodCall::RequestPermission { id, params } => {
            Some(AgentUiEvent::RequestPermission {
                client_id,
                request_id: id,
                session_id: params.session_id,
                tool_call_id: params.tool_call.tool_call_id,
                options: params.options,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read text content for a path, preferring the editor's in-memory buffer.
fn read_file_content(
    editor: &Editor,
    path: &std::path::Path,
    line: Option<u32>,
    limit: Option<u32>,
) -> Result<String, String> {
    // Try to find an open buffer first (includes unsaved changes).
    for doc in editor.documents.values() {
        if doc.path().map(|p| p == path).unwrap_or(false) {
            let text = doc.text();
            let start_line = line.map(|l| l.saturating_sub(1) as usize).unwrap_or(0);
            let end_line = limit
                .map(|lim| (start_line + lim as usize).min(text.len_lines()))
                .unwrap_or_else(|| text.len_lines());

            let content = text
                .lines_at(start_line)
                .take(end_line - start_line)
                .map(|l| l.to_string())
                .collect::<String>();
            return Ok(content);
        }
    }

    // Fall back to disk.
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

/// Apply a file write via a Transaction (preserves undo history) or via disk.
pub fn apply_write(
    editor: &mut Editor,
    client_id: AgentClientId,
    request_id: helix_acp::types::jsonrpc::Id,
    path: PathBuf,
    new_content: String,
) {
    // Find the document, if open.
    let doc_id = editor
        .documents
        .values()
        .find(|d| d.path().map(|p| p.as_path() == path.as_path()).unwrap_or(false))
        .map(|d| d.id());

    if let Some(doc_id) = doc_id {
        // Apply via Transaction so the change enters the undo history.
        let doc = editor.documents.get_mut(&doc_id).unwrap();
        let new_rope = helix_core::Rope::from(new_content.as_str());
        let transaction = helix_core::diff::compare_ropes(doc.text(), &new_rope);
        // Apply to all views that show this document.
        let view_ids: Vec<_> = editor
            .tree
            .views()
            .filter(|(v, _focused)| v.doc == doc_id)
            .map(|(v, _)| v.id)
            .collect();
        for view_id in view_ids {
            doc.apply(&transaction, view_id);
        }
        request_redraw();
    } else {
        // Document not open — write to disk.
        if let Err(e) = std::fs::write(&path, &new_content) {
            log::error!("ACP fs/write_text_file disk write failed: {e}");
            if let Some(client) = editor.agent_clients.get(client_id) {
                client.reply_write_file_error(request_id, e.to_string());
            }
            return;
        }
    }

    if let Some(client) = editor.agent_clients.get(client_id) {
        client.reply_write_file_ok(request_id);
    }
}
