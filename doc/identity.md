# Identity

Primary player identity should be the Minecraft profile UUID.

Recommended account identity fields:

- `profile_id`: UUID, canonical Minecraft profile UUID.
- `name`: latest known player name, cacheable and replaceable.

The client will authenticate by presenting its Minecraft access token. The backend should validate that token against
Minecraft services and resolve the caller's profile UUID. After validation, the backend should use `profile_id`
internally.

If a short-lived NetherLink session token is used, it should be memory-only or statelessly signed, short TTL, and
revocable by expiry. It should not be stored as a login table.

## Runtime Instance Identity

NetherLink should use a server-issued runtime instance token instead of depending on Mojang `pmid` or a client-selected
instance id.

`profile_id` identifies the Minecraft account. `instance_token` identifies and authorizes one currently running
authenticated game instance for that account.

This matters because Java Edition users may run multiple game instances at the same time, possibly with different
Minecraft versions, loaders, modpacks, worlds, or server targets. Presence must therefore be multi-instance. A single
account can publish several simultaneous Presence records, one per server-issued instance token.

When a game process starts, it should request a new instance token from NLI server using its Minecraft token. The backend
validates the Minecraft identity, generates a fresh instance token, and immediately creates an `ONLINE` Presence record
for that runtime instance. The Minecraft token is discarded after validation.

Recommended runtime identity fields:

- `profile_id`: UUID, canonical Minecraft profile UUID.
- `presence_id`: UUID or opaque string, public id for this runtime Presence entry.
- `instance_token`: private bearer token for this runtime instance.
- `pmid`: optional UUID or string, kept only as an official compatibility/debug field.
- `instance_started_at`: timestamp supplied by the client or assigned by the server.

The backend should route signaling to a specific `presence_id`, not only to a `profile_id`. Only the holder of the
matching `instance_token` may modify that Presence or open the WebSocket for that runtime instance.
