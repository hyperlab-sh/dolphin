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

Dolphin does not run its own plugin system. It is a set of Husk plugins. They read Herdr state and publish it to Husk, and later stream Herdr's pane surface into Husk's `main-stage`. Other Husk plugins can subscribe to that state.

## Herdr side

- `herdr.sock` carries JSON-RPC (`workspace.list`, `tab.list`, `pane.list`, ...) and `events.subscribe`. `herdr-sidebar` needs only this.
- `herdr-client.sock` carries the bincode wire protocol, including `PaneSurfaceFrame` with the rectangle of every visible pane and split. Rendering real panes will need it.

## Client versus server

Most interface features need no change to Herdr: sidebar replacement, row badges, docked panels, tab bar widgets, menus, overlays, theming. A few need server changes: inline terminal embeds, synchronous veto hooks, internal PTY events, and agent launch injection.
