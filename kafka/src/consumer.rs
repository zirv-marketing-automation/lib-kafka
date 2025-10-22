//! Type-safe Kafka consumer with topic-based routing.

use crate::error::Result;
use rdkafka::consumer::{Consumer as RdConsumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::ClientConfig;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Type alias for message handlers.
///
/// Handlers are async functions that take a message payload and return a Result.
pub type HandlerBox = Box<
    dyn Fn(&[u8]) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> + Send + Sync,
>;

/// Configuration for the Kafka consumer.
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Comma-separated list of Kafka brokers.
    pub brokers: String,
    
    /// Consumer group ID.
    pub group_id: String,
    
    /// Maximum number of retry attempts on error.
    pub max_retries: u32,
    
    /// Initial backoff duration for retries.
    pub initial_backoff: Duration,
    
    /// Maximum backoff duration for retries.
    pub max_backoff: Duration,
    
    /// Whether to enable auto-commit (default: false, we commit manually).
    pub auto_commit: bool,
}

impl ConsumerConfig {
    /// Creates a new consumer configuration.
    ///
    /// # Arguments
    ///
    /// * `brokers` - Comma-separated list of Kafka brokers
    /// * `group_id` - Consumer group ID
    pub fn new(brokers: impl Into<String>, group_id: impl Into<String>) -> Self {
        Self {
            brokers: brokers.into(),
            group_id: group_id.into(),
            max_retries: 3,
            initial_backoff: Duration::from_millis(100),
            max_backoff: Duration::from_secs(30),
            auto_commit: false,
        }
    }

    /// Sets the maximum number of retry attempts.
    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Sets the initial backoff duration.
    pub fn with_initial_backoff(mut self, duration: Duration) -> Self {
        self.initial_backoff = duration;
        self
    }

    /// Sets the maximum backoff duration.
    pub fn with_max_backoff(mut self, duration: Duration) -> Self {
        self.max_backoff = duration;
        self
    }

    /// Enables auto-commit (not recommended, use manual commit).
    pub fn with_auto_commit(mut self, enabled: bool) -> Self {
        self.auto_commit = enabled;
        self
    }
}

/// Type-safe Kafka consumer with topic-based message routing.
///
/// The consumer routes messages to handlers based on their topic using a HashMap.
/// It includes retry logic with exponential backoff, automatic commit on success,
/// and graceful shutdown support.
pub struct Consumer {
    inner: StreamConsumer,
    handlers: HashMap<&'static str, HandlerBox>,
    config: ConsumerConfig,
    shutdown: Arc<RwLock<bool>>,
}

