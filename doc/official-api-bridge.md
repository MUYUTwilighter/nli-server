# Official API Bridge

Official bridge behavior should be best effort, never the only source of truth.

Implemented bridge behavior:

- During `POST /v1/instances`, fetch `GET https://api.minecraftservices.com/friends` with the same Minecraft token and
  import established official friends as `minecraft_import` relationships. Import failure never blocks registration.
- During `DELETE /v1/friends/{profileId}`, optionally accept `X-Minecraft-Access-Token`, verify that it belongs to the
  same profile as the instance token, and call the official friends endpoint with `updateType=REMOVE`.
- Cache imported profile names in Redis. No official token or official response is persisted.
- Do not block NetherLink add or accept if official add is blocked by official Presence requirements.
- Record official sync results in structured logs and Prometheus counters.

No persistent sync metadata table is created because persistent storage remains limited to the friend graph.
Operational results use `nli_official_friend_sync_total` with operation and result labels.

The official wire requests match the existing NetherLink client implementation:

```text
GET /friends
Authorization: Bearer <minecraft token>

PUT /friends
Authorization: Bearer <minecraft token>
{"profileId":"uuid","updateType":"REMOVE"}
```
