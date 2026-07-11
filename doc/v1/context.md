# Project Context

Mojang has temporarily disabled the official Java P2P multiplayer path. Current testing shows:

- The official friend list can still be queried.
- Removing friends still works through the official API.
- Publishing the player's own Presence no longer works reliably.
- Adding friends is blocked by official requirements such as the target being `ONLINE`.
- Joining hosted friend games is blocked because official Presence no longer reaches `PLAYING_HOSTED_SERVER`.

NetherLink should therefore stop treating Mojang Presence and signaling as the source of truth. The backend should
provide a small relay service for in-game friends, Presence, and WebRTC signaling.
