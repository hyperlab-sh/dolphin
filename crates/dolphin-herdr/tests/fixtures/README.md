# Herdr wire fixtures

Recorded from the public, read-only commands `herdr workspace list`, `herdr agent list` and `herdr pane layout --current`, plus a `session.snapshot` JSON-RPC call.

Identifiers are consistently remapped to `example-*`; labels, titles, repository names, session values and free-form token values are replaced with `example`. Paths are replaced with `/home/example/project`. Numeric values, status values, protocol versions and wire structure are retained. Extra server fields intentionally remain to exercise forward-compatible decoding.

Tests read these fixtures and use isolated mock Unix sockets; they never focus, create, close or otherwise modify a live session.
