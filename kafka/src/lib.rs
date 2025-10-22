//! Type-safe Kafka producer and consumer library.
//!
//! This crate provides type-safe Kafka operations built on top of `rdkafka` and `tokio`.
//!
//! # Features
//!
//! - Type-safe producer with `send<T: KafkaMessage>(&T)` API
//! - Consumer with topic-based routing using `HashMap<&'static str, HandlerBox>`
//! - `topic_handlers!` macro for easy handler registration
//! - JSON serialization/deserialization
//! - Automatic commit on success, retry with backoff on error
//! - Graceful shutdown support
//! - Integrated tracing
//!
//! # Example Producer
//!
//! ```no_run
//! use kafka::Producer;
//! use kafka_messages::KafkaMessage;
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Debug, Serialize, Deserialize)]
//! struct UserCreated {
//!     user_id: String,
//! }
//!
//! impl KafkaMessage for UserCreated {
//!     const TOPIC: &'static str = "user.created";
//! }
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let producer = Producer::new("localhost:9092")?;
//!     
//!     let message = UserCreated {
//!         user_id: "123".to_string(),
//!     };
//!     
//!     producer.send(&message).await?;
//!     Ok(())
//! }
//! ```

mod producer;
mod consumer;
mod error;

pub use producer::Producer;
pub use consumer::{Consumer, ConsumerConfig, HandlerBox};
pub use error::{KafkaError, Result};

/// Re-export the KafkaMessage trait for convenience
pub use kafka_messages::KafkaMessage;

/// Macro to create a HashMap of topic handlers for the Consumer.
///
/// # Example
///
/// ```no_run
/// use kafka::{topic_handlers, KafkaMessage};
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
/// async fn handle_user_created(msg: UserCreated) -> anyhow::Result<()> {
///     println!("User created: {:?}", msg);
///     Ok(())
/// }
///
/// let handlers = topic_handlers![
///     UserCreated => handle_user_created,
/// ];
/// ```
#[macro_export]
macro_rules! topic_handlers {
    ($($msg_type:ty => $handler:expr),* $(,)?) => {{
        let mut map: ::std::collections::HashMap<
            &'static str,
            $crate::HandlerBox,
        > = ::std::collections::HashMap::new();
        $(
            let handler = ::std::sync::Arc::new($handler);
            map.insert(
                <$msg_type as $crate::KafkaMessage>::TOPIC,
                Box::new(move |payload: &[u8]| {
                    let handler = ::std::sync::Arc::clone(&handler);
                    let payload = payload.to_vec();
                    Box::pin(async move {
                        let msg: $msg_type = ::serde_json::from_slice(&payload)
                            .map_err(|e| $crate::KafkaError::Deserialization(e.to_string()))?;
                        handler(msg).await
                            .map_err(|e| $crate::KafkaError::Handler(e.to_string()))
                    })
                })
            );
        )*
        map
    }};
}

