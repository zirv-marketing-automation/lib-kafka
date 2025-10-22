//! Type-safe Kafka producer.

use crate::error::{KafkaError, Result};
use kafka_messages::KafkaMessage;
use rdkafka::producer::{FutureProducer, FutureRecord, Producer as RdProducer};
use rdkafka::ClientConfig;
use std::time::Duration;
use tracing::{debug, error, info};

/// Type-safe Kafka producer.
///
/// The producer provides a generic `send` method that accepts any type implementing
/// `KafkaMessage`. Messages are automatically serialized to JSON and sent to the
/// topic specified by the message type.
///
/// # Example
///
/// ```no_run
/// use kafka::Producer;
/// use kafka_messages::KafkaMessage;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct UserCreated {
///     user_id: String,
/// }
///
/// impl KafkaMessage for UserCreated {
///     const TOPIC: &'static str = "user.created";
/// }
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let producer = Producer::new("localhost:9092")?;
///     
///     let message = UserCreated {
///         user_id: "123".to_string(),
///     };
///     
///     producer.send(&message).await?;
///     Ok(())
/// }
/// ```
pub struct Producer {
    inner: FutureProducer,
}

impl Producer {
    /// Creates a new Kafka producer.
    ///
    /// # Arguments
    ///
    /// * `brokers` - Comma-separated list of Kafka brokers (e.g., "localhost:9092")
    ///
    /// # Errors
    ///
    /// Returns an error if the producer cannot be created.
    pub fn new(brokers: &str) -> Result<Self> {
        info!("Creating Kafka producer with brokers: {}", brokers);
        
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("message.timeout.ms", "5000")
            .set("queue.buffering.max.messages", "100000")
            .set("queue.buffering.max.kbytes", "1048576")
            .set("batch.num.messages", "10000")
            .create()?;

        Ok(Self { inner: producer })
    }

    /// Creates a new Kafka producer with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `config` - Pre-configured ClientConfig
    ///
    /// # Errors
    ///
    /// Returns an error if the producer cannot be created.
    pub fn from_config(config: ClientConfig) -> Result<Self> {
        let producer: FutureProducer = config.create()?;
        Ok(Self { inner: producer })
    }

    /// Sends a typed message to Kafka.
    ///
    /// The message is serialized to JSON and sent to the topic specified by
    /// the message type's `TOPIC` constant.
    ///
    /// # Arguments
    ///
    /// * `message` - A reference to a message implementing `KafkaMessage`
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails or if the message cannot be sent.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use kafka::Producer;
    /// # use kafka_messages::KafkaMessage;
    /// # use serde::{Deserialize, Serialize};
    /// # #[derive(Debug, Serialize, Deserialize)]
    /// # struct UserCreated { user_id: String }
    /// # impl KafkaMessage for UserCreated {
    /// #     const TOPIC: &'static str = "user.created";
    /// # }
    /// # async fn example() -> anyhow::Result<()> {
    /// let producer = Producer::new("localhost:9092")?;
    /// let message = UserCreated { user_id: "123".to_string() };
    /// producer.send(&message).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send<T: KafkaMessage>(&self, message: &T) -> Result<()> {
        let topic = T::TOPIC;
        
        let payload = serde_json::to_vec(message)
            .map_err(|e| KafkaError::Serialization(e.to_string()))?;

        debug!(
            "Sending message to topic '{}' ({} bytes)",
            topic,
            payload.len()
        );

        let record = FutureRecord {
            topic,
            partition: None,
            payload: Some(&payload),
            key: None::<&[u8]>,
            timestamp: None,
            headers: None,
        };

        match self.inner.send(record, Duration::from_secs(5)).await {
            Ok((partition, offset)) => {
                debug!(
                    "Message sent successfully to topic '{}' (partition: {}, offset: {})",
                    topic, partition, offset
                );
                Ok(())
            }
            Err((kafka_err, _msg)) => {
                error!("Failed to send message to topic '{}': {}", topic, kafka_err);
                Err(KafkaError::Kafka(kafka_err))
            }
        }
    }

    /// Sends a typed message with a specific key to Kafka.
    ///
    /// The key is used for partition assignment and ordering guarantees.
    ///
    /// # Arguments
    ///
    /// * `message` - A reference to a message implementing `KafkaMessage`
    /// * `key` - The message key
    ///
    /// # Errors
    ///
    /// Returns an error if serialization fails or if the message cannot be sent.
    pub async fn send_with_key<T: KafkaMessage>(&self, message: &T, key: &str) -> Result<()> {
        let topic = T::TOPIC;
        
        let payload = serde_json::to_vec(message)
            .map_err(|e| KafkaError::Serialization(e.to_string()))?;

        debug!(
            "Sending message to topic '{}' with key '{}' ({} bytes)",
            topic,
            key,
            payload.len()
        );

        let key_bytes = key.as_bytes();
        let record = FutureRecord {
            topic,
            partition: None,
            payload: Some(&payload),
            key: Some(key_bytes),
            timestamp: None,
            headers: None,
        };

        match self.inner.send(record, Duration::from_secs(5)).await {
            Ok((partition, offset)) => {
                debug!(
                    "Message sent successfully to topic '{}' with key '{}' (partition: {}, offset: {})",
                    topic, key, partition, offset
                );
                Ok(())
            }
            Err((kafka_err, _msg)) => {
                error!(
                    "Failed to send message to topic '{}' with key '{}': {}",
                    topic, key, kafka_err
                );
                Err(KafkaError::Kafka(kafka_err))
            }
        }
    }

    /// Flushes any pending messages.
    ///
    /// This ensures all messages are sent before the producer is dropped.
    ///
    /// # Errors
    ///
    /// Returns an error if the flush operation fails.
    pub fn flush(&self, timeout: Duration) -> Result<()> {
        RdProducer::flush(&self.inner, timeout)?;
        Ok(())
    }
}
