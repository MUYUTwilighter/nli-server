ALTER TABLE friendships
    DROP CONSTRAINT friendships_source_valid,
    ADD CONSTRAINT friendships_source_valid CHECK (source = 'minecraft_sync');

ALTER TABLE friend_requests
    DROP CONSTRAINT friend_requests_source_valid,
    ADD CONSTRAINT friend_requests_source_valid CHECK (source = 'minecraft_sync');
