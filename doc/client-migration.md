# Client Migration Notes

NetherLink client code should eventually replace:

- Instance startup and Minecraft identity verification with `/v1/instances`.
- `ClientFriendService` official friend list calls with `/v1/friends` using the returned `instanceToken`.
- `PresencePublisher` with `/v1/presence` using the returned `instanceToken`.
- `SignalingClient` official signaling configuration and JSON-RPC calls with `/v1/signaling/ws`.
- TURN auth request with `/v1/turn`.

The existing WebRTC handshake message model can remain mostly unchanged. The transport envelope should route by
`profile_id + presence_id` so a friend can choose a specific running instance to join.
