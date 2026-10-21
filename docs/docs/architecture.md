---
title: "Architecture"
description: "How Dolphin sits between Herdr and Husk."
---

# Architecture

```text
Husk (workbench host: slots, compositor, events)
   ^  Husk plugin protocol (newline-delimited JSON)
Dolphin plugins (herdr-sidebar, ...)
   ^  herdr.sock (JSON-RPC, events)   herdr-client.sock (bincode)
Herdr server (workspaces, tabs, panes, PTYs, agent detection)
```

Dolphin is written in Rust. Its sidebar uses serde/serde_json to mirror the public wire types, with no dependencies on Herdr's own crates. Because Husk plugins speak JSON lines, the language choice needs nothing from Husk. Dolphin does not run its own plugin system. It is a set of Husk plugins. They read Herdr state and publish it to Husk; streaming Herdr's terminal pane surfaces into Husk's `main-stage` is future work. Other Husk plugins can subscribe to the selected workspace context.

## Rust workspace

- `crates/husk-plugin`: typed envelopes and payloads for Husk's plugin protocol, theme palette, bounded read loop and send function. It connects to `HUSK_SOCKET_PATH`, or uses stdin/stdout when unset. No Herdr code lives here.
- `crates/dolphin-herdr`: Herdr-only snapshot/layout types, socket path resolution, typed RPC methods and an independent event subscription connection. Each ordinary RPC owns a short-lived socket with a timeout.
- `crates/herdr-sidebar`: the plugin binary, split/tree/agent view state, priority/space sorting, keyboard and mouse input, ANSI true-color rendering, workspace-context publication and `--probe`.

A single-thread Tokio runtime serializes sidebar state updates. An asynchronous reader handles Husk input, and a dedicated Herdr event task reconnects after disconnection. A 220 ms timer animates working-agent indicators. Host EOF aborts both tasks and closes their sockets. The generic Husk transport ignores malformed JSON messages, bounds line sizes and propagates I/O errors.

The sidebar renders navigation, list, footer, header, topology stage and inspector slots. As in the Go reference, it advertises `thread-row-status` without emitting a separate row-status surface. It registers no activity-rail items. The Go spike remains under `spike/go` for comparison.

## Build and checks

Requires Rust 1.88 or newer on Unix:

```bash
cargo build --release
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
./target/release/herdr-sidebar --probe
```

Use `cargo test -p <changed-crate>` for affected tests, including dependent crates for shared changes. Tests cover Go-compatible Husk payload fixtures, sanitized live Herdr responses, RPC request parameters and sidebar input/render state without modifying a live server.

## Herdr side

- `herdr.sock` carries JSON-RPC (`session.snapshot`, workspace/tab/pane/agent focus, `workspace.create`, `pane.read`, `pane.layout`) and `events.subscribe`. `herdr-sidebar` needs only this. Socket resolution uses `HERDR_SOCKET_PATH`, then `XDG_CONFIG_HOME/herdr/herdr.sock`, then `~/.config/herdr/herdr.sock`.
- `herdr-client.sock` carries the bincode wire protocol, including `PaneSurfaceFrame` with the rectangle of every visible pane and split. Rendering real panes will need it.

## Client versus server

Most interface features need no change to Herdr: sidebar replacement, row badges, docked panels, tab bar widgets, menus, overlays, theming. A few need server changes: inline terminal embeds, synchronous veto hooks, internal PTY events, and agent launch injection.
