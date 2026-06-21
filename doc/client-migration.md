# Client Migration Notes

NetherLink client code should eventually replace:

- Instance startup and Minecraft identity verification with `/v1/instances`.
- `ClientFriendService` official friend list and Presence calls with one `/v1/friends` request using the returned
  `instanceToken` and current Minecraft token; each friend contains its active `presences` array.
- `PresencePublisher` with `/v1/presence` using the returned `instanceToken`.
- `SignalingClient` official signaling configuration and JSON-RPC calls with `/v1/signaling/ws`.
- TURN auth request with `/v1/turn`.

Instance registration performs an initial best-effort official synchronization. Every `/v1/friends` request must add
the current account token as `X-Minecraft-Access-Token`. The server verifies that it belongs to the instance profile,
uses it for exactly one official friends request, and discards it.

The existing WebRTC handshake message model can remain mostly unchanged. The transport envelope should route by
`profile_id + presence_id` so a friend can choose a specific running instance to join.

## Multi-account Processes

A physical mod process may manage multiple accounts without creating multiple HTTP connection pools. Use one shared
HTTP client and keep an account-session map such as `profileId -> instanceToken + presenceId`. Register each account
with a separate `POST /v1/instances` request. Retain access to that account's current Minecraft token on the client for
friend operations; never send several tokens in one request.

Presence, friend, TURN, renewal, and close requests must use the corresponding account's instance token. Friend
requests additionally use that account's Minecraft token in `X-Minecraft-Access-Token`. Do not combine multiple
Minecraft or instance tokens into one `Authorization` header. If several managed accounts need signaling, open
one authenticated WebSocket per account Presence; ordinary HTTP requests may continue sharing the same HTTP client.
