# Husk protocol fixtures

`envelopes.json` contains one complete example for each of the 14 message types in Husk's public Go protocol. Field names, required zero-value fields and `omitempty` behavior follow the Go structs' JSON tags, including the palette and nested command, keybinding, rail-item and hit-target types.

Round-trip tests compare JSON values, not key ordering. `close_modal` is a boolean envelope field, not an object payload.
