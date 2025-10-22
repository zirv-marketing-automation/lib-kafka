# Best Practices and Patterns

This document outlines best practices and common patterns for using lib-kafka in production microservices.

## Message Design

### Naming Conventions

Use consistent topic naming:

```rust
// Good: hierarchical namespace
impl KafkaMessage for UserCreated {
    const TOPIC: &'static str = "user.created";
}

impl KafkaMessage for OrderPlaced {
    const TOPIC: &'static str = "order.placed";
}

// Consider: service prefix for multi-service architectures
impl KafkaMessage for PaymentReceived {
    const TOPIC: &'static str = "payment.payment_received";
}
```

### Message Evolution

Design messages for forward/backward compatibility:

```rust
#[derive(Debug, Serialize, Deserialize)]
struct UserCreated {
    // Required fields
    pub user_id: String,
    pub email: String,
    
    // Optional fields for evolution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<HashMap<String, String>>,
    
    // Version field helps with migrations
    #[serde(default)]
    pub version: u32,
}
```

### Message Versioning

When you need breaking changes, create new message types:

```rust
impl KafkaMessage for UserCreatedV1 {
    const TOPIC: &'static str = "user.created.v1";
}

impl KafkaMessage for UserCreatedV2 {
    const TOPIC: &'static str = "user.created.v2";
}

// Or use a version field and handle internally
async fn handle_user_created(msg: UserCreated) -> anyhow::Result<()> {
    match msg.version {
        1 => handle_v1(msg).await,
        2 => handle_v2(msg).await,
        _ => Err(anyhow::anyhow!("Unsupported version")),
    }
}
```

## Producer Patterns

### Batch Processing

Send multiple messages efficiently:

```rust
async fn process_batch(producer: &Producer, items: Vec<Item>) -> anyhow::Result<()> {
    for item in items {
        let message = ItemProcessed {
            item_id: item.id,
            timestamp: Utc::now().timestamp(),
        };
        
        producer.send(&message).await?;
    }
    
    // Flush after batch
    producer.flush(Duration::from_secs(5))?;
    Ok(())
}
```

### Transaction Outbox Pattern

Ensure message delivery with database transactions:

```rust
async fn create_user_with_event(
    db: &Database,
    producer: &Producer,
    user: User,
) -> anyhow::Result<()> {
    // Start database transaction
    let mut tx = db.begin().await?;
    
    // Save to database
    sqlx::query("INSERT INTO users (...) VALUES (...)")
        .execute(&mut tx)
        .await?;
    
    // Save outbox message
    let event = UserCreated { /* ... */ };
    let payload = serde_json::to_string(&event)?;
    
    sqlx::query("INSERT INTO outbox (topic, payload) VALUES (?, ?)")
        .bind(UserCreated::TOPIC)
        .bind(&payload)
        .execute(&mut tx)
        .await?;
    
    // Commit transaction
    tx.commit().await?;
    
    // Separate process sends from outbox to Kafka
    Ok(())
}
```

### Idempotent Producer

Add message IDs for idempotency:

```rust
#[derive(Debug, Serialize, Deserialize)]
struct UserCreated {
    #[serde(default = "generate_id")]
    pub message_id: String,
    pub user_id: String,
    pub email: String,
}

fn generate_id() -> String {
    uuid::Uuid::new_v4().to_string()
}
```

## Consumer Patterns

### Idempotent Processing

Handle duplicate messages gracefully:

```rust
async fn handle_user_created(msg: UserCreated) -> anyhow::Result<()> {
    // Check if already processed
    if already_processed(&msg.message_id).await? {
        return Ok(());
    }
    
    // Process message
    process_user(&msg).await?;
    
    // Mark as processed
    mark_processed(&msg.message_id).await?;
    
    Ok(())
}
```

### Dead Letter Queue

Handle poison messages:

