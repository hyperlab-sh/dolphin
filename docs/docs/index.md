---
title: "Dolphin"
description: "An agentic development environment TUI built on Husk and backed by Herdr."
---

# Dolphin

Dolphin is an agentic development environment (ADE) in the terminal. It is built on [Husk](https://github.com/hyperlab-sh/husk) and backed by [Herdr](https://herdr.dev).

## Why

Herdr runs workspaces, tabs, panes and coding agents, but its plugins can only open panes inside a tab's layout tree. A custom sidebar built that way disappears and respawns on every workspace switch, and causes two full-terminal resizes each time. App chrome has to live in the client, outside the server's per-tab layout.

Herdr already has the right boundary: the client owns the outer terminal layout and tells the server how big the pane surface is. Dolphin replaces the client, keeps an unmodified `herdr server`, and builds its interface from Husk plugins.

## Status

Early. Dolphin is implemented in Rust, and the Rust code is not written yet. A throwaway Go prototype of the Herdr sidebar plugin exists as reference. See [Architecture](/architecture/) and [Backends](/backends/).
