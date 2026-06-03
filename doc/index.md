# NetherLink Server Docs

NetherLink Server is a planned standalone Rust backend for NetherLink's Minecraft Java P2P multiplayer flow. It provides
REST endpoints for friends, runtime instances, Presence, TURN credentials, and a WebSocket relay for WebRTC signaling.

The backend should stop treating Mojang Presence and official signaling as the source of truth. It should instead offer a
small service layer that keeps persistent data narrow and stores runtime multiplayer state as short-lived volatile data.

## Document Map

- [Project Context](context.md): why the backend exists and what official behavior it replaces.
- [Development Boundaries](boundaries.md): non-negotiable persistence, token, and runtime-state limits.
- [Identity](identity.md): Minecraft account identity and runtime instance identity.
- [Data Model](data-model.md): friend graph, pending requests, and volatile Presence records.
- [REST API Draft](rest-api.md): HTTP endpoints for auth, friends, instances, and Presence.
- [Signaling API Draft](signaling-api.md): WebSocket envelope, forwarding rules, and signaling errors.
- [TURN Auth](turn-auth.md): temporary TURN credential endpoint requirements.
- [Official API Bridge](official-api-bridge.md): best-effort Mojang API integration behavior.
- [Security](security.md): security requirements and recommended rate limits.
- [MVP Order](mvp-order.md): suggested implementation order.
- [Client Migration Notes](client-migration.md): current client responsibilities to replace.
- [Open Questions](open-questions.md): unresolved product and architecture choices.

## Core Development Direction

Start from the hard boundaries, then build the smallest service that satisfies the current client migration path:

1. Validate Minecraft identity without persisting login state or access tokens.
2. Issue short-lived runtime instance tokens for active game processes.
3. Store only the friend relationship graph persistently.
4. Keep Presence, signaling sessions, and rate-limit state volatile.
5. Relay WebRTC signaling only between authenticated friends and specific active Presence entries.
