---
title: "Backends"
description: "Herdr today, other backends under evaluation."
---

# Backends

Dolphin is backed by Herdr. Support for other backends is an open question, not a promise.

| Backend | State |
| :--- | :--- |
| Herdr | Supported. Dolphin talks to an unmodified `herdr server` |
| Orca | Under evaluation |
| bb | Under evaluation |

## Evaluation

The first planned task compares the APIs and protocols of Orca and bb with Herdr's: how workspaces, agents, terminals and events are modeled, and how a client attaches. Full backend independence may not be achievable, since the models can diverge. The result decides one of two paths:

1. Define a backend contract and implement the Herdr connection as one adapter.
2. Build directly on Herdr.