```rust
async fn handle_with_dlq(
    msg: MyMessage,
    dlq_producer: &Producer,
) -> anyhow::Result<()> {
    match process_message(&msg).await {
        Ok(()) => Ok(()),
        Err(e) if is_retriable(&e) => Err(e), // Will be retried
        Err(e) => {
            // Send to DLQ
            let dlq_msg = DeadLetter {
                original_topic: MyMessage::TOPIC.to_string(),
                payload: serde_json::to_value(&msg)?,
                error: e.to_string(),
                timestamp: Utc::now(),
            };
            
            dlq_producer.send(&dlq_msg).await?;
            Ok(()) // Don't retry
        }
    }
}
```

### Saga Pattern

Coordinate distributed transactions:

```rust
async fn handle_order_placed(msg: OrderPlaced) -> anyhow::Result<()> {
    let saga_id = msg.order_id.clone();
    
    // Step 1: Reserve inventory
    let inventory_reserved = reserve_inventory(&msg).await?;
    if !inventory_reserved {
        publish_saga_failed(saga_id, "inventory").await?;
        return Ok(());
    }
    
    // Step 2: Process payment
    match process_payment(&msg).await {
        Ok(()) => {
            publish_saga_completed(saga_id).await?;
        }
        Err(e) => {
            // Compensate: release inventory
            release_inventory(&msg).await?;
            publish_saga_failed(saga_id, "payment").await?;
        }
    }
    
    Ok(())
}
```

### Fan-out Pattern

One message triggers multiple actions:

```rust
async fn handle_user_created(msg: UserCreated) -> anyhow::Result<()> {
    // Spawn multiple tasks
    let tasks = vec![
        tokio::spawn(send_welcome_email(msg.clone())),
        tokio::spawn(create_user_profile(msg.clone())),
        tokio::spawn(notify_analytics(msg.clone())),
    ];
    
    // Wait for all to complete
    for task in tasks {
        task.await??;
    }
    
    Ok(())
}
```

## Error Handling

### Categorize Errors

```rust
#[derive(Debug)]
enum ProcessingError {
    Retriable(String),      // Network errors, temporary failures
    NonRetriable(String),   // Validation errors, business logic errors
    Fatal(String),          // Configuration errors
}

async fn handle_message(msg: MyMessage) -> anyhow::Result<()> {
    match validate_and_process(&msg).await {
        Ok(()) => Ok(()),
        Err(e) if is_retriable(&e) => {
            Err(anyhow::anyhow!("Retriable error: {}", e))
        }
        Err(e) => {
            log_non_retriable_error(&e);
            Ok(()) // Don't retry
        }
    }
}
```

### Circuit Breaker

Prevent cascading failures:

```rust
use std::sync::Arc;
use tokio::sync::RwLock;

struct CircuitBreaker {
    failures: Arc<RwLock<u32>>,
    threshold: u32,
}

impl CircuitBreaker {
    async fn call<F, T>(&self, f: F) -> anyhow::Result<T>
    where
        F: Future<Output = anyhow::Result<T>>,
    {
        let failures = self.failures.read().await;
        if *failures >= self.threshold {
            return Err(anyhow::anyhow!("Circuit breaker open"));
        }
        drop(failures);
        
        match f.await {
            Ok(result) => {
                *self.failures.write().await = 0;
                Ok(result)
            }
            Err(e) => {
                *self.failures.write().await += 1;
                Err(e)
            }
        }
    }
}
```

## Performance Optimization

### Handler Concurrency

Process multiple partitions concurrently:

```rust
// The consumer automatically processes partitions concurrently
// But ensure your handlers don't block:

async fn handle_message(msg: MyMessage) -> anyhow::Result<()> {
    // Good: async I/O
    let result = async_http_call(&msg).await?;
    
    // Bad: blocking I/O (use spawn_blocking instead)
    // let result = blocking_operation(&msg);
    
    Ok(())
}
```

### Batch Processing in Handlers

```rust
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
struct BatchProcessor {
    buffer: Arc<Mutex<Vec<MyMessage>>>,
}

impl BatchProcessor {
    async fn process(&self, msg: MyMessage) -> anyhow::Result<()> {
        let mut buffer = self.buffer.lock().await;
        buffer.push(msg);
        
        if buffer.len() >= 100 {
            let batch = buffer.drain(..).collect::<Vec<_>>();
            drop(buffer); // Release lock before processing
            
            self.process_batch(batch).await?;
        }
        
        Ok(())
    }
}
```

