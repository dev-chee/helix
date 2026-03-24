use super::*;

use helix_view::current_ref;

// Use the platform-specific event types matching what the test harness expects.
// crossterm and termina have the same MouseEvent structure but different modifier type names.
#[cfg(windows)]
use crossterm::event::{Event, KeyModifiers as Modifiers, MouseEvent, MouseEventKind};
#[cfg(not(windows))]
use termina::event::{Event, Modifiers, MouseEvent, MouseEventKind};

/// Helper to create a mouse scroll event at a given position.
/// The row/column coordinates should fall within the editor's inner area
/// so that `pos_and_view()` can resolve the target view.
fn mouse_scroll_event(kind: MouseEventKind, row: u16, column: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        row,
        column,
        modifiers: Modifiers::NONE,
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mouse_scroll_right() -> anyhow::Result<()> {
    // Create a document with a very long line so horizontal scrolling is meaningful.
    let long_line = "x".repeat(500);
    let input = format!("#[|{}]#\n", long_line);

    let mut app = helpers::AppBuilder::new()
        .with_input_text(&input)
        .build()?;

    // Send multiple ScrollRight events to accumulate horizontal offset.
    // The mouse position (row=1, column=1) must land within the editor view area.
    let events = vec![
        mouse_scroll_event(MouseEventKind::ScrollRight, 1, 10),
        mouse_scroll_event(MouseEventKind::ScrollRight, 1, 10),
    ];

    helpers::test_event_sequence(
        &mut app,
        events,
        Some(&|app| {
            let (view, doc) = current_ref!(app.editor);
            let offset = doc.view_offset(view.id);
            // Default scroll_lines is 3, so 2 scroll events should give offset 6.
            assert!(
                offset.horizontal_offset > 0,
                "expected horizontal_offset > 0 after ScrollRight, got {}",
                offset.horizontal_offset
            );
        }),
        false,
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mouse_scroll_left() -> anyhow::Result<()> {
    let long_line = "x".repeat(500);
    let input = format!("#[|{}]#\n", long_line);

    let mut app = helpers::AppBuilder::new()
        .with_input_text(&input)
        .build()?;

    // First scroll right, then scroll left, and verify offset decreased.
    let events = vec![
        mouse_scroll_event(MouseEventKind::ScrollRight, 1, 10),
        mouse_scroll_event(MouseEventKind::ScrollRight, 1, 10),
        mouse_scroll_event(MouseEventKind::ScrollRight, 1, 10),
        mouse_scroll_event(MouseEventKind::ScrollLeft, 1, 10),
    ];

    helpers::test_event_sequence(
        &mut app,
        events,
        Some(&|app| {
            let (view, doc) = current_ref!(app.editor);
            let offset = doc.view_offset(view.id);
            // 3 right scrolls (3*3=9) minus 1 left scroll (3) = 6
            assert_eq!(
                offset.horizontal_offset, 6,
                "expected horizontal_offset == 6 after 3 right + 1 left scroll"
            );
        }),
        false,
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mouse_scroll_left_saturates_at_zero() -> anyhow::Result<()> {
    // Start with default horizontal_offset of 0 and scroll left.
    let input = "#[|hello world]#\n";

    let mut app = helpers::AppBuilder::new()
        .with_input_text(input)
        .build()?;

    let events = vec![
        mouse_scroll_event(MouseEventKind::ScrollLeft, 1, 10),
        mouse_scroll_event(MouseEventKind::ScrollLeft, 1, 10),
    ];

    helpers::test_event_sequence(
        &mut app,
        events,
        Some(&|app| {
            let (view, doc) = current_ref!(app.editor);
            let offset = doc.view_offset(view.id);
            // horizontal_offset should remain at 0 (saturating subtraction).
            assert_eq!(
                offset.horizontal_offset, 0,
                "expected horizontal_offset == 0 after ScrollLeft from zero"
            );
        }),
        false,
    )
    .await
}

#[tokio::test(flavor = "multi_thread")]
async fn test_mouse_scroll_horizontal_soft_wrap_noop() -> anyhow::Result<()> {
    let long_line = "x".repeat(500);
    let input = format!("#[|{}]#\n", long_line);

    // Enable soft_wrap in the editor config.
    let config = helix_term::config::Config {
        editor: helix_view::editor::Config {
            soft_wrap: helix_core::syntax::config::SoftWrap {
                enable: Some(true),
                ..Default::default()
            },
            lsp: helix_view::editor::LspConfig {
                enable: false,
                ..Default::default()
            },
            ..Default::default()
        },
        keys: helix_term::keymap::default(),
        ..Default::default()
    };

    let mut app = helpers::AppBuilder::new()
        .with_config(config)
        .with_input_text(&input)
        .build()?;

    let events = vec![
        mouse_scroll_event(MouseEventKind::ScrollRight, 1, 10),
        mouse_scroll_event(MouseEventKind::ScrollRight, 1, 10),
    ];

    helpers::test_event_sequence(
        &mut app,
        events,
        Some(&|app| {
            let (view, doc) = current_ref!(app.editor);
            let offset = doc.view_offset(view.id);
            // With soft_wrap enabled, horizontal scroll should be a no-op.
            assert_eq!(
                offset.horizontal_offset, 0,
                "expected horizontal_offset == 0 with soft_wrap enabled"
            );
        }),
        false,
    )
    .await
}
