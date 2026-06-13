use std::time::Duration;

use anyhow::{Context, Result, bail};
use chrono::Utc;
use redis::{AsyncCommands, aio::ConnectionManager};
use serde::{Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::model::{
    presence::Presence, runtime_instance::RuntimeInstance, signaling::SignalingSession,
    token::RuntimeTokenHash,
};

const KEY_PREFIX: &str = "nli";

#[derive(Clone)]
pub struct RedisStore {
    connection: ConnectionManager,
}

impl RedisStore {
    pub async fn connect(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).context("invalid Redis URL")?;
        let mut connection = ConnectionManager::new(client)
            .await
            .context("failed to connect to Redis")?;
        redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
            .context("failed to verify Redis connection")?;

        Ok(Self { connection })
    }

    pub async fn health_check(&self) -> Result<()> {
        let mut connection = self.connection.clone();
        let response = redis::cmd("PING")
            .query_async::<String>(&mut connection)
            .await
            .context("Redis health check failed")?;
        if response != "PONG" {
            bail!("Redis health check returned an unexpected response");
        }
        Ok(())
    }

    pub async fn put_runtime_instance(
        &self,
        token_hash: &RuntimeTokenHash,
        instance: &RuntimeInstance,
        ttl: Duration,
    ) -> Result<()> {
        ensure_ttl(ttl)?;
        let mut connection = self.connection.clone();
        let reverse_key = presence_instance_key(&instance.presence_id);
        let previous_hash: Option<String> = connection.get(&reverse_key).await?;

        if let Some(previous_hash) = previous_hash
            && previous_hash != token_hash.as_str()
        {
            let _: usize = connection.del(instance_key(&previous_hash)).await?;
        }

        let payload = serialize(instance)?;
        let mut pipeline = redis::pipe();
        pipeline
            .atomic()
            .set_ex(instance_key(token_hash.as_str()), payload, ttl.as_secs())
            .ignore()
            .set_ex(reverse_key, token_hash.as_str(), ttl.as_secs())
            .ignore()
            .zadd(
                profile_instances_key(instance.profile_id),
                &instance.presence_id,
                instance.expires_at.timestamp(),
            )
            .ignore()
            .expire(
                profile_instances_key(instance.profile_id),
                seconds_as_i64(ttl.as_secs().saturating_add(60))?,
            )
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(())
    }

    pub async fn register_runtime_instance(
        &self,
        token_hash: &RuntimeTokenHash,
        instance: &RuntimeInstance,
        ttl: Duration,
        max_instances: usize,
    ) -> Result<bool> {
        ensure_ttl(ttl)?;
        let mut connection = self.connection.clone();
        let script = redis::Script::new(
            r#"
            redis.call('ZREMRANGEBYSCORE', KEYS[3], '-inf', ARGV[5])
            if redis.call('ZCARD', KEYS[3]) >= tonumber(ARGV[7]) then
                return 0
            end
            local previous = redis.call('GET', KEYS[2])
            if previous and previous ~= ARGV[2] then
                redis.call('DEL', ARGV[8] .. previous)
            end
            redis.call('SET', KEYS[1], ARGV[1], 'EX', ARGV[3])
            redis.call('SET', KEYS[2], ARGV[2], 'EX', ARGV[3])
            redis.call('ZADD', KEYS[3], ARGV[4], ARGV[6])
            redis.call('EXPIRE', KEYS[3], ARGV[9])
            return 1
            "#,
        );
        let registered = script
            .key(instance_key(token_hash.as_str()))
            .key(presence_instance_key(&instance.presence_id))
            .key(profile_instances_key(instance.profile_id))
            .arg(serialize(instance)?)
            .arg(token_hash.as_str())
            .arg(ttl.as_secs())
            .arg(instance.expires_at.timestamp())
            .arg(Utc::now().timestamp())
            .arg(&instance.presence_id)
            .arg(max_instances)
            .arg(format!("{KEY_PREFIX}:instance:"))
            .arg(ttl.as_secs().saturating_add(60))
            .invoke_async::<u8>(&mut connection)
            .await?;
        Ok(registered == 1)
    }

    pub async fn runtime_instance(
        &self,
        token_hash: &RuntimeTokenHash,
    ) -> Result<Option<RuntimeInstance>> {
        self.get_json(&instance_key(token_hash.as_str())).await
    }

    pub async fn rotate_runtime_instance(
        &self,
        old_token_hash: &RuntimeTokenHash,
        new_token_hash: &RuntimeTokenHash,
        instance: &RuntimeInstance,
        ttl: Duration,
    ) -> Result<bool> {
        ensure_ttl(ttl)?;
        let mut connection = self.connection.clone();
        let script = redis::Script::new(
            r#"
            local current = redis.call('GET', KEYS[1])
            if current ~= ARGV[1] then
                return 0
            end
            redis.call('SET', KEYS[2], ARGV[3], 'EX', ARGV[4])
            redis.call('SET', KEYS[1], ARGV[2], 'EX', ARGV[4])
            redis.call('ZADD', KEYS[4], ARGV[5], ARGV[6])
            redis.call('EXPIRE', KEYS[4], ARGV[7])
            redis.call('DEL', KEYS[3])
            return 1
            "#,
        );
        let rotated = script
            .key(presence_instance_key(&instance.presence_id))
            .key(instance_key(new_token_hash.as_str()))
            .key(instance_key(old_token_hash.as_str()))
            .key(profile_instances_key(instance.profile_id))
            .arg(old_token_hash.as_str())
            .arg(new_token_hash.as_str())
            .arg(serialize(instance)?)
            .arg(ttl.as_secs())
            .arg(instance.expires_at.timestamp())
            .arg(&instance.presence_id)
            .arg(ttl.as_secs().saturating_add(60))
            .invoke_async::<u8>(&mut connection)
            .await?;
        Ok(rotated == 1)
    }

    pub async fn delete_runtime_instance(&self, token_hash: &RuntimeTokenHash) -> Result<bool> {
        let Some(instance) = self.runtime_instance(token_hash).await? else {
            return Ok(false);
        };
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        pipeline
            .atomic()
            .del(instance_key(token_hash.as_str()))
            .ignore()
            .del(presence_instance_key(&instance.presence_id))
            .ignore()
            .zrem(
                profile_instances_key(instance.profile_id),
                &instance.presence_id,
            )
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(true)
    }

    pub async fn close_runtime_instance(
        &self,
        token_hash: &RuntimeTokenHash,
        instance: &RuntimeInstance,
    ) -> Result<bool> {
        let mut connection = self.connection.clone();
        let script = redis::Script::new(
            r#"
            local current = redis.call('GET', KEYS[2])
            if current ~= ARGV[1] then
                return 0
            end
            redis.call('DEL', KEYS[1], KEYS[2], KEYS[3])
            redis.call('ZREM', KEYS[4], ARGV[2])
            redis.call('ZREM', KEYS[5], ARGV[2])
            return 1
            "#,
        );
        let deleted = script
            .key(instance_key(token_hash.as_str()))
            .key(presence_instance_key(&instance.presence_id))
            .key(presence_key(&instance.presence_id))
            .key(profile_presences_key(instance.profile_id))
            .key(profile_instances_key(instance.profile_id))
            .arg(token_hash.as_str())
            .arg(&instance.presence_id)
            .invoke_async::<u8>(&mut connection)
            .await?;
        Ok(deleted == 1)
    }

    pub async fn put_presence(&self, presence: &Presence, ttl: Duration) -> Result<Presence> {
        ensure_ttl(ttl)?;
        let now = Utc::now();
        let mut stored_presence = presence.clone();
        stored_presence.updated_at = now;
        stored_presence.expires_at = now
            + chrono::Duration::from_std(ttl).context("Presence TTL exceeds supported range")?;
        let payload = serialize(&stored_presence)?;
        let key = presence_key(&stored_presence.presence_id);
        let index_key = profile_presences_key(stored_presence.profile_id);
        let expires_at = stored_presence.expires_at.timestamp();
        let index_ttl = ttl.as_secs().saturating_add(60);
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        pipeline
            .atomic()
            .set_ex(key, payload, ttl.as_secs())
            .ignore()
            .zadd(index_key.clone(), &stored_presence.presence_id, expires_at)
            .ignore()
            .expire(index_key, seconds_as_i64(index_ttl)?)
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(stored_presence)
    }

    pub async fn presence(&self, presence_id: &str) -> Result<Option<Presence>> {
        self.get_json(&presence_key(presence_id)).await
    }

    pub async fn presences_for_profile(&self, profile_id: Uuid) -> Result<Vec<Presence>> {
        let index_key = profile_presences_key(profile_id);
        let now = Utc::now().timestamp();
        let mut connection = self.connection.clone();
        redis::cmd("ZREMRANGEBYSCORE")
            .arg(&index_key)
            .arg("-inf")
            .arg(now)
            .query_async::<usize>(&mut connection)
            .await?;
        let presence_ids = redis::cmd("ZRANGEBYSCORE")
            .arg(&index_key)
            .arg(now + 1)
            .arg("+inf")
            .query_async::<Vec<String>>(&mut connection)
            .await?;

        let mut presences = Vec::with_capacity(presence_ids.len());
        for presence_id in presence_ids {
            if let Some(presence) = self.presence(&presence_id).await? {
                presences.push(presence);
            } else {
                let _: usize = connection.zrem(&index_key, presence_id).await?;
            }
        }
        Ok(presences)
    }

    pub async fn delete_presence(&self, presence_id: &str) -> Result<bool> {
        let Some(presence) = self.presence(presence_id).await? else {
            return Ok(false);
        };
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        pipeline
            .atomic()
            .del(presence_key(presence_id))
            .ignore()
            .zrem(profile_presences_key(presence.profile_id), presence_id)
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(true)
    }

    pub async fn put_signaling_session(
        &self,
        session: &SignalingSession,
        ttl: Duration,
    ) -> Result<()> {
        ensure_ttl(ttl)?;
        let payload = serialize(session)?;
        let index_ttl = ttl.as_secs().saturating_add(60);
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        pipeline
            .atomic()
            .set_ex(
                signaling_session_key(&session.session_id),
                payload,
                ttl.as_secs(),
            )
            .ignore()
            .sadd(
                presence_signaling_sessions_key(&session.initiator.presence_id),
                &session.session_id,
            )
            .ignore()
            .expire(
                presence_signaling_sessions_key(&session.initiator.presence_id),
                seconds_as_i64(index_ttl)?,
            )
            .ignore()
            .sadd(
                presence_signaling_sessions_key(&session.target.presence_id),
                &session.session_id,
            )
            .ignore()
            .expire(
                presence_signaling_sessions_key(&session.target.presence_id),
                seconds_as_i64(index_ttl)?,
            )
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(())
    }

    pub async fn signaling_session(&self, session_id: &str) -> Result<Option<SignalingSession>> {
        self.get_json(&signaling_session_key(session_id)).await
    }

    pub async fn delete_signaling_session(&self, session_id: &str) -> Result<bool> {
        let session = self.signaling_session(session_id).await?;
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        pipeline
            .atomic()
            .del(signaling_session_key(session_id))
            .ignore();
        if let Some(session) = session {
            pipeline
                .srem(
                    presence_signaling_sessions_key(&session.initiator.presence_id),
                    session_id,
                )
                .ignore()
                .srem(
                    presence_signaling_sessions_key(&session.target.presence_id),
                    session_id,
                )
                .ignore();
        }
        let deleted: usize = connection.exists(signaling_session_key(session_id)).await?;
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(deleted > 0)
    }

    pub async fn delete_signaling_sessions_for_presence(&self, presence_id: &str) -> Result<usize> {
        let index_key = presence_signaling_sessions_key(presence_id);
        let mut connection = self.connection.clone();
        let session_ids: Vec<String> = connection.smembers(&index_key).await?;
        let mut deleted = 0;
        for session_id in session_ids {
            if self.delete_signaling_session(&session_id).await? {
                deleted += 1;
            }
        }
        let _: usize = connection.del(index_key).await?;
        Ok(deleted)
    }

    pub async fn put_nonce(&self, namespace: &str, nonce_hash: &str, ttl: Duration) -> Result<()> {
        ensure_ttl(ttl)?;
        let mut connection = self.connection.clone();
        connection
            .set_ex::<_, _, ()>(nonce_key(namespace, nonce_hash), "1", ttl.as_secs())
            .await?;
        Ok(())
    }

    pub async fn consume_nonce(&self, namespace: &str, nonce_hash: &str) -> Result<bool> {
        let mut connection = self.connection.clone();
        let value = redis::cmd("GETDEL")
            .arg(nonce_key(namespace, nonce_hash))
            .query_async::<Option<String>>(&mut connection)
            .await?;
        Ok(value.is_some())
    }

    pub async fn increment_rate_limit(&self, bucket: &str, window: Duration) -> Result<u64> {
        ensure_ttl(window)?;
        let key = rate_limit_key(bucket);
        let mut connection = self.connection.clone();
        let script = redis::Script::new(
            r#"
            local count = redis.call('INCR', KEYS[1])
            if count == 1 then
                redis.call('EXPIRE', KEYS[1], ARGV[1])
            end
            return count
            "#,
        );
        Ok(script
            .key(key)
            .arg(seconds_as_i64(window.as_secs())?)
            .invoke_async::<u64>(&mut connection)
            .await?)
    }

    pub async fn cache_profile(&self, profile_id: Uuid, name: &str, ttl: Duration) -> Result<()> {
        ensure_ttl(ttl)?;
        let profile = CachedProfile {
            profile_id,
            name: name.to_owned(),
        };
        let payload = serialize(&profile)?;
        let mut connection = self.connection.clone();
        let mut pipeline = redis::pipe();
        pipeline
            .atomic()
            .set_ex(profile_by_id_key(profile_id), &payload, ttl.as_secs())
            .ignore()
            .set_ex(profile_by_name_key(name), payload, ttl.as_secs())
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(())
    }

    pub async fn cached_profile_by_id(&self, profile_id: Uuid) -> Result<Option<(Uuid, String)>> {
        Ok(self
            .get_json::<CachedProfile>(&profile_by_id_key(profile_id))
            .await?
            .map(|profile| (profile.profile_id, profile.name)))
    }

    pub async fn cached_profile_by_name(&self, name: &str) -> Result<Option<(Uuid, String)>> {
        Ok(self
            .get_json::<CachedProfile>(&profile_by_name_key(name))
            .await?
            .map(|profile| (profile.profile_id, profile.name)))
    }

    async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut connection = self.connection.clone();
        let payload: Option<String> = connection.get(key).await?;
        payload.map(|payload| deserialize(&payload)).transpose()
    }
}

