//! Redis pub/sub helpers.
//!
//! Provides publish and subscribe functionality for inter-process communication.

use super::{CacheError, RedisClient};
use fred::prelude::*;

/// A message received from a Redis channel.
#[derive(Debug, Clone)]
pub struct PubSubMessage {
    pub channel: String,
    pub payload: String,
}

/// Shared state for the lazily-initialized Redis subscriber.
///
/// One Redis client and one background task are shared by every subscriber
/// returned from [`RedisClient::subscribe`]. The `ready` watch channel signals
/// when the subscriber has finished connecting (or failed to connect) so the
/// first `subscribe` call can surface connection errors.
pub(crate) struct SharedSubscriber {
    sender: tokio::sync::broadcast::Sender<PubSubMessage>,
    ready: tokio::sync::watch::Receiver<Option<Result<(), String>>>,
}

/// Run the shared Redis subscriber in a background task.
///
/// Connects a dedicated subscriber client, subscribes to `full_channel`, then
/// forwards every incoming message to the broadcast sender until the stream
/// ends. `ready_tx` is set once the initial connection succeeds or fails.
async fn run_subscriber(
    subscriber_client: Client,
    full_channel: String,
    tx: tokio::sync::broadcast::Sender<PubSubMessage>,
    ready_tx: &tokio::sync::watch::Sender<Option<Result<(), String>>>,
) -> Result<(), CacheError> {
    subscriber_client
        .init()
        .await
        .map_err(CacheError::CommandFailed)?;
    subscriber_client
        .subscribe(&full_channel)
        .await
        .map_err(CacheError::CommandFailed)?;
    let _ = ready_tx.send(Some(Ok(())));

    let mut message_rx = subscriber_client.message_rx();
    loop {
        match message_rx.recv().await {
            Ok(message) => {
                let channel_str = message.channel.to_string();
                let payload = match message.value.clone().convert::<String>() {
                    Ok(s) => s,
                    Err(_) => {
                        // Fallback: try as lossy UTF-8
                        match message.value.as_str_lossy() {
                            Some(cow) => cow.to_string(),
                            None => continue, // skip messages that can't be converted
                        }
                    }
                };
                let msg = PubSubMessage {
                    channel: channel_str,
                    payload,
                };
                if tx.send(msg).is_err() {
                    tracing::debug!("pubsub broadcast has no receivers");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "pubsub message receive error");
                break;
            }
        }
    }
    let _ = subscriber_client.quit().await;
    Ok(())
}

impl RedisClient {
    /// Publish a message to a channel.
    pub async fn publish(&self, channel: &str, message: &str) -> Result<(), CacheError> {
        let full_channel = self.make_key("pubsub", channel);
        self.client
            .publish::<(), _, _>(&full_channel, message)
            .await
            .map_err(CacheError::CommandFailed)?;
        Ok(())
    }

    /// Subscribe to a channel and return messages as a broadcast receiver.
    ///
    /// The first call lazily spawns ONE background task with a dedicated Redis
    /// subscriber connection. Subsequent calls reuse that subscriber and simply
    /// return a new [`tokio::sync::broadcast::Receiver`] backed by the same
    /// internal broadcast channel, so all subscribers share a single Redis
    /// connection and a single background task.
    pub async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<tokio::sync::broadcast::Receiver<PubSubMessage>, CacheError> {
        let full_channel = self.make_key("pubsub", channel);

        let shared = self.pubsub_subscriber.get_or_init(|| {
            let (tx, _) = tokio::sync::broadcast::channel(256);
            let (ready_tx, ready_rx) = tokio::sync::watch::channel(None);
            let subscriber_client = self.client.clone_new();
            let task_tx = tx.clone();
            tokio::spawn(async move {
                let result =
                    run_subscriber(subscriber_client, full_channel, task_tx, &ready_tx).await;
                let _ = ready_tx.send(Some(result.map_err(|e| e.to_string())));
            });
            SharedSubscriber {
                sender: tx,
                ready: ready_rx,
            }
        });

        let mut ready = shared.ready.clone();
        if ready.borrow().is_none() {
            let _ = ready.changed().await;
        }
        match ready.borrow().as_ref() {
            Some(Ok(())) => Ok(shared.sender.subscribe()),
            Some(Err(e)) => Err(CacheError::CommandFailed(fred::error::Error::new(
                fred::error::ErrorKind::IO,
                e.clone(),
            ))),
            None => Err(CacheError::ConnectionFailed(
                "pubsub subscriber did not start".into(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_util::TEST_NAMESPACE;

    #[tokio::test]
    async fn integration_publish_subscribe_roundtrip() {
        let Some(client) = super::super::test_util::test_client(
            &super::super::test_util::require_redis_url(),
            TEST_NAMESPACE,
        )
        .await
        else {
            return;
        };
        let mut rx = client.subscribe("test-channel").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        client
            .publish("test-channel", "hello pubsub")
            .await
            .unwrap();
        let msg = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for message")
            .expect("receive error");
        assert!(msg.channel.ends_with("test-channel"));
        assert_eq!(msg.payload, "hello pubsub");
    }

    #[tokio::test]
    async fn integration_publish_multiple_messages() {
        let Some(client) = super::super::test_util::test_client(
            &super::super::test_util::require_redis_url(),
            TEST_NAMESPACE,
        )
        .await
        else {
            return;
        };
        let mut rx = client.subscribe("multi-channel").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        client.publish("multi-channel", "msg1").await.unwrap();
        client.publish("multi-channel", "msg2").await.unwrap();
        let msg1 = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout msg1")
            .expect("receive error");
        assert_eq!(msg1.payload, "msg1");
        let msg2 = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout msg2")
            .expect("receive error");
        assert_eq!(msg2.payload, "msg2");
    }
}
