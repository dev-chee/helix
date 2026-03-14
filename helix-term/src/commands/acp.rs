//! ACP (Agent Client Protocol) editor commands.
//!
//! Bound under `space + a` in normal mode.

use helix_acp::{AgentClientConfig, AgentClientId};
use helix_view::{editor::AcpServerConfig, Editor};

use super::Context;
use crate::{
    compositor,
    job::{Callback, Jobs},
    ui::{self, overlay::overlaid, PickerColumn, PromptEvent},
};

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn first_ready_client(editor: &Editor) -> Option<AgentClientId> {
    editor
        .agent_clients
        .iter()
        .find(|(_, c)| c.is_initialized())
        .or_else(|| editor.agent_clients.iter().next())
        .map(|(id, _)| id)
}

// ---------------------------------------------------------------------------
// agent_connect
// ---------------------------------------------------------------------------

/// Connect to a configured agent server, spawning it if not already running.
pub fn agent_connect(cx: &mut Context) {
    let configs: Vec<AcpServerConfig> = cx.editor.config().agent.servers.clone();

    if configs.is_empty() {
        cx.editor.set_error(
            "No agent servers configured. Add [[agent.servers]] entries to config.toml.",
        );
        return;
    }

    if configs.len() == 1 {
        launch_agent(cx.editor, cx.jobs, configs.into_iter().next().unwrap());
        return;
    }

    // Multiple servers — let the user pick.
    let columns = vec![
        PickerColumn::new("name", |c: &AcpServerConfig, _| c.name.as_str().into()),
        PickerColumn::new("command", |c: &AcpServerConfig, _| {
            c.command.to_string_lossy().into_owned().into()
        }),
    ];
    let picker = ui::Picker::new(
        columns,
        0,
        configs,
        (),
        |cx: &mut compositor::Context, config: &AcpServerConfig, _action| {
            let config = config.clone();
            launch_agent(cx.editor, cx.jobs, config);
        },
    );
    cx.push_layer(Box::new(overlaid(picker)));
}

fn launch_agent(editor: &mut Editor, jobs: &mut Jobs, server: AcpServerConfig) {
    let config = AgentClientConfig {
        name: server.name.clone(),
        command: server.command,
        args: server.args,
        env: Vec::new(),
        timeout_secs: server.timeout,
    };

    let client_id = match editor.agent_clients.start_client(config) {
        Ok(id) => id,
        Err(e) => {
            editor.set_error(format!("Failed to start agent '{}': {e}", server.name));
            return;
        }
    };

    let init_handle = match editor.agent_clients.get(client_id) {
        Some(c) => c.handle(),
        None => {
            editor.set_error("Agent disappeared immediately after start.");
            return;
        }
    };

    editor.set_status(format!(
        "Starting agent '{}' (initializing…)",
        server.name
    ));

    jobs.spawn(async move {
        match init_handle.initialize().await {
            Ok(r) => {
                log::info!(
                    "ACP agent initialized: {:?}",
                    r.agent_info.as_ref().map(|i| &i.name)
                );
            }
            Err(e) => {
                log::error!("ACP initialize failed: {e}");
            }
        }
        Ok(())
    });
}

// ---------------------------------------------------------------------------
// agent_send_selection
// ---------------------------------------------------------------------------

/// Send the current selection (or the full file) to the active agent.
pub fn agent_send_selection(cx: &mut Context) {
    let client_id = match first_ready_client(cx.editor) {
        Some(id) => id,
        None => {
            cx.editor
                .set_error("No active agent. Run 'agent_connect' first.");
            return;
        }
    };

    let (view, doc) = helix_view::current!(cx.editor);
    let sel = doc.selection(view.id);
    let text = doc.text();
    let primary = sel.primary();
    let selected_text = if primary.is_empty() {
        text.to_string()
    } else {
        text.slice(primary.from()..primary.to()).to_string()
    };

    let uri = doc
        .path()
        .map(|p| format!("file://{}", p.display()))
        .unwrap_or_else(|| "untitled".to_string());

    let cwd = helix_stdx::path::canonicalize(
        doc.path()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new(".")),
    );

    let prompt_content = vec![
        helix_acp::types::ContentBlock::Text {
            text: selected_text,
        },
        helix_acp::types::ContentBlock::ResourceLink {
            resource: helix_acp::types::ResourceLink {
                uri,
                mime_type: doc.language_name().map(|l| format!("text/{l}")),
            },
        },
    ];

    start_agent_session_and_prompt(cx.editor, cx.jobs, client_id, cwd, prompt_content);
}

