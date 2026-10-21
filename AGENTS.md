# Dolphin — Agent Instructions

Dolphin is an agentic development environment (ADE) TUI built on [Husk](https://github.com/hyperlab-sh/husk) and backed by Herdr. Herdr runs workspaces, tabs, panes and agents; Dolphin replaces Herdr's client. It talks to an unmodified `herdr server` over `herdr.sock` (JSON-RPC and events) and `herdr-client.sock` (bincode, `PaneSurfaceFrame`), and presents the result as Husk plugins and slots.

Whether other backends (Orca, bb) can be supported is an open question. The first task (`dolphin-res`) researches their APIs and decides if Dolphin defines a backend contract with Herdr as an adapter. Do not build backend abstractions before that decision.

## Language

Dolphin is implemented in **Rust** so it can reuse Herdr's protocol types and bincode codecs. The Rust workspace does not exist yet (`dolphin-rs`, after `dolphin-res`). Do not add new features in Go.

`spike/go` is a throwaway Go prototype of the Herdr sidebar (a Husk plugin). It is reference material to port from and will be deleted when the Rust version reaches parity.

## Build & Test

No Rust build yet. The Go spike:

```bash
cd spike/go
go build -o bin/herdr-sidebar ./cmd/herdr-sidebar
go vet ./...
```

The spike imports `github.com/hyperlab-sh/husk/pkg/huskipc` and `pkg/theme`. For local development use a gitignored `go.work` that includes a local Husk checkout:

```bash
go work init . <path-to-husk-checkout>
```

There are no tests yet.

## Layout

| Path | Purpose |
| :--- | :--- |
| `spike/go/cmd/herdr-sidebar/` | Go prototype Husk plugin: spaces, worktrees, tabs and agents from `herdr.sock`; publishes `workspace_context` |
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
