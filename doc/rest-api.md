# REST API Draft

The client submits its Minecraft access token when creating a runtime instance. Subsequent authenticated endpoints
derive the caller profile and Presence from the returned `instanceToken`. Every friend-list read and mutation also
submits the current token in `X-Minecraft-Access-Token`; it is verified, used for one official request, and never
persisted.

A runtime instance is an account-level authorization unit. One physical mod process may register several accounts by
calling `POST /v1/instances` separately for each account and may reuse one HTTP connection pool, but every request uses
exactly one account's bearer token.

Use JSON for request and response bodies.

The backend should replace these current client responsibilities:

- Friend list query.
- Add, accept, decline, revoke, remove friend.
- Presence publish and friend Presence query.
- WebRTC signaling message relay.
- TURN credential retrieval or relay of TURN configuration.

## Health

```http
GET /health
```

Returns service status.

Healthy response:

```json
{
  "status": "ok",
  "dependencies": {
    "postgres": {
      "healthy": true,
      "latencyMs": 2
    },
    "redis": {
      "healthy": true,
      "latencyMs": 1
    }
  }
}
```

The endpoint returns `503 Service Unavailable` with `status=degraded` when either dependency fails or exceeds the
health-check timeout. Dependency errors are logged internally but are not included in the response.

Prometheus metrics are available from `GET /metrics`. When `NLI_METRICS_TOKEN` is set, requests must include
`Authorization: Bearer <metrics_token>`. Metrics include HTTP request counts and latency, active signaling WebSockets,
rate-limit events, Minecraft upstream failures, and official friend synchronization results.

## Service Terms

```http
GET /v1/terms
Accept-Language: zh-CN, en;q=0.8
```

The public endpoint returns the current Version 1 service terms as `text/plain; charset=utf-8`. It supports English and
Chinese, sets `Content-Language` to `en` or `zh`, and defaults to English when no supported language is requested.

A client that needs to select the language explicitly may use a JSON body. The body takes precedence over
`Accept-Language`; regional tags such as `en-US` and `zh-CN` are accepted.

```http
POST /v1/terms
Accept-Language: en
Content-Type: application/json

{
  "language": "zh"
}
```

## Create Runtime Instance

```http
POST /v1/instances
Authorization: Bearer <minecraft_access_token>
Content-Type: application/json

{
  "displayText": "Minecraft Java instance"
}
```

The backend validates the Minecraft token once, discards it, creates a fresh runtime Presence entry with
`status=ONLINE`, and returns the verified identity plus a private token for this account-level runtime instance.

Response:

```json
{
  "profileId": "uuid",
  "name": "PlayerName",
  "presenceId": "public-presence-id",
  "instanceToken": "private-runtime-token",
  "expiresAt": "2026-06-01T15:04:05Z"
}
```

`presenceId` is public and may be shown to friends or used as a signaling target. `instanceToken` is private and must be
used for Presence updates and the signaling WebSocket.

Registration also attempts to synchronize the complete official friend and pending-request snapshot. This is best
effort: registration succeeds even when the official friends endpoint is unavailable or returns an incompatible
response.

Instance token requirements:

- A Minecraft profile may have at most five active runtime instances. A sixth registration returns
  `409 INSTANCE_LIMIT_REACHED`.
- Token TTL should be short, initially 15 to 30 minutes.
- The client should renew the runtime instance before token expiry.
- The token must bind `profileId` and `presenceId`.
- If using opaque random tokens, store only `hash(instanceToken) -> profileId, presenceId, expiresAt` in Redis.
- If using stateless signed tokens, prefer PASETO or JWT with strong signing keys and short expiry.
- Expiring an instance token should also make its WebSocket unusable and allow its Presence to expire naturally.

## Renew Runtime Instance

```http
POST /v1/instances/renew
Authorization: Bearer <instance_token>
```

Response:

```json
{
  "profileId": "uuid",
  "presenceId": "public-presence-id",
  "instanceToken": "new-private-runtime-token",
  "expiresAt": "2026-06-01T15:19:05Z"
}
```

Renewal should rotate the token. The old token should be invalidated immediately if opaque tokens are used. If stateless
tokens are used, keep the old token lifetime short and optionally maintain a small Redis revocation set until expiry.

## Close Runtime Instance

```http
DELETE /v1/instances/current
Authorization: Bearer <instance_token>
```

Returns `204 No Content`. The operation atomically invalidates the runtime token and removes its Presence, closes the
corresponding signaling WebSocket, and deletes every signaling session involving that `presenceId`. The same token is
invalid after a successful close, so a repeated request with it returns `401 INVALID_INSTANCE_TOKEN`. Closing or expiry
releases one of the profile's five runtime-instance slots.

## Friend Snapshot