// ---------------------------------------------------------------------------
// agent_prompt
// ---------------------------------------------------------------------------

/// Open a prompt so the user can type a question/instruction for the agent.
pub fn agent_prompt(cx: &mut Context) {
    let client_id = match first_ready_client(cx.editor) {
        Some(id) => id,
        None => {
            cx.editor
                .set_error("No active agent. Run 'agent_connect' first.");
            return;
        }
    };

    let (_, doc) = helix_view::current!(cx.editor);
    let cwd = helix_stdx::path::canonicalize(
        doc.path()
            .and_then(|p| p.parent())
            .unwrap_or_else(|| std::path::Path::new(".")),
    );

    cx.push_layer(Box::new(ui::Prompt::new(
        "agent: ".into(),
        None,
        ui::completers::none,
        move |cx: &mut compositor::Context, input: &str, event: PromptEvent| {
            if event != PromptEvent::Validate || input.is_empty() {
                return;
            }
            let content = vec![helix_acp::types::ContentBlock::Text {
                text: input.to_string(),
            }];
            start_agent_session_and_prompt(cx.editor, cx.jobs, client_id, cwd.clone(), content);
        },
    )));
}

// ---------------------------------------------------------------------------
// agent_output
// ---------------------------------------------------------------------------

/// Show the latest agent output in a popup.
pub fn agent_output(cx: &mut Context) {
    let text = cx
        .editor
        .acp_sessions
        .values()
        .last()
        .map(|s| s.accumulated_text.clone())
        .unwrap_or_default();

    if text.is_empty() {
        cx.editor.set_status("No agent output yet.");
        return;
    }

    let contents = ui::Text::new(text);
    let popup = ui::Popup::new("agent-output", contents).auto_close(true);
    cx.push_layer(Box::new(popup));
}

// ---------------------------------------------------------------------------
// agent_cancel
// ---------------------------------------------------------------------------

/// Cancel the current agent turn.
pub fn agent_cancel(cx: &mut Context) {
    let running: Vec<(helix_acp::types::SessionId, AgentClientId)> = cx
        .editor
        .acp_sessions
        .values()
        .filter(|s| s.running)
        .filter_map(|s| s.client_id.map(|c| (s.session_id.clone(), c)))
        .collect();

    if running.is_empty() {
        cx.editor.set_status("No active agent turn to cancel.");
        return;
    }

    for (sid, cid) in running {
        if let Some(client) = cx.editor.agent_clients.get(cid) {
            client.cancel(&sid);
        }
        if let Some(state) = cx.editor.acp_sessions.get_mut(&sid) {
            state.running = false;
        }
    }
    cx.editor.set_status("Agent turn cancelled.");
}

// ---------------------------------------------------------------------------
// agent_sessions
// ---------------------------------------------------------------------------

/// List all active agent sessions.
pub fn agent_sessions(cx: &mut Context) {
    let summaries: Vec<String> = cx
        .editor
        .acp_sessions
        .iter()
        .map(|(sid, state)| {
            let name = state
                .client_id
                .and_then(|id| cx.editor.agent_clients.get(id))
                .map(|c| c.name.as_str())
                .unwrap_or("unknown");
            let short_id = &sid[..sid.len().min(12)];
            format!(
                "[{}] {} — {}",
                if state.running { "●" } else { "○" },
                name,
                short_id
            )
        })
        .collect();

    if summaries.is_empty() {
        cx.editor.set_status("No active agent sessions.");
    } else {
        cx.editor
            .set_status(format!("{} session(s): {}", summaries.len(), summaries.join(", ")));
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn start_agent_session_and_prompt(
    editor: &mut Editor,
    jobs: &mut Jobs,
    client_id: AgentClientId,
    cwd: PathBuf,
    content: Vec<helix_acp::types::ContentBlock>,
) {
    let handle = match editor.agent_clients.get(client_id) {
        Some(c) => c.handle(),
        None => {
            editor.set_error("Agent client disconnected.");
            return;
        }
    };

    editor.set_status("Agent: sending prompt…");

    jobs.callback(async move {
        let sid = handle
            .new_session(cwd, vec![])
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let stop = handle
            .prompt(sid.clone(), content)
            .await
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(Callback::Editor(Box::new(move |editor: &mut Editor| {
            if let Some(state) = editor.acp_sessions.get_mut(&sid) {
                state.running = false;
            }
            editor.set_status(format!(
                "Agent session complete ({:?})",
                stop
            ));
        })))
    });
}
