---
title: "Backends"
description: "Supported and planned backends."
---

# Backends

| Backend | State | Notes |
| :--- | :--- | :--- |
| Herdr | First target | Client is replaceable over its existing sockets |
| Orca | Planned | Needs a backend abstraction separating Herdr-specific bridging from published state |
| bb | Analysis pending | Compare bb's thread and plugin model with Dolphin's workspace, tab, pane and agent model |

## Backend abstraction

Backend code sits behind an interface so the state Dolphin publishes on the Husk bus does not depend on which backend produced it. Slots and plugins written against that state work with any backend that can fill it.
