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

    /// Subscribe to a channel and return messages as a channel.
    ///
    /// Creates a new Redis client connection dedicated to receiving messages.
    /// The subscriber runs in a background task.
    pub async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<tokio::sync::mpsc::Receiver<PubSubMessage>, CacheError> {
        let full_channel = self.make_key("pubsub", channel);
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        // Clone the client for the subscriber connection
        let subscriber_client = self.client.clone_new();
        subscriber_client
            .init()
            .await
            .map_err(CacheError::CommandFailed)?;

        // Subscribe to the channel
        subscriber_client
            .subscribe(&full_channel)
            .await
            .map_err(CacheError::CommandFailed)?;

        // Spawn a task to forward messages
        tokio::spawn(async move {
            let mut message_rx = subscriber_client.message_rx();
            loop {
                match message_rx.recv().await {
                    Ok(message) => {
                        let channel_str = message.channel.to_string();
                        // Convert fred Value to String
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
                        if tx.send(msg).await.is_err() {
                            break; // receiver dropped
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "pubsub message receive error");
                        break;
                    }
                }
            }
            let _ = subscriber_client.quit().await;
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn integration_publish_subscribe_roundtrip() {
        let client = RedisClient::new("redis://127.0.0.1:6379/9", "test")
            .await
            .expect("redis connect");
        let mut rx = client.subscribe("test-channel").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        client
            .publish("test-channel", "hello pubsub")
            .await
            .unwrap();
        let msg = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout waiting for message")
            .expect("channel closed");
        assert!(msg.channel.ends_with("test-channel"));
        assert_eq!(msg.payload, "hello pubsub");
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn integration_publish_multiple_messages() {
        let client = RedisClient::new("redis://127.0.0.1:6379/9", "test")
            .await
            .expect("redis connect");
        let mut rx = client.subscribe("multi-channel").await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        client.publish("multi-channel", "msg1").await.unwrap();
        client.publish("multi-channel", "msg2").await.unwrap();
        let msg1 = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout msg1")
            .expect("channel closed");
        assert_eq!(msg1.payload, "msg1");
        let msg2 = tokio::time::timeout(tokio::time::Duration::from_secs(2), rx.recv())
            .await
            .expect("timeout msg2")
            .expect("channel closed");
        assert_eq!(msg2.payload, "msg2");
    }
}
