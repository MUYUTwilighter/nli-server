# Signaling API Draft

Use WebSocket for signaling.

```text
GET /v1/signaling/ws
Authorization: Bearer <instance_token>
```

Each connected client is associated with the `profile_id` and `presence_id` derived from its instance token. A single
`profile_id` may have multiple simultaneous WebSocket connections, one per running game instance.

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
