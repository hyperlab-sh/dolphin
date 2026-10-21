# Dolphin — Agent Instructions

Dolphin is a pluggable client for terminal agent multiplexers. It started as an alternative client for Herdr and is being built as a Rust plugin for [Husk](https://github.com/hyperlab-sh/husk), the generic TUI workbench. Dolphin bridges a backend to Husk's slots: it speaks `herdr-client.sock` (bincode, PaneSurfaceFrame) and `herdr.sock` (JSON-RPC and events) to an unmodified `herdr server` on one side, and Husk's plugin protocol on the other.

Planned backends: Herdr (first), Orca, and possibly `bb` (analysis pending).

This is a hyper space: project files live in `worktrees/main/`; `bin/`, `data/`, `notes/`, `scratch/` at the space root are local-only. Read `HYPER.md` at the space root for layout rules.

## State

There is no code yet. The repository holds only the docs site (`docs/`, docmd). The design research is not in the repo: it lives in the space's local-only `notes/research/` (`../../notes/research/` from the worktree). `00-overview-and-roadmap.md` there is the index; docs 07–09 (Husk architecture, prior art, language analysis) are in the Husk space's `notes/research/`.

## Conventions

- Husk owns the UI host; Dolphin must use Husk's public protocol, never reach around it.
- Backend-specific code (Herdr, Orca, bb) sits behind a backend abstraction so the state published on the Husk bus is backend-neutral.
- Conventional commits with detailed messages. No attribution trailers.

## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. The database is shared with Husk and lives at `/work/lab/.beads` (Dolphin issues use the `dolphin-` prefix, Husk issues `husk-`; a Dolphin issue may depend on a Husk one). Run `bd prime` for full workflow context.

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

- Use `bd` for ALL task tracking. Do not use TodoWrite, TaskCreate, or markdown TODO lists.
- Do not commit or push unless asked. At handoff, report changed files, validation, and suggested next commands.
