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
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(())
    }

    pub async fn runtime_instance(
        &self,
        token_hash: &RuntimeTokenHash,
    ) -> Result<Option<RuntimeInstance>> {
        self.get_json(&instance_key(token_hash.as_str())).await
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
            .ignore();
        pipeline.query_async::<()>(&mut connection).await?;
        Ok(true)
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
        self.set_json(&signaling_session_key(&session.session_id), session, ttl)
            .await
    }

    pub async fn signaling_session(&self, session_id: &str) -> Result<Option<SignalingSession>> {
        self.get_json(&signaling_session_key(session_id)).await
    }

    pub async fn delete_signaling_session(&self, session_id: &str) -> Result<bool> {
        let mut connection = self.connection.clone();
        let deleted: usize = connection.del(signaling_session_key(session_id)).await?;
        Ok(deleted > 0)
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

    async fn set_json<T: Serialize>(&self, key: &str, value: &T, ttl: Duration) -> Result<()> {
        ensure_ttl(ttl)?;
        let mut connection = self.connection.clone();
        connection
            .set_ex::<_, _, ()>(key, serialize(value)?, ttl.as_secs())
            .await?;
        Ok(())
    }

    async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let mut connection = self.connection.clone();
        let payload: Option<String> = connection.get(key).await?;
        payload.map(|payload| deserialize(&payload)).transpose()
    }
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

fn signaling_session_key(session_id: &str) -> String {
    format!("{KEY_PREFIX}:signaling-session:{session_id}")
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
            signaling_session_key("session"),
            "nli:signaling-session:session"
        );
    }

    #[test]
    fn rejects_zero_ttl() {
        assert!(ensure_ttl(Duration::ZERO).is_err());
    }
}
