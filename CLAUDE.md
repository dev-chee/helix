# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Helix is a Kakoune/Neovim-inspired modal text editor written in Rust. It features multiple selections as a core primitive, built-in LSP/DAP support, and tree-sitter-based syntax highlighting.

## Build & Run Commands

```bash
# Run in debug mode (faster compile, slower runtime)
cargo run

# Run with a log file for debugging
cargo run -- --log foo.log
# In another terminal: tail -f foo.log

# Build release
cargo build --release

# Run with log verbosity (for log::info! output)
cargo run -- -v <file>
```

## Testing

```bash
# Unit tests and doc tests across all packages
cargo test --workspace

# Integration tests (helix-term)
cargo integration-test

# Integration tests with logging
HELIX_LOG_LEVEL=debug cargo integration-test

# Run a single test by name
cargo test --workspace <test_name>
```

Integration tests live in [helix-term/tests/test/](helix-term/tests/test/). Use [helix-term/tests/test/helpers.rs](helix-term/tests/test/helpers.rs) for test utilities.

## xtask Commands

```bash
# Regenerate auto-generated docs (run after changing languages.toml or commands)
cargo xtask docgen

# Validate tree-sitter queries for all languages (or specific ones)
cargo xtask query-check
cargo xtask query-check rust python

# Validate theme files
cargo xtask theme-check
```

## Documentation

```bash
# Preview the book locally (requires mdbook)
mdbook serve book
```

## Debug Logging

Use `log::info!`, `log::warn!`, or `log::error!` macros. Pass `-v` (or more `v`s for higher verbosity) to enable log output. Use `:log-open` command inside Helix to view logs.

## Crate Architecture

The workspace is organized into these crates (dependency order, roughly):

| Crate | Role |
|-------|------|
| `helix-stdx` | Standard library extensions |
| `helix-parsec` | Parser combinators |
| `helix-core` | Core editing primitives: `Rope`, `Selection`, `Transaction`, `Syntax` (tree-sitter) |
| `helix-lsp-types` | LSP type definitions (forked) |
| `helix-dap-types` | DAP type definitions |
| `helix-event` | Event system with hooks and `AsyncHook` for debounced async tasks |
| `helix-loader` | Config/grammar loading, fetching tree-sitter grammars |
| `helix-vcs` | Version control integration |
| `helix-lsp` | LSP client |
| `helix-dap` | Debug Adapter Protocol client |
| `helix-tui` | TUI rendering primitives (forked from tui-rs): `Surface`, `Rect`, `Component` |
| `helix-view` | `Document`, `View`, `Editor` — UI-agnostic editor state |
| `helix-term` | Terminal frontend: `Application`, `commands.rs`, `keymap.rs`, `Compositor` |
| `xtask` | Build/codegen tasks |

### Key Concepts

- **`Rope`** (from `ropey`): The buffer data structure. Cheap to clone, enables easy snapshots.
- **`Selection`/`Range`**: Multiple selections are a first-class primitive. A cursor is just a selection with one range where head == anchor.
- **`Transaction`**: OT-like change representation applied to a Rope. Invertible for undo. Selections can be mapped through transactions.
- **`Document`**: Ties together Rope, Selections (per view), Syntax, History, and language server state.
- **`View`**: An open split in the UI. Holds the current document ID and scroll state. Multiple views can show the same document.
- **`Editor`**: Global state — all open documents, view tree, config, and language server registry.
- **`Compositor`**: Layer stack of `Component`s rendered in order (file picker over editor, etc.).
- **`commands.rs`** (helix-term): All editor commands tied to keybindings — the most important file for feature work.
- **`helix-event`**: Events (document open/change/close, LSP start/stop, etc.) dispatched to registered hooks. `AsyncHook` enables debounced async processing.

## Adding Language Support

1. Add `[[language]]` and optionally `[[grammar]]` entries in [languages.toml](languages.toml)
2. Create tree-sitter query files in `runtime/queries/<lang>/` (highlights.scm, indents.scm, etc.)
3. Run `cargo xtask docgen` to update generated docs
4. Set `HELIX_RUNTIME` env var to point to the local `runtime/` folder during development
5. Fetch/build grammars: `hx --grammar fetch && hx --grammar build`

## MSRV

The minimum supported Rust version is defined in three places (keep in sync when updating):
- `workspace.package.rust-version` in [Cargo.toml](Cargo.toml)
- `env.MSRV` in [.github/workflows/build.yml](.github/workflows/build.yml)
- `toolchain.channel` in `rust-toolchain.toml`
