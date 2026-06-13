# REST API Draft

The client submits its Minecraft access token only when creating a runtime instance. Every subsequent authenticated
endpoint derives the caller profile and Presence from the returned `instanceToken`.

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
`status=ONLINE`, and returns the verified identity plus a private token for this game process.

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

The backend resolves display names from the Minecraft profile service. `name` is `null` when a stored profile no longer
resolves, while the UUID and relationship remain available.

## Add Friend

```http
POST /v1/friends/requests
Authorization: Bearer <instance_token>
Content-Type: application/json

{
  "name": "PlayerName"
}
```

The backend resolves the target profile UUID by name. Names must contain 3 to 16 ASCII letters, digits, or underscores.
This creates a NetherLink request without attempting an official friend operation. Sending a reciprocal request accepts
the pending request immediately.

Response:

```json
{
  "result": "SUCCESS",
  "relationship": "REQUESTED",
  "officialSync": "SKIPPED"
}
```

`relationship` is `REQUESTED` for a newly pending request and `ACCEPTED` when a reciprocal request completes the
friendship. Requests targeting the caller are rejected, and adding an existing friend returns `409 Conflict`.

`officialSync` values:

- `SUCCESS`
- `FAILED`
- `SKIPPED`
- `UNSUPPORTED`

## Accept Request

```http
POST /v1/friends/{profileId}/accept
Authorization: Bearer <instance_token>
```

Creates a NetherLink friendship from an incoming request. A missing incoming request returns `404 Not Found`.

Response:

```json
{
  "result": "SUCCESS",
  "relationship": "ACCEPTED",
  "officialSync": "SKIPPED"
}
```

## Decline Or Revoke Request

```http
DELETE /v1/friends/requests/{profileId}
Authorization: Bearer <instance_token>
```

Deletes the pending NetherLink request involving the caller and target. Returns `204 No Content` and is idempotent.

## Remove Friend

```http
DELETE /v1/friends/{profileId}
Authorization: Bearer <instance_token>
```

Deletes the NetherLink friendship. Returns `204 No Content` and is idempotent. Official friend synchronization is not
currently attempted.

Friend request, acceptance, decline, revoke, and removal operations share a limit of 10 attempts per caller per minute.

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

## Friend Presence

```http
GET /v1/friends/presence
Authorization: Bearer <instance_token>
```

Response:

```json
{
  "statuses": [
    {
      "profileId": "uuid",
      "presenceId": "public-presence-id",
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
The caller's own Presence and pending friend requests are not included. Expired entries are removed from the Redis
profile index while the snapshot is assembled.
