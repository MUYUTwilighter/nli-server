# MVP Order

Implemented:

1. Runtime instance registration that verifies Minecraft identity once and returns `profileId`, `name`, `presenceId`,
   and `instanceToken` without persisted login state.
2. Friend graph CRUD using the runtime instance token.
3. Redis-backed Presence publish/query with TTL expiry.
4. WebSocket signaling relay between friends.
5. TURN credential endpoint.
6. Official friend list import during instance registration.
7. Best-effort official deletion bridge using an optional one-use Minecraft token.

Deferred:

8. Optional official add bridge if Mojang allows it again.
