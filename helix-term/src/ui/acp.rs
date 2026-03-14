//! ACP (Agent Client Protocol) UI components
//!
//! Provides the user interface for interacting with AI coding agents.

use crate::compositor::{Component, Context, Event, EventResult};
use helix_acp::types as acp_types;
use helix_view::editor::{AgentRole, AgentSessionStatus};
use helix_view::graphics::Rect;
use helix_view::keyboard::KeyCode;
use helix_view::Editor;
use tui::{
    buffer::Buffer as Surface,
    widgets::{Block, Borders, Widget},
};

use std::borrow::Cow;

/// A popup component for displaying and interacting with an AI agent
pub struct AgentPanel {
    /// The agent session being displayed
    session_id: Option<acp_types::SessionId>,
    /// Input buffer for user messages
    input: String,
    /// Whether the panel is focused
    focused: bool,
    /// Scroll position
    scroll: usize,
}

impl Default for AgentPanel {
    fn default() -> Self {
        Self {
            session_id: None,
            input: String::new(),
            focused: true,
            scroll: 0,
        }
    }
}

impl AgentPanel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_session(session_id: acp_types::SessionId) -> Self {
        Self {
            session_id: Some(session_id),
            ..Default::default()
        }
    }

    /// Render the chat messages
    fn render_messages(&self, area: Rect, surface: &mut Surface, editor: &Editor) {
        let Some(session_id) = &self.session_id else {
            // No session active
            let text = "No active agent session. Use :agent-start to begin.";
            surface.set_string(area.x, area.y, text, editor.theme.get("ui.text"));
            return;
        };

        let Some(session) = editor.agent_sessions.get(session_id) else {
            let text = "Session not found.";
            surface.set_string(area.x, area.y, text, editor.theme.get("ui.text"));
            return;
        };

        // Render messages
        let mut y = area.y;
        let max_y = area.y + area.height.saturating_sub(1);

        // Skip messages based on scroll position
        for msg in session.messages.iter().skip(self.scroll) {
            if y >= max_y {
                break;
            }

            let role_style = match msg.role {
                AgentRole::User => editor.theme.get("ui.text"),
                AgentRole::Agent => editor.theme.get("ui.text.info"),
            };

            let prefix = match msg.role {
                AgentRole::User => "You: ",
                AgentRole::Agent => "Agent: ",
            };

            // Render each content block
            for content in &msg.content {
                if y >= max_y {
                    break;
                }

                match content {
                    acp_types::ContentBlock::Text(text_content) => {
                        let line = format!("{}{}", prefix, text_content.text);
                        surface.set_string(area.x, y, line, role_style);
                        y += 1;
                    }
                    acp_types::ContentBlock::Image(_) => {
                        surface.set_string(area.x, y, format!("{}[Image]", prefix), role_style);
                        y += 1;
                    }
                    acp_types::ContentBlock::Audio(_) => {
                        surface.set_string(area.x, y, format!("{}[Audio]", prefix), role_style);
                        y += 1;
                    }
                    acp_types::ContentBlock::ResourceLink(link) => {
                        let line = format!("{}[Resource: {}]", prefix, link.uri);
                        surface.set_string(area.x, y, line, role_style);
                        y += 1;
                    }
                    acp_types::ContentBlock::Resource(_) => {
                        surface.set_string(area.x, y, format!("{}[Embedded Resource]", prefix), role_style);
                        y += 1;
                    }
                }
            }
        }
    }

    /// Render the input area
    fn render_input(&self, area: Rect, surface: &mut Surface, editor: &Editor) {
        let style = if self.focused {
            editor.theme.get("ui.text")
        } else {
            editor.theme.get("ui.text.inactive")
        };

        let input_text = if self.input.is_empty() {
            Cow::Borrowed("Type your message...")
        } else {
            Cow::Owned(self.input.clone())
        };

        surface.set_string(area.x, area.y, input_text, style);
    }

    /// Render the status bar
    fn render_status(&self, area: Rect, surface: &mut Surface, editor: &Editor) {
        let status_text = if let Some(session_id) = &self.session_id {
            if let Some(session) = editor.agent_sessions.get(session_id) {
                match session.status {
                    AgentSessionStatus::Idle => "Status: Idle",
                    AgentSessionStatus::Processing => "Status: Processing...",
                    AgentSessionStatus::Cancelled => "Status: Cancelled",
                    AgentSessionStatus::Ended => "Status: Ended",
                }
            } else {
                "Status: Unknown"
            }
        } else {
            "No session"
        };

        let style = editor.theme.get("ui.statusline");
        surface.set_string(area.x, area.y, status_text, style);
    }
}

