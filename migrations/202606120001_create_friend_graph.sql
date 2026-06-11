CREATE TABLE friendships (
    profile_low UUID NOT NULL,
    profile_high UUID NOT NULL,
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (profile_low, profile_high),
    CONSTRAINT friendships_ordered_pair CHECK (profile_low < profile_high),
    CONSTRAINT friendships_source_valid CHECK (
        source IN ('netherlink', 'minecraft_import', 'minecraft_sync')
    )
);

CREATE TABLE friend_requests (
    requester_profile_id UUID NOT NULL,
    target_profile_id UUID NOT NULL,
    source TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (requester_profile_id, target_profile_id),
    CONSTRAINT friend_requests_not_self CHECK (requester_profile_id <> target_profile_id),
    CONSTRAINT friend_requests_source_valid CHECK (
        source IN ('netherlink', 'minecraft_import', 'minecraft_sync')
    )
);

CREATE UNIQUE INDEX friend_requests_unordered_pair_unique
    ON friend_requests (
        LEAST(requester_profile_id, target_profile_id),
        GREATEST(requester_profile_id, target_profile_id)
    );

CREATE INDEX friend_requests_target_idx
    ON friend_requests (target_profile_id, created_at);

CREATE INDEX friendships_profile_high_idx
    ON friendships (profile_high);
