# Dolphin

A pluggable client for terminal agent multiplexers, built as a Rust plugin for [Husk](https://github.com/hyperlab-sh/husk).

Dolphin began as an alternative client for [Herdr](https://herdr.dev). Herdr already separates its client shell from its headless server, so a replacement client can own all the UI chrome (sidebars, rails, overlays, panels) while an unmodified `herdr server` keeps managing PTYs, layouts and agent detection. The research found that about 65% of the extension points a `bb`-style plugin system would want need no server changes.

## Status

Research phase. There is no code yet.

- [`docs/`](docs/) is the docmd documentation site.
- Design research is kept outside the repository, in the project space's `notes/research/`.

## Backends

| Backend | State |
| :--- | :--- |
| Herdr | First target, researched |
| Orca | Planned |
| bb | Feasibility analysis pending |

Work is tracked in beads (`dolphin-*` issues, shared with Husk).