impl Consumer {
    /// Creates a new Kafka consumer with topic handlers.
    ///
    /// # Arguments
    ///
    /// * `config` - Consumer configuration
    /// * `handlers` - HashMap of topic names to handler functions
    ///
    /// # Errors
    ///
    /// Returns an error if the consumer cannot be created.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use kafka::{Consumer, ConsumerConfig, topic_handlers, KafkaMessage};
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
    /// # async fn example() -> anyhow::Result<()> {
    /// let config = ConsumerConfig::new("localhost:9092", "my-service");
    /// let handlers = topic_handlers![
    ///     UserCreated => handle_user_created,
    /// ];
    ///
    /// let consumer = Consumer::new(config, handlers)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(
        config: ConsumerConfig,
        handlers: HashMap<&'static str, HandlerBox>,
    ) -> Result<Self> {
        info!(
            "Creating Kafka consumer with brokers: {}, group: {}",
            config.brokers, config.group_id
        );

        let mut client_config = ClientConfig::new();
        client_config
            .set("bootstrap.servers", &config.brokers)
            .set("group.id", &config.group_id)
            .set("enable.auto.commit", config.auto_commit.to_string())
            .set("auto.offset.reset", "earliest")
            .set("session.timeout.ms", "6000")
            .set("enable.partition.eof", "false");

        let consumer: StreamConsumer = client_config.create()?;

        Ok(Self {
            inner: consumer,
            handlers,
            config,
            shutdown: Arc::new(RwLock::new(false)),
        })
    }

    /// Subscribes to all topics that have registered handlers.
    ///
    /// # Errors
    ///
    /// Returns an error if subscription fails.
    pub fn subscribe(&self) -> Result<()> {
        let topics: Vec<&str> = self.handlers.keys().copied().collect();
        info!("Subscribing to topics: {:?}", topics);
        
        self.inner.subscribe(&topics)?;
        Ok(())
    }

    /// Starts consuming messages.
    ///
    /// This method will run until a shutdown signal is received via `shutdown()`.
    /// Messages are processed according to their topic handlers, with automatic
    /// retries on failure and commit on success.
    ///
    /// # Errors
    ///
    /// Returns an error if consumption fails critically.
    pub async fn run(&self) -> Result<()> {
        info!("Starting consumer loop");

        loop {
            // Check for shutdown signal
            if *self.shutdown.read().await {
                info!("Shutdown signal received, stopping consumer");
                break;
            }

            // Poll for messages with a timeout
            match tokio::time::timeout(
                Duration::from_secs(1),
                self.inner.recv(),
            ).await {
                Ok(Ok(message)) => {
                    let topic = message.topic();
                    let partition = message.partition();
                    let offset = message.offset();
                    
                    debug!(
                        "Received message from topic '{}' (partition: {}, offset: {})",
                        topic, partition, offset
                    );

                    // Get handler for this topic
                    if let Some(handler) = self.handlers.get(topic) {
                        if let Some(payload) = message.payload() {
                            // Process message with retries
                            if let Err(e) = self.process_with_retry(handler, payload).await {
                                error!(
                                    "Failed to process message from topic '{}' after retries: {}",
                                    topic, e
                                );
                                // Continue processing other messages
                                continue;
                            }

                            // Commit offset on success
                            if !self.config.auto_commit {
                                if let Err(e) = self.inner.commit_message(&message, rdkafka::consumer::CommitMode::Async) {
                                    error!("Failed to commit offset: {}", e);
                                }
                            }
                        } else {
                            warn!("Received message with no payload from topic '{}'", topic);
                        }
                    } else {
                        warn!("No handler registered for topic '{}'", topic);
                    }
                }
                Ok(Err(e)) => {
                    error!("Error receiving message: {}", e);
                    sleep(Duration::from_secs(1)).await;
                }
                Err(_) => {
                    // Timeout, check shutdown and continue
                    continue;
                }
            }
        }

        info!("Consumer stopped");
        Ok(())
    }

    /// Processes a message with retry logic and exponential backoff.
    async fn process_with_retry(
        &self,
        handler: &HandlerBox,
        payload: &[u8],
    ) -> Result<()> {
        let mut attempt = 0;
        let mut backoff = self.config.initial_backoff;

        loop {
            match handler(payload).await {
                Ok(()) => {
                    if attempt > 0 {
                        info!("Message processed successfully after {} retries", attempt);
                    }
                    return Ok(());
                }
                Err(e) => {
                    attempt += 1;
                    
                    if attempt > self.config.max_retries {
                        error!(
                            "Failed to process message after {} attempts: {}",
                            self.config.max_retries, e
                        );
                        return Err(e);
                    }

                    warn!(
                        "Error processing message (attempt {}/{}): {}. Retrying in {:?}",
                        attempt, self.config.max_retries, e, backoff
                    );

                    sleep(backoff).await;

                    // Exponential backoff with cap
                    backoff = std::cmp::min(backoff * 2, self.config.max_backoff);
                }
            }
        }
    }

    /// Requests a graceful shutdown of the consumer.
    ///
    /// This signals the consumer to stop processing messages and exit cleanly.
    pub async fn shutdown(&self) {
        info!("Requesting consumer shutdown");
        let mut shutdown = self.shutdown.write().await;
        *shutdown = true;
    }

    /// Returns a shutdown handle that can be used to signal shutdown from another task.
    pub fn shutdown_handle(&self) -> ShutdownHandle {
        ShutdownHandle {
            shutdown: Arc::clone(&self.shutdown),
        }
    }
}

/// Handle for requesting consumer shutdown from another task.
#[derive(Clone)]
pub struct ShutdownHandle {
    shutdown: Arc<RwLock<bool>>,
}

impl ShutdownHandle {
    /// Requests a graceful shutdown of the consumer.
    pub async fn shutdown(&self) {
        info!("Requesting consumer shutdown via handle");
        let mut shutdown = self.shutdown.write().await;
        *shutdown = true;
    }
}
