# Dolphin — Agent Instructions

Dolphin is an agentic development environment (ADE) TUI built on [Husk](https://github.com/hyperlab-sh/husk) and backed by Herdr. Herdr runs workspaces, tabs, panes and agents; Dolphin replaces Herdr's client. It talks to an unmodified `herdr server` over `herdr.sock` (JSON-RPC and events) and `herdr-client.sock` (bincode, `PaneSurfaceFrame`), and presents the result as Husk plugins and slots.

Dolphin builds directly on Herdr. Do not add a backend abstraction or backend trait. All Herdr-specific protocol code belongs in `dolphin-herdr`; the generic `husk-plugin` crate must stay independent of Herdr.

## Language

Dolphin is implemented in **Rust** (minimum toolchain 1.88). The sidebar uses serde/serde_json and Tokio to speak the public socket protocols, without depending on Herdr's own crates. Do not add new features in Go.

`spike/go` is a throwaway Go prototype of the Herdr sidebar (a Husk plugin). It is reference material to port from and will be deleted when the Rust version reaches parity.

## Build & Test

From the repository root:

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
./target/release/herdr-sidebar --probe
```

Run affected tests with `cargo test -p <changed-crate>`; include dependent crates when changing shared protocol code. All three crates have unit tests; Herdr fixtures are sanitized recorded responses, and RPC tests use isolated mock Unix sockets. Tests do not mutate a live Herdr server.

`--probe` reads a live snapshot and exits. Normal plugin execution connects to `$HUSK_SOCKET_PATH`, or stdin/stdout when unset. Herdr socket resolution is `$HERDR_SOCKET_PATH`, `$XDG_CONFIG_HOME/herdr/herdr.sock`, then `~/.config/herdr/herdr.sock`. Keep diagnostics on stderr; stdout is the plugin protocol.

## Layout

| Path | Purpose |
| :--- | :--- |
| `crates/husk-plugin/` | Generic Husk protocol payloads and newline-delimited JSON socket/stdio transport |
| `crates/dolphin-herdr/` | Herdr RPC client, typed snapshots/layouts, events and sanitized fixtures |
| `crates/herdr-sidebar/` | Rust sidebar binary, state machine, ANSI rendering and Husk manifest |
| `spike/go/cmd/herdr-sidebar/` | Go reference prototype; retained until a later cleanup task |
| `spike/go/pkg/herdr/` | Prototype Herdr JSON-RPC socket client |
| `spike/go/plugins/herdr-sidebar/husk-plugin.json` | Prototype plugin manifest |
| `docs/` | docmd documentation site |

## Conventions

- Husk owns the UI host. Dolphin must use Husk's public protocol and never reach around it.
- Herdr-specific code stays in its own module (the Herdr client) and the plugins that use it.
- Conventional commits with detailed messages. No attribution trailers.

## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. The database is shared with Husk (Dolphin issues use the `dolphin-` prefix, Husk issues `husk-`; a Dolphin issue may depend on a Husk one). Run `bd prime` for full workflow context.

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

- Use `bd` for ALL task tracking. Do not use TodoWrite, TaskCreate, or markdown TODO lists.
- Do not commit or push unless asked. At handoff, report changed files, validation, and suggested next commands.
