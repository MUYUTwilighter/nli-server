# Security

Minimum requirements:

- HTTPS only.
- WebSocket over TLS only.
- Do not log Minecraft access tokens.
- Do not persist Minecraft access tokens.
- Require and revalidate `X-Minecraft-Access-Token` on every friend-list read and mutation, then discard it immediately.
- Do not combine multiple Minecraft access tokens into one bearer value. Multi-account processes must register each
  account independently and discard each Minecraft token after registration.
- Do not expose `instanceToken` to friends or put it in signaling payloads.
- Treat `presenceId` as public and `instanceToken` as private.
- Rate limit auth verification, friend actions, Presence publish, and signaling messages.
- Apply global and client-IP limits before calling Minecraft authentication. Trust `X-Forwarded-For` only when
  `NLI_TRUST_PROXY_HEADERS=true` and the service is reachable exclusively through a trusted reverse proxy.
- Multiple accounts registered by one physical process share its client-IP and global pre-authentication limits; each
  resulting account remains subject to its own profile and instance limits.
- Prevent signaling to non-friends.
- Prevent joining non-joinable Presence.
- Validate all UUIDs and message sizes.
- Cap SDP and ICE payload sizes.
- Never trust caller-supplied source identity fields.
- Signaling source identity must be derived from `instanceToken`.
- TURN credentials must be short-lived.
- Logs must not include Minecraft tokens, instance tokens, TURN credentials, full SDP, or full ICE candidates.
- Set `NLI_METRICS_TOKEN` in production so `GET /metrics` requires `Authorization: Bearer <token>`.
- Production startup must reject weak TURN secrets, non-HTTPS Mojang endpoints, and non-HTTPS CORS origins. Binding to
  loopback is recommended when a trusted reverse proxy runs on the same host.

Recommended limits:

- Runtime instance creation: 10 requests per minute per profile.
- Presence publish: 1 request per 10 seconds per instance token.
- Friend mutation: 10 requests per minute per profile.
- Signaling: 60 messages per minute per profile per peer.
- SDP payload: 128 KiB max.
- ICE payload: 8 KiB max.
- WebSocket connections: 8 active runtime instances per profile by default.
- Signaling sessions: 8 active sessions per runtime instance by default.
