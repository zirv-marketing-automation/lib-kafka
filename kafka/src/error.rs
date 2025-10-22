//! Error types for the Kafka library.

use thiserror::Error;

/// Result type alias for Kafka operations.
pub type Result<T> = std::result::Result<T, KafkaError>;

/// Errors that can occur during Kafka operations.
#[derive(Error, Debug)]
pub enum KafkaError {
    /// Error from the underlying rdkafka library.
    #[error("Kafka error: {0}")]
    Kafka(#[from] rdkafka::error::KafkaError),

    /// Error serializing a message to JSON.
    #[error("Serialization error: {0}")]
    Serialization(String),

    /// Error deserializing a message from JSON.
    #[error("Deserialization error: {0}")]
    Deserialization(String),

    /// Error from a message handler.
    #[error("Handler error: {0}")]
    Handler(String),

    /// Error during consumer shutdown.
    #[error("Shutdown error: {0}")]
    Shutdown(String),

    /// Generic error.
    #[error("{0}")]
    Other(String),
}
