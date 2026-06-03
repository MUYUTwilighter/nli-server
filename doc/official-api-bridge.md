# Official API Bridge

Official bridge behavior should be best effort, never the only source of truth.

Expected bridge behavior:

- Import official friend list into NetherLink cache when available.
- Remove friend from official API when still allowed.
- Do not block NetherLink add or accept if official add is blocked by official Presence requirements.
- Record official sync result for debugging.

Suggested sync metadata:

```text
official_sync
- profile_id uuid
- target_profile_id uuid
- operation text
- result text
- error text nullable
- updated_at timestamp
```

If the "persistent storage only friend graph and Presence" rule must be interpreted strictly, do not create this table.
Instead, log sync results only.
