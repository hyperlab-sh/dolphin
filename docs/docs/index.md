---
title: "Dolphin"
description: "A pluggable client for terminal agent multiplexers, built on Husk."
---

# Dolphin

Dolphin is a pluggable client for terminal agent multiplexers. It started as an alternative client for Herdr and is now a Rust plugin for [Husk](https://github.com/hyperlab-sh/husk).

## Why

Herdr plugins can only open panes inside a tab's layout tree. A custom sidebar built that way disappears and respawns on every workspace switch, and causes two full-terminal resizes each time. App chrome has to live in the client, outside the server's per-tab layout.

Herdr already has the right boundary: the client owns the outer terminal layout and tells the server how big the pane surface is. Dolphin takes over the client side and keeps an unmodified `herdr server`.

## Status

Research phase, no code yet.

Read [Architecture](/architecture/) for the design and [Backends](/backends/) for what Dolphin will run on.