impl Component for AgentPanel {
    fn render(&mut self, area: Rect, surface: &mut Surface, cx: &mut Context) {
        // Create the main block with borders
        let block = Block::default()
            .title(" Agent ")
            .borders(Borders::ALL)
            .border_style(cx.editor.theme.get("ui.border"));

        let inner = block.inner(area);
        block.render(area, surface);

        // Split the inner area into messages, input, and status
        let messages_height = inner.height.saturating_sub(3);
        let messages_area = Rect::new(inner.x, inner.y, inner.width, messages_height);
        let input_area = Rect::new(
            inner.x,
            inner.y + messages_height,
            inner.width,
            1,
        );
        let status_area = Rect::new(
            inner.x,
            inner.y + messages_height + 1,
            inner.width,
            1,
        );

        // Render each section
        self.render_messages(messages_area, surface, cx.editor);
        self.render_input(input_area, surface, cx.editor);
        self.render_status(status_area, surface, cx.editor);
    }

    fn required_size(&mut self, viewport: (u16, u16)) -> Option<(u16, u16)> {
        // Request at least 40% of the viewport width and 60% of height
        let width = (viewport.0 as f32 * 0.4).max(40.0) as u16;
        let height = (viewport.1 as f32 * 0.6).max(15.0) as u16;
        Some((width, height))
    }

    fn handle_event(&mut self, event: &Event, _cx: &mut Context) -> EventResult {
        if !self.focused {
            return EventResult::Ignored(None);
        }

        match event {
            Event::Key(key) => {
                // Handle key events
                match key.code {
                    KeyCode::Char(c) => {
                        self.input.push(c);
                        EventResult::Consumed(None)
                    }
                    KeyCode::Backspace => {
                        self.input.pop();
                        EventResult::Consumed(None)
                    }
                    KeyCode::Enter => {
                        // Send message - this will be handled by a command
                        if !self.input.is_empty() {
                            let input = std::mem::take(&mut self.input);
                            EventResult::Consumed(Some(Box::new(move |_compositor, cx| {
                                // Trigger the agent-prompt command with the input
                                cx.editor.set_status(format!("Sending: {}", input));
                            })))
                        } else {
                            EventResult::Consumed(None)
                        }
                    }
                    KeyCode::Esc => {
                        EventResult::Ignored(None)
                    }
                    _ => EventResult::Ignored(None),
                }
            }
            _ => EventResult::Ignored(None),
        }
    }

    fn type_name(&self) -> &'static str {
        "AgentPanel"
    }
}

/// Picker item for selecting an agent
#[derive(Debug, Clone)]
pub struct AgentItem {
    pub name: String,
    pub command: String,
    pub enabled: bool,
}

/// Create a picker for selecting available agents
pub fn agent_picker(editor: &Editor) -> crate::ui::Picker<AgentItem, ()> {
    use crate::ui::{Picker, PickerColumn};

    // Get available agents from configuration
    let agents: Vec<AgentItem> = editor
        .agents
        .configurations()
        .iter()
        .map(|(name, config)| AgentItem {
            name: name.clone(),
            command: config.command.clone(),
            enabled: config.enabled,
        })
        .collect();

    let columns = [
        PickerColumn::new("name", |item: &AgentItem, _| item.name.as_str().into()),
        PickerColumn::new("command", |item: &AgentItem, _| item.command.as_str().into()),
        PickerColumn::new("enabled", |item: &AgentItem, _| {
            if item.enabled { "yes" } else { "no" }.into()
        }),
    ];

    Picker::new(columns, 0, agents, (), |cx, item: &AgentItem, _action| {
        // Start the selected agent
        cx.editor.set_status(format!("Starting agent: {}", item.name));
        // The actual start will be handled by a command
    })
}
