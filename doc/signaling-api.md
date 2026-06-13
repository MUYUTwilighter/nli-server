# Signaling API

Use WebSocket for signaling.

```text
GET /v1/signaling/ws
Authorization: Bearer <instance_token>
```

Each connected client is associated with the `profile_id` and `presence_id` derived from its instance token. A single
`profile_id` may have multiple simultaneous WebSocket connections, one per running game instance.

Only one connection is retained for each `presenceId`. A newer connection replaces and closes the previous connection.
Text frames are required; binary frames receive a `BAD_REQUEST` error. Ping frames receive Pong responses.

The server sends a Ping every 30 seconds. Any text, binary, Ping, or Pong frame refreshes activity; a connection with no
inbound activity for 90 seconds is closed. Runtime tokens are revalidated during heartbeat checks and before processing
each text frame. Each profile may have at most eight simultaneous Presence connections; an additional connection
receives `CONNECTION_LIMIT` and is closed.

On process shutdown, the server sends Close frames to all locally registered signaling connections before Axum finishes
graceful shutdown. Connection exit then removes the Presence's signaling-session indexes from Redis.

The active connection registry is currently process-local. Deploy signaling as a single server replica until cross-node
routing is implemented with Redis Pub/Sub, Redis Streams, or another shared message bus.

## Envelope

```json
{
  "id": "client-message-id",
  "type": "JOIN_REQUEST",
  "to": "target-profile-uuid",
  "toPresenceId": "target-public-presence-id",
  "sessionId": "webrtc-session-id",
  "payload": {}
}
```

Server forwards to the target as:

```json
{
  "id": "server-message-id",
  "type": "JOIN_REQUEST",
  "from": "sender-profile-uuid",
  "fromPresenceId": "sender-public-presence-id",
  "toPresenceId": "target-public-presence-id",
  "sessionId": "webrtc-session-id",
  "payload": {}
}
```

Message types needed by current NetherLink:

- `JOIN_REQUEST`
- `JOIN_ACCEPTED`
- `JOIN_REJECTED`
- `INVITE_DECLINED`
- `OFFER`
- `ANSWER`
- `ICE_CANDIDATE`

The server should only forward signaling messages when:

- Sender is authenticated.
- Sender and receiver are NetherLink friends.
- Receiver has an active WebSocket connection for `toPresenceId`.
- For `JOIN_REQUEST`, receiver Presence for `toPresenceId` is `HOSTING` and `joinable=true`.
- `toPresenceId` belongs to the `to` profile id.

These checks are implemented for every message. `JOIN_REQUEST` creates the Redis-backed signaling session. The target
host sends `JOIN_ACCEPTED` or `JOIN_REJECTED`; the initiator sends `OFFER`; the host sends `ANSWER`; either side may send
`ICE_CANDIDATE`. `JOIN_REJECTED` and `INVITE_DECLINED` close the signaling session.

## Game Flow

The server enforces this state machine:

```text
PENDING_JOIN --JOIN_ACCEPTED--> ACCEPTED --OFFER--> OFFER_SENT --ANSWER--> ANSWER_SENT
      |                              |
      +--JOIN_REJECTED--> closed     +--INVITE_DECLINED--> closed
```

- Only the host can accept or reject a pending join request.
- Only the joining instance can send the offer; only the host can send the answer.
- `JOIN_REJECTED` is invalid after the host has accepted.
- `INVITE_DECLINED` is invalid after WebRTC negotiation has started.
- ICE candidates are accepted from either peer after the offer exists.
- The host Presence is checked again for `JOIN_ACCEPTED` and `OFFER`. If the world is no longer `HOSTING` and joinable,
  the signaling session is deleted and the sender receives `TARGET_NOT_JOINABLE`.
- A revoked, rotated, or expired instance token prevents further messages on an already-open WebSocket. The server sends
  `INVALID_INSTANCE_TOKEN` and closes that connection.
- Signaling frames are not buffered for reconnect. Disconnecting a WebSocket deletes every signaling session involving
  that Presence. A peer sending afterward receives `SESSION_NOT_FOUND` and must begin again with a new `sessionId` and
  `JOIN_REQUEST`. A target that was never connected returns `TARGET_UNAVAILABLE`.

If the receiver is offline or unavailable, return an error frame to the sender.

Trust boundary:

- Ignore any client-supplied `from` or `fromPresenceId`.
- The forwarded envelope must inject `from` and `fromPresenceId` from the sender's `instanceToken`.
- The server should validate message type, `sessionId`, target profile id, target Presence id, and payload size before
  forwarding.
- A signaling session should be bound to `(fromProfileId, fromPresenceId, toProfileId, toPresenceId, sessionId)`.

Suggested per-session limits:

- One `OFFER`.
- One `ANSWER`.
- 128 ICE candidates per side.
- Session TTL: 5 minutes before WebRTC connection establishment.

Current enforced limits:

- One `OFFER` and one `ANSWER` in offer-before-answer order.
- `OFFER` is rejected until the host has sent `JOIN_ACCEPTED`.
- `ICE_CANDIDATE` is rejected until an `OFFER` exists for the session.
- 128 ICE candidates per side.
- 60 messages per minute for each sender-profile and target-profile pair.
- 128 KiB JSON payload for `OFFER` and `ANSWER`.
- 8 KiB JSON payload for control messages and `ICE_CANDIDATE`.
- 256 KiB maximum WebSocket message size.
- 128 characters for message ids, session ids, and Presence ids; control characters are rejected.

## Error Envelope

```json
{
  "id": "client-message-id",
  "type": "ERROR",
  "code": "TARGET_UNAVAILABLE",
  "message": "Target is not connected to signaling"
}
```

Suggested error codes:

- `UNAUTHORIZED`
- `FORBIDDEN`
- `NOT_FRIENDS`
- `TARGET_UNAVAILABLE`
- `TARGET_NOT_JOINABLE`
- `INSTANCE_NOT_FOUND`
- `SESSION_NOT_FOUND`
- `RATE_LIMITED`
- `BAD_REQUEST`
- `INVALID_SESSION_STATE`
- `INVALID_INSTANCE_TOKEN`
- `CONNECTION_LIMIT`
