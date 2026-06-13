# TURN Auth

The backend provides coturn REST API compatible temporary credentials for WebRTC.

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

- A valid `instanceToken` is required.
- `username` is `{unix-expiry}:{profileId}`.
- `credential` is `Base64(HMAC-SHA1(TURN_SHARED_SECRET, username))`, compatible with coturn
  `use-auth-secret` and `static-auth-secret`.
- Credential TTL is configured by `TURN_CREDENTIAL_TTL_SECONDS` and must be 60 to 3600 seconds.
- Requests are limited to 10 per minute per runtime instance and 20 per minute per profile.
- Credentials, instance tokens, and the shared secret are never logged or persisted.
- If possible, allocate TURN regions based on client preference or latency, but keep the selected relay node configurable.

Required server configuration:

```dotenv
TURN_URLS=stun:turn.example.com:3478,turn:turn.example.com:3478?transport=udp,turn:turn.example.com:3478?transport=tcp
TURN_SHARED_SECRET=replace-with-the-same-secret-used-by-coturn
TURN_CREDENTIAL_TTL_SECONDS=600
```

Corresponding coturn settings include:

```text
use-auth-secret
static-auth-secret=replace-with-the-same-secret-used-by-coturn
realm=turn.example.com
```
