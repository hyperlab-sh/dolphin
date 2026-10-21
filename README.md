# Dolphin

An agentic development environment (ADE) in the terminal. Dolphin is a TUI built on [Husk](https://github.com/hyperlab-sh/husk) and backed by [Herdr](https://herdr.dev), the terminal multiplexer for coding agents.

Herdr runs the workspaces, tabs, panes and agents. Dolphin replaces its client: it owns the interface (sidebar, rails, panels, overlays) and talks to an unmodified `herdr server`. Because the interface is Husk plugins rather than tab-scoped panes, it stays put when you switch workspaces instead of respawning and resizing the terminal.

## Status

Early. Dolphin is implemented in **Rust**, and the Rust code does not exist yet. The only code is a throwaway Go prototype in [`spike/go`](spike/go): a `herdr-sidebar` Husk plugin that connects to a running Herdr and shows spaces, worktrees and agent status.

Planned, in order:

1. Research the APIs and protocols of Orca and bb and compare them with Herdr's, then decide whether Dolphin defines a backend contract with Herdr as an adapter, or builds directly on Herdr. Backend independence may not be achievable.
2. Set up the Rust workspace and port the sidebar from the spike; delete the spike at parity.
3. Replicate the Herdr UI (real tabs and panes) as a starting point.

Work is tracked in beads (`dolphin-*`, shared with Husk since Dolphin depends on Husk features).

## Build

There is no Rust build yet. To try the Go spike (it imports Husk's public packages, so a gitignored `go.work` pointing at a local Husk checkout is needed):

```bash
cd spike/go
go build -o bin/herdr-sidebar ./cmd/herdr-sidebar
```

It connects to `$HERDR_SOCKET_PATH`, or `~/.config/herdr/herdr.sock`.
