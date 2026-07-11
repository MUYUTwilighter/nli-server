# Development Boundaries

The backend must not persist login state.

The backend must not persist Microsoft or Minecraft access tokens.

One physical mod process may register several accounts, but every registration authenticates exactly one Minecraft
token and produces exactly one account-bound runtime instance. The backend must not accept, combine, or retain a list of
Minecraft access tokens as one authentication identity.

The backend verifies a Minecraft access token during runtime instance registration and on every friend-list read or
friend mutation. Friend endpoints receive the current token in `X-Minecraft-Access-Token`, verify that it belongs to
the same profile as the runtime instance, call the official friends API, and discard it immediately. It is never stored
or logged.

Persistent storage is limited to:

- Friend relationship graph.

Runtime-only volatile state may include:

- Active WebSocket connections.
- In-flight signaling sessions.
- Short-lived request nonces or rate-limit buckets.
- Ephemeral server-issued session tokens, if needed, as long as they are not persisted.
- Presence state, preferably stored in Redis or another TTL-capable in-memory store.
