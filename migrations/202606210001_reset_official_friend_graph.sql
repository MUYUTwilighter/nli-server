-- Version 1 now treats the official Minecraft friends service as its source of truth.
-- Existing NetherLink-only relationships cannot be promoted without an official request.
TRUNCATE TABLE friend_requests, friendships;
