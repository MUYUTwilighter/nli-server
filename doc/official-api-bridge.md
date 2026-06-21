# Official Friends Bridge

The official Minecraft friends service is the Version 1 source of truth for friends and pending requests. PostgreSQL
stores a synchronized local projection solely for Presence visibility and WebSocket signaling authorization.

Every `GET /v1/friends` and friend mutation requires both credentials:

- `Authorization: Bearer <instance token>` identifies the active NetherLink runtime instance.
- `X-Minecraft-Access-Token: <Minecraft token>` authorizes the official request and must resolve to the same profile.

The Minecraft token is verified, used for that request, and discarded. It is never stored or logged.

Wire mapping:

```text
GET /v1/friends
  -> GET https://api.minecraftservices.com/friends

POST /v1/friends/requests {"name":"Player"}
  -> PUT /friends {"name":"Player","updateType":"ADD"}

POST /v1/friends/requests/{profileId}
  -> PUT /friends {"profileId":"uuid","updateType":"ADD"}

DELETE /v1/friends/requests/{profileId}
DELETE /v1/friends/{profileId}
  -> PUT /friends {"profileId":"uuid","updateType":"REMOVE"}
```

Successful official responses contain the complete friend, incoming-request, and outgoing-request snapshot. The
backend replaces the caller's local projection transactionally with that snapshot and caches returned profile names in
Redis. A later read or instance registration repairs the projection if an official operation succeeded but local
storage was temporarily unavailable.

Friend mutations fail when the official service fails; the backend does not create a local-only relationship. Instance
registration still treats initial synchronization as best effort so an official outage does not prevent startup.

Migration `202606210001_reset_official_friend_graph.sql` discards all pre-bridge relationships and requests. Version 1
does not attempt to promote legacy NetherLink-only relationships into official friendships. Migration
`202606210002_restrict_friend_sources.sql` then restricts both projection tables to `minecraft_sync` rows.
