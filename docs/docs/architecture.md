---
title: "Architecture"
description: "How Dolphin sits between a backend and Husk."
---

# Architecture

```text
Husk (workbench host, slots, compositor)
   ^  Husk plugin protocol
Dolphin (Rust plugin)
   ^  backend bridge
Backend: herdr server | Orca | bb
```

Dolphin is one Husk plugin. It does not run its own plugin system. It publishes backend state (workspaces, tabs, panes, agents) on Husk's bus and streams the backend's pane surface into Husk's `main-stage`. Other Husk plugins subscribe to that state.

## Herdr side

Dolphin talks to an unmodified `herdr server` over two sockets:

- `herdr-client.sock` carries the bincode wire protocol, including `PaneSurfaceFrame` with the exact rectangle of every visible pane and split.
- `herdr.sock` carries JSON-RPC and `events.subscribe`.

Because the frame includes pane rectangles, plugin UI can be anchored to individual panes: header buttons, status pills and banners.

## What needs no server changes

The research counted 35 extension points. Of those, 30 can be built in the client alone, including sidebar replacement, row badges, docked panels, tab bar widgets, context menus, overlays, the command palette, theme hooks and client-diffed events. Five need changes inside `herdr server`: inline terminal embeds, synchronous veto hooks, internal PTY events, agent launch injection and remote host workers.