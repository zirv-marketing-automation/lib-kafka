//! Type-safe Kafka message trait definitions.
//!
//! This crate provides the core `KafkaMessage` trait that all Kafka message types
//! should implement. The trait associates a message type with its Kafka topic.

use serde::{Deserialize, Serialize};

/// Trait for types that can be sent as Kafka messages.
///
/// Implementors must specify the Kafka topic where messages of this type should be sent.
/// The trait also requires `Serialize` and `Deserialize` for JSON encoding/decoding.
///
/// # Example
///
/// ```
/// use kafka_messages::KafkaMessage;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Debug, Serialize, Deserialize)]
/// struct UserCreated {
///     user_id: String,
///     email: String,
/// }
///
/// impl KafkaMessage for UserCreated {
///     const TOPIC: &'static str = "user.created";
/// }
/// ```
pub trait KafkaMessage: Serialize + for<'de> Deserialize<'de> + Send + Sync {
    /// The Kafka topic where messages of this type should be sent.
    const TOPIC: &'static str;
}