#[derive(Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CachedProfile {
    profile_id: Uuid,
    name: String,
}

fn serialize<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).context("failed to serialize Redis value")
}

fn deserialize<T: DeserializeOwned>(payload: &str) -> Result<T> {
    serde_json::from_str(payload).context("failed to deserialize Redis value")
}

fn ensure_ttl(ttl: Duration) -> Result<()> {
    if ttl.is_zero() {
        bail!("Redis TTL must be greater than zero");
    }
    Ok(())
}

fn seconds_as_i64(seconds: u64) -> Result<i64> {
    i64::try_from(seconds).context("Redis TTL exceeds supported range")
}

fn instance_key(token_hash: &str) -> String {
    format!("{KEY_PREFIX}:instance:{token_hash}")
}

fn presence_instance_key(presence_id: &str) -> String {
    format!("{KEY_PREFIX}:presence-instance:{presence_id}")
}

fn presence_key(presence_id: &str) -> String {
    format!("{KEY_PREFIX}:presence:{presence_id}")
}

fn profile_presences_key(profile_id: Uuid) -> String {
    format!("{KEY_PREFIX}:profile-presences:{profile_id}")
}

fn profile_instances_key(profile_id: Uuid) -> String {
    format!("{KEY_PREFIX}:profile-instances:{profile_id}")
}

