# Open Questions

- Should signaling require a host-issued invite token in addition to friend relationship and `HOSTING` Presence?

Resolved decisions:

- Friend requests are created by player name; subsequent relationship operations use profile UUIDs.
- Only the latest known player name is cached. Rename history is not persisted.
- Established official friends are imported directly as `minecraft_import` friendships during instance registration.