## Monitoring and Observability

### Structured Logging

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(msg))]
async fn handle_user_created(msg: UserCreated) -> anyhow::Result<()> {
    info!(
        user_id = %msg.user_id,
        email = %msg.email,
        "Processing user creation"
    );
    
    match process_user(&msg).await {
        Ok(()) => {
            info!(user_id = %msg.user_id, "User created successfully");
            Ok(())
        }
        Err(e) => {
            error!(
                user_id = %msg.user_id,
                error = %e,
                "Failed to create user"
            );
            Err(e)
        }
    }
}
```

### Metrics

Track important metrics:

```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone)]
struct Metrics {
    messages_processed: Arc<AtomicU64>,
    messages_failed: Arc<AtomicU64>,
}

impl Metrics {
    async fn record_success(&self) {
        self.messages_processed.fetch_add(1, Ordering::Relaxed);
    }
    
    async fn record_failure(&self) {
        self.messages_failed.fetch_add(1, Ordering::Relaxed);
    }
}
```

## Testing Strategies

### Unit Test Handlers

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_user_creation_handler() {
        let msg = UserCreated {
            user_id: "test-123".to_string(),
            email: "test@example.com".to_string(),
        };
        
        let result = handle_user_created(msg).await;
        assert!(result.is_ok());
    }
    
    #[tokio::test]
    async fn test_invalid_email() {
        let msg = UserCreated {
            user_id: "test-123".to_string(),
            email: "invalid".to_string(),
        };
        
        let result = handle_user_created(msg).await;
        assert!(result.is_err());
    }
}
```

### Integration Tests with Testcontainers

```rust
#[cfg(test)]
mod integration_tests {
    use testcontainers::*;
    
    #[tokio::test]
    async fn test_end_to_end() {
        // Start Kafka container
        let docker = clients::Cli::default();
        let kafka = docker.run(images::kafka::Kafka::default());
        
        let bootstrap = format!(
            "localhost:{}",
            kafka.get_host_port_ipv4(9092)
        );
        
        // Create producer and consumer
        let producer = Producer::new(&bootstrap)?;
        let config = ConsumerConfig::new(&bootstrap, "test-group");
        
        // Run test...
    }
}
```

## Deployment

### Health Checks

```rust
use axum::{Router, routing::get};

async fn health() -> &'static str {
    "healthy"
}

async fn ready() -> &'static str {
    // Check Kafka connectivity
    "ready"
}

#[tokio::main]
async fn main() {
    // Start HTTP server for health checks
    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready));
    
    tokio::spawn(async {
        axum::Server::bind(&"0.0.0.0:8080".parse().unwrap())
            .serve(app.into_make_service())
            .await
            .unwrap();
    });
    
    // Start consumer
    // ...
}
```

### Configuration Management

```rust
use config::{Config, ConfigError, Environment, File};

#[derive(Debug, Deserialize)]
struct Settings {
    kafka_brokers: String,
    consumer_group: String,
    max_retries: u32,
}

impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        Config::builder()
            .add_source(File::with_name("config/default"))
            .add_source(Environment::with_prefix("APP"))
            .build()?
            .try_deserialize()
    }
}
```

## Summary

These patterns help you build robust, scalable microservices with lib-kafka:

1. **Message Design**: Plan for evolution and versioning
2. **Producer Patterns**: Use batch processing and idempotency
3. **Consumer Patterns**: Implement retries, DLQ, and saga patterns
4. **Error Handling**: Categorize and handle errors appropriately
5. **Performance**: Optimize for your specific use case
6. **Monitoring**: Use structured logging and metrics
7. **Testing**: Test handlers and use integration tests
8. **Deployment**: Implement health checks and configuration management

For more examples, see the `kafka/examples/` directory.
