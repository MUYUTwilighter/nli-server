use std::{collections::HashMap, sync::Arc};

use axum::extract::ws::Message;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct SignalingConnections {
    inner: Arc<RwLock<HashMap<String, Connection>>>,
}

#[derive(Clone)]
struct Connection {
    id: Uuid,
    profile_id: Uuid,
    sender: mpsc::Sender<Message>,
}

pub struct RegisteredConnection {
    pub id: Uuid,
    pub receiver: mpsc::Receiver<Message>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegisterError {
    ProfileConnectionLimit,
}

const MAX_CONNECTIONS_PER_PROFILE: usize = 8;

impl SignalingConnections {
    pub async fn register(
        &self,
        profile_id: Uuid,
        presence_id: String,
    ) -> Result<RegisteredConnection, RegisterError> {
        let (sender, receiver) = mpsc::channel(64);
        let id = Uuid::new_v4();
        let mut connections = self.inner.write().await;
        let replacing = connections.contains_key(&presence_id);
        let profile_connections = connections
            .values()
            .filter(|connection| connection.profile_id == profile_id)
            .count();
        if !replacing && profile_connections >= MAX_CONNECTIONS_PER_PROFILE {
            return Err(RegisterError::ProfileConnectionLimit);
        }
        let previous = connections.insert(
            presence_id,
            Connection {
                id,
                profile_id,
                sender,
            },
        );
        drop(connections);
        if let Some(previous) = previous {
            let _ = previous.sender.try_send(Message::Close(None));
        }
        Ok(RegisteredConnection { id, receiver })
    }

    pub async fn unregister(&self, presence_id: &str, connection_id: Uuid) {
        let mut connections = self.inner.write().await;
        if connections
            .get(presence_id)
            .is_some_and(|connection| connection.id == connection_id)
        {
            connections.remove(presence_id);
        }
    }

    pub async fn send(&self, presence_id: &str, message: Message) -> bool {
        let sender = self
            .inner
            .read()
            .await
            .get(presence_id)
            .map(|connection| connection.sender.clone());
        let Some(sender) = sender else {
            return false;
        };
        sender.try_send(message).is_ok()
    }

    pub async fn close(&self, presence_id: &str) -> bool {
        let connection = self.inner.write().await.remove(presence_id);
        let Some(connection) = connection else {
            return false;
        };
        let _ = connection.sender.try_send(Message::Close(None));
        true
    }

    pub async fn close_all(&self) -> usize {
        let connections = std::mem::take(&mut *self.inner.write().await);
        let count = connections.len();
        for connection in connections.into_values() {
            let _ = connection.sender.try_send(Message::Close(None));
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn old_connection_cannot_unregister_replacement() {
        let connections = SignalingConnections::default();
        let profile_id = Uuid::new_v4();
        let first = connections
            .register(profile_id, "presence".to_owned())
            .await
            .unwrap();
        let mut second = connections
            .register(profile_id, "presence".to_owned())
            .await
            .unwrap();

        connections.unregister("presence", first.id).await;
        assert!(
            connections
                .send("presence", Message::Text("hello".into()))
                .await
        );
        assert_eq!(
            second.receiver.recv().await,
            Some(Message::Text("hello".into()))
        );
    }

    #[tokio::test]
    async fn limits_connections_per_profile() {
        let connections = SignalingConnections::default();
        let profile_id = Uuid::new_v4();
        for index in 0..MAX_CONNECTIONS_PER_PROFILE {
            connections
                .register(profile_id, format!("presence-{index}"))
                .await
                .unwrap();
        }
        assert_eq!(
            connections
                .register(profile_id, "presence-over-limit".to_owned())
                .await
                .err(),
            Some(RegisterError::ProfileConnectionLimit)
        );
    }

    #[tokio::test]
    async fn closes_all_connections_for_shutdown() {
        let connections = SignalingConnections::default();
        let profile_id = Uuid::new_v4();
        let mut first = connections
            .register(profile_id, "first".to_owned())
            .await
            .unwrap();
        let mut second = connections
            .register(profile_id, "second".to_owned())
            .await
            .unwrap();

        assert_eq!(connections.close_all().await, 2);
        assert_eq!(first.receiver.recv().await, Some(Message::Close(None)));
        assert_eq!(second.receiver.recv().await, Some(Message::Close(None)));
    }
}
