# Client Migration Notes

NetherLink client code should eventually replace:

- Instance startup and Minecraft identity verification with `/v1/instances`.
- `ClientFriendService` official friend list and Presence calls with one `/v1/friends` request using the returned
  `instanceToken`; each friend contains its active `presences` array.
- `PresencePublisher` with `/v1/presence` using the returned `instanceToken`.
- `SignalingClient` official signaling configuration and JSON-RPC calls with `/v1/signaling/ws`.
- TURN auth request with `/v1/turn`.

Instance registration automatically imports established official friends. To request best-effort removal from both
NetherLink and the official friend graph, the client may add its current Minecraft token as
`X-Minecraft-Access-Token` on `DELETE /v1/friends/{profileId}`. Ordinary NetherLink requests must not resend that token.

The existing WebRTC handshake message model can remain mostly unchanged. The transport envelope should route by
`profile_id + presence_id` so a friend can choose a specific running instance to join.

## Multi-account Processes

A physical mod process may manage multiple accounts without creating multiple HTTP connection pools. Use one shared
HTTP client and keep an account-session map such as `profileId -> instanceToken + presenceId`. Register each account
with a separate `POST /v1/instances` request, then discard that account's Minecraft access token after registration.

Presence, friend, TURN, renewal, and close requests must use the corresponding account's instance token. Do not combine
multiple Minecraft or instance tokens into one `Authorization` header. If several managed accounts need signaling, open
one authenticated WebSocket per account Presence; ordinary HTTP requests may continue sharing the same HTTP client.