```http
GET /v1/friends
Authorization: Bearer <instance_token>
X-Minecraft-Access-Token: <minecraft_access_token>
```

Response:

```json
{
  "friends": [
    {
      "profileId": "uuid",
      "name": "PlayerName",
      "source": "minecraft_sync",
      "presences": [
        {
          "profileId": "uuid",
          "presenceId": "public-presence-id",
          "status": "HOSTING",
          "joinable": true,
          "sessionId": "host-session-id",
          "endpoint": null,
          "displayText": "1.18.2 Forge - Singleplayer world",
          "updatedAt": "2026-06-01T15:03:35Z",
          "expiresAt": "2026-06-01T15:04:35Z"
        }
      ]
    }
  ],
  "incomingRequests": [],
  "outgoingRequests": []
}
```

Before assembling this response, the backend refreshes and transactionally mirrors the complete official friend
snapshot. Returned official names are cached in Redis. `presences` contains every active NetherLink runtime instance
for that friend and is empty when the friend has none. The caller's own Presence and pending-request Presence are not
included.

## Add Friend

```http
POST /v1/friends/requests
Authorization: Bearer <instance_token>
X-Minecraft-Access-Token: <minecraft_access_token>
Content-Type: application/json

{
  "name": "PlayerName"
}
```

Names must contain 3 to 16 ASCII letters, digits, or underscores. The backend sends an official `ADD` update by name,
then replaces the caller's local projection with the official response.

Response:

```json
{
  "result": "SUCCESS",
  "relationship": "REQUESTED",
  "officialSync": "SUCCESS"
}
```

`relationship` is `REQUESTED` when the official response contains an outgoing request and `ACCEPTED` when it contains
an established friendship. `officialSync` is `SUCCESS`; failed official operations return an HTTP error and do not
create local-only relationships.

## Accept Request

```http
POST /v1/friends/requests/{profileId}
Authorization: Bearer <instance_token>
X-Minecraft-Access-Token: <minecraft_access_token>
```

Sends an official `ADD` update by profile UUID and synchronizes the returned snapshot.

Response:

```json
{
  "result": "SUCCESS",
  "relationship": "ACCEPTED",
  "officialSync": "SUCCESS"
}
```

## Decline Or Revoke Request

```http
DELETE /v1/friends/requests/{profileId}
Authorization: Bearer <instance_token>
X-Minecraft-Access-Token: <minecraft_access_token>
```

Sends an official `REMOVE` update to decline an incoming request or revoke an outgoing request, synchronizes the
returned snapshot, and returns `204 No Content`.

## Remove Friend

```http
DELETE /v1/friends/{profileId}
Authorization: Bearer <instance_token>
X-Minecraft-Access-Token: <minecraft_access_token>
```

Sends an official `REMOVE` update, synchronizes the returned snapshot, and returns `204 No Content`.

Friend request, acceptance, decline, revoke, and removal operations share a limit of 10 attempts per caller per minute.
Missing or mismatched Minecraft credentials return `401`; official permission, rate-limit, malformed-response, and
availability failures map to `403`, `429`, `502`, and `503` respectively.

## Publish Presence

```http
PUT /v1/presence
Authorization: Bearer <instance_token>
Content-Type: application/json

{
  "status": "HOSTING",
  "joinable": true,
  "sessionId": "optional-host-session-id",
  "endpoint": "optional",
  "ttlSeconds": 90,
  "displayText": "1.18.2 Forge - Singleplayer world"
}
```

Response:

```json
{
  "result": "SUCCESS",
  "profileId": "uuid",
  "presenceId": "public-presence-id",
  "expiresAt": "2026-06-01T15:04:05Z"
}
```

The backend should derive `profileId` and `presenceId` from the `instance_token`, not from the request body. It should
clamp TTL to a safe range, for example 30 to 180 seconds.

Presence update rules:

- The request body must not be trusted for `profileId` or `presenceId`.
- Only the `instanceToken` holder can update that Presence entry.
- `displayText` is display-only and should be sanitized before storing.
- `joinable=true` should be allowed only for statuses that the server accepts as host-capable, initially `HOSTING`.
- Updating Presence should refresh the Redis TTL.
- `OFFLINE` is rejected by this endpoint; clients should use `DELETE /v1/presence` instead.
- Presence publishing is limited to one successful publish attempt per runtime instance every 10 seconds.
- `sessionId` is limited to 128 characters and `endpoint` to 512 characters. Control characters are rejected.
- If `displayText` is omitted, the existing value is retained. If no Presence exists, the generic fallback is used.

## Clear Presence

```http
DELETE /v1/presence
Authorization: Bearer <instance_token>
```

Sets the Presence associated with this instance token to `OFFLINE` or removes it.

Returns `204 No Content`. The operation is idempotent while the runtime instance token remains valid.
