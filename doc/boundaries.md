# Development Boundaries

The backend must not persist login state.

The backend must not persist Microsoft or Minecraft access tokens.

The backend may verify a Minecraft access token during a request or session handshake, but must discard it immediately
after validation.

Persistent storage is limited to:

- Friend relationship graph.

Runtime-only volatile state may include:

- Active WebSocket connections.
- In-flight signaling sessions.
- Short-lived request nonces or rate-limit buckets.
- Ephemeral server-issued session tokens, if needed, as long as they are not persisted.
- Presence state, preferably stored in Redis or another TTL-capable in-memory store.
