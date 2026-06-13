# Identity

Primary player identity should be the Minecraft profile UUID.

Recommended account identity fields:

- `profile_id`: UUID, canonical Minecraft profile UUID.
- `name`: latest known player name, cacheable and replaceable.

The client authenticates during runtime instance registration by presenting its Minecraft access token. The backend
validates that token against Minecraft services and resolves the caller's profile UUID. After registration, HTTP and
WebSocket operations use the issued `instance_token`, which is bound to the resolved `profile_id`. A Minecraft token may
be supplied once more only for best-effort official friend deletion; it is revalidated and immediately discarded.

The runtime instance token is short-lived, stored only as a hash in Redis, and revocable by rotation, expiry, or active
instance closure. It is not a persistent account login session.

## Runtime Instance Identity

NetherLink uses a server-issued runtime instance token instead of depending on a client-selected instance id.

`profile_id` identifies the Minecraft account. `instance_token` identifies and authorizes one currently running
authenticated game instance for that account.

This matters because Java Edition users may run multiple game instances at the same time, possibly with different
Minecraft versions, loaders, modpacks, worlds, or server targets. Presence must therefore be multi-instance. A single
account can publish several simultaneous Presence records, one per server-issued instance token.

A profile may hold at most five active runtime instance tokens. Registration and slot allocation are atomic in Redis;
expired and actively closed instances release their slots.

When a game process starts, it should request a new instance token from NLI server using its Minecraft token. The backend
validates the Minecraft identity, generates a fresh instance token, and immediately creates an `ONLINE` Presence record
for that runtime instance. The Minecraft token is discarded after validation.

Recommended runtime identity fields:

- `profile_id`: UUID, canonical Minecraft profile UUID.
- `presence_id`: UUID or opaque string, public id for this runtime Presence entry.
- `instance_token`: private bearer token for this runtime instance.
- `instance_started_at`: timestamp supplied by the client or assigned by the server.

The backend should route signaling to a specific `presence_id`, not only to a `profile_id`. Only the holder of the
matching `instance_token` may modify that Presence or open the WebSocket for that runtime instance.
