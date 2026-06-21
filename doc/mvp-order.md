# MVP Order

Implemented:

1. Runtime instance registration that verifies Minecraft identity once and returns `profileId`, `name`, `presenceId`,
   and `instanceToken` without persisted login state.
2. Official friend graph bridge using the runtime instance token and a per-request Minecraft token.
3. Redis-backed Presence publish/query with TTL expiry.
4. WebSocket signaling relay between friends.
5. TURN credential endpoint.
6. Complete official friend and request synchronization during registration and friend operations.
7. Official add, accept, decline, revoke, and remove bridge.

Deferred:

8. Official Presence integration beyond NetherLink world availability.