fn profile_by_id_key(profile_id: Uuid) -> String {
    format!("{KEY_PREFIX}:profile:id:{profile_id}")
}

fn profile_by_name_key(name: &str) -> String {
    format!("{KEY_PREFIX}:profile:name:{}", name.to_ascii_lowercase())
}

fn signaling_session_key(session_id: &str) -> String {
    format!("{KEY_PREFIX}:signaling-session:{session_id}")
}

fn presence_signaling_sessions_key(presence_id: &str) -> String {
    format!("{KEY_PREFIX}:presence-signaling-sessions:{presence_id}")
}

fn nonce_key(namespace: &str, nonce_hash: &str) -> String {
    format!("{KEY_PREFIX}:nonce:{namespace}:{nonce_hash}")
}

fn rate_limit_key(bucket: &str) -> String {
    format!("{KEY_PREFIX}:rate-limit:{bucket}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_namespaced_keys() {
        assert_eq!(instance_key("hash"), "nli:instance:hash");
        assert_eq!(presence_key("id"), "nli:presence:id");
        assert_eq!(
            profile_instances_key(Uuid::nil()),
            "nli:profile-instances:00000000-0000-0000-0000-000000000000"
        );
        assert_eq!(
            signaling_session_key("session"),
            "nli:signaling-session:session"
        );
    }

    #[test]
    fn rejects_zero_ttl() {
        assert!(ensure_ttl(Duration::ZERO).is_err());
    }
}
