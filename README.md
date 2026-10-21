# Dolphin

An agentic development environment (ADE) in the terminal. Dolphin is a TUI built on [Husk](https://github.com/hyperlab-sh/husk) and backed by [Herdr](https://herdr.dev), the terminal multiplexer for coding agents.

Herdr runs the workspaces, tabs, panes and agents. Dolphin replaces its client: it owns the interface (sidebar, rails, panels, overlays) and talks to an unmodified `herdr server`. Because the interface is Husk plugins rather than tab-scoped panes, it stays put when you switch workspaces instead of respawning and resizing the terminal.

## Status

Early. Dolphin's **Rust workspace** contains the `herdr-sidebar` Husk plugin, a generic Husk protocol client, and a Herdr JSON-RPC client. The sidebar shows live spaces, worktrees, tabs, panes, agent status and topology; it publishes the selected workspace context to Husk.

Dolphin builds directly on Herdr, with no backend abstraction. The throwaway [Go spike](spike/go) remains as reference material; deleting it and rendering real terminal pane surfaces are later tasks.

Work is tracked in beads (`dolphin-*`, shared with Husk since Dolphin depends on Husk features).

## Build

Requires Rust 1.88 or newer and a running Herdr server on Unix.

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check

# Read-only live connectivity check; no Husk host required.
./target/release/herdr-sidebar --probe
```

For affected tests, use `cargo test -p husk-plugin`, `cargo test -p dolphin-herdr`, or `cargo test -p herdr-sidebar`. Run tests for downstream crates when changing a shared crate.

The binary connects to `$HUSK_SOCKET_PATH`, or speaks newline-delimited JSON on stdin/stdout when unset. Configure Husk with [`crates/herdr-sidebar/husk-plugin.json`](crates/herdr-sidebar/husk-plugin.json), launching from the repository root. Herdr socket lookup uses `$HERDR_SOCKET_PATH`, then `$XDG_CONFIG_HOME/herdr/herdr.sock`, then `~/.config/herdr/herdr.sock`.

## Workspace

| Crate | Purpose |
| :--- | :--- |
| `crates/husk-plugin` | Typed Husk envelopes, payloads, palette and socket/stdio transport; no Herdr dependencies |
| `crates/dolphin-herdr` | Herdr JSON-RPC calls, snapshot types and dedicated event subscription connection |
| `crates/herdr-sidebar` | Sidebar binary, view state, rendering, manifest and read-only `--probe` |

In Husk, use `1`/`2`/`3` for split/tree/agent views, `j`/`k` to navigate, Enter to focus the selection in Herdr, `o` to sort, `r` to refresh, and `n` to create a workspace. A first click selects an item; a second click focuses it.
