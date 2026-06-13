# REST API Draft

All authenticated endpoints should resolve the caller profile from the provided Minecraft token or NetherLink short
session token.

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

## Auth Probe

```http
POST /v1/auth/verify
Authorization: Bearer <minecraft_access_token>
```

Response:

```json
{
  "profileId": "uuid",
  "name": "PlayerName",
  "sessionToken": "optional-short-lived-token",
  "expiresIn": 900
}
```

The backend must not persist the Minecraft token.

## Create Runtime Instance

```http
POST /v1/instances
Authorization: Bearer <minecraft_access_token>
Content-Type: application/json

{
  "pmid": "optional-official-pmid",
  "displayText": "Minecraft Java instance"
}
```

The backend validates the Minecraft token, discards it, creates a fresh runtime Presence entry with `status=ONLINE`,
and returns a private token for this game process.

Response:

```json
{
  "profileId": "uuid",
  "presenceId": "public-presence-id",
  "instanceToken": "private-runtime-token",
  "expiresAt": "2026-06-01T15:04:05Z"
}
```

`presenceId` is public and may be shown to friends or used as a signaling target. `instanceToken` is private and must be
used for Presence updates and the signaling WebSocket.

Instance token requirements:

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

## Friend Snapshot

```http
GET /v1/friends
Authorization: Bearer <token>
```

Response:

```json
{
  "friends": [
    {
      "profileId": "uuid",
      "name": "PlayerName",
      "source": "netherlink"
    }
  ],
  "incomingRequests": [],
  "outgoingRequests": []
}
```

## Add Friend

```http
POST /v1/friends/requests
Authorization: Bearer <token>
Content-Type: application/json

{
  "name": "PlayerName"
}
```

The backend should resolve the target profile UUID by name. This creates a NetherLink request even if Mojang refuses the
official add operation.

Response:

```json
{
  "result": "SUCCESS",
  "officialSync": "SKIPPED"
}
```

`officialSync` values:

- `SUCCESS`
- `FAILED`
- `SKIPPED`
- `UNSUPPORTED`

## Accept Request

```http
POST /v1/friends/{profileId}/accept
Authorization: Bearer <token>
```

Creates a NetherLink friendship. Optionally attempts official synchronization if supported.

## Decline Or Revoke Request

```http
DELETE /v1/friends/requests/{profileId}
Authorization: Bearer <token>
```

Deletes the pending NetherLink request involving the caller and target.

## Remove Friend

```http
DELETE /v1/friends/{profileId}
Authorization: Bearer <token>
```

Deletes the NetherLink friendship. If official friend removal is still allowed, call the official API as best effort and
report the result.

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

## Clear Presence

```http
DELETE /v1/presence
Authorization: Bearer <instance_token>
```

Sets the Presence associated with this instance token to `OFFLINE` or removes it.

## Friend Presence

```http
GET /v1/friends/presence
Authorization: Bearer <token>
```

Response:

```json
{
  "statuses": [
    {
      "profileId": "uuid",
      "presenceId": "public-presence-id",
      "pmid": "optional-official-pmid",
      "status": "HOSTING",
      "joinable": true,
      "sessionId": "host-session-id",
      "displayText": "1.18.2 Forge - Singleplayer world",
      "updatedAt": "2026-06-01T15:03:35Z",
      "expiresAt": "2026-06-01T15:04:35Z"
    }
  ]
}
```

Only return Presence for profiles that are friends with the caller. If a friend has multiple active `presence_id`
records, return multiple list entries. The client should display those as separate join targets under the same account.
