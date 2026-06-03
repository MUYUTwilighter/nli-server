# TURN Auth

The backend should provide TURN configuration for WebRTC.

```http
POST /v1/turn
Authorization: Bearer <instance_token>
```

Response:

```json
{
  "urls": [
    "stun:example.com:3478",
    "turn:example.com:3478?transport=udp",
    "turn:example.com:3478?transport=tcp"
  ],
  "username": "temporary-user",
  "credential": "temporary-password",
  "expiresAt": "2026-06-01T15:10:00Z"
}
```

TURN credential requirements:

- Require a valid `instanceToken`.
- Prefer coturn time-limited REST API credentials.
- Credential TTL should be short, for example 5 to 10 minutes.
- Do not return long-lived TURN passwords.
- Rate limit TURN credential requests per profile and per runtime instance.
- If possible, allocate TURN regions based on client preference or latency, but keep the selected relay node configurable.
