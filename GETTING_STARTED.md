# Getting Started with lib-kafka

This guide will help you get started with the lib-kafka library for building type-safe Kafka producers and consumers in Rust.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Installation](#installation)
3. [Core Concepts](#core-concepts)
4. [Producer Guide](#producer-guide)
5. [Consumer Guide](#consumer-guide)
6. [Error Handling](#error-handling)
7. [Testing](#testing)
8. [Production Considerations](#production-considerations)

## Prerequisites

- Rust 1.70 or later
- Kafka broker accessible (local or remote)
- Basic understanding of Kafka concepts (topics, partitions, consumer groups)

## Installation

This is a workspace containing two crates. To use it in your project:

1. Clone or copy this repository into your workspace
2. Add the dependencies to your `Cargo.toml`:

```toml
[dependencies]
kafka = { path = "path/to/lib-kafka/kafka" }
kafka-messages = { path = "path/to/lib-kafka/kafka-messages" }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = "0.3"
```

## Core Concepts

### KafkaMessage Trait

All message types must implement the `KafkaMessage` trait:

```rust
use kafka_messages::KafkaMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct MyMessage {
    id: String,
    data: String,
}

impl KafkaMessage for MyMessage {
    const TOPIC: &'static str = "my.topic";
}
```

**Key points:**
- The `TOPIC` constant defines where messages of this type are sent/received
- Messages must be serializable to/from JSON
- The trait ensures compile-time type safety

### Type Safety

The library provides compile-time guarantees:
- You can't accidentally send a message to the wrong topic
- Handler registration is checked at compile time
- Message deserialization is type-safe

## Producer Guide

### Basic Usage

```rust
use kafka::Producer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Create producer
    let producer = Producer::new("localhost:9092")?;
    
    // Create and send a message
    let message = MyMessage {
        id: "123".to_string(),
        data: "Hello, Kafka!".to_string(),
    };
    
    producer.send(&message).await?;
    
    Ok(())
}
```

### Sending with Keys

Use keys to ensure messages for the same entity go to the same partition:

```rust
// All messages with the same user_id will go to the same partition
producer.send_with_key(&message, &user_id).await?;
```

### Custom Configuration

```rust
use rdkafka::ClientConfig;

let mut config = ClientConfig::new();
config
    .set("bootstrap.servers", "localhost:9092")
    .set("compression.type", "snappy")
    .set("batch.size", "32768")
    .set("linger.ms", "10");

let producer = Producer::from_config(config)?;
```

### Flushing

Always flush before shutting down to ensure all messages are sent:

```rust
use std::time::Duration;

producer.flush(Duration::from_secs(5))?;
```

## Consumer Guide

### Basic Setup

```rust
use kafka::{Consumer, ConsumerConfig, topic_handlers};
use std::time::Duration;

async fn handle_my_message(msg: MyMessage) -> anyhow::Result<()> {
    println!("Received: {:?}", msg);
    // Process the message
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure consumer
    let config = ConsumerConfig::new("localhost:9092", "my-consumer-group");
    
    // Register handlers
    let handlers = topic_handlers![
        MyMessage => handle_my_message,
    ];
    
    // Create and run consumer
    let consumer = Consumer::new(config, handlers)?;
    consumer.subscribe()?;
    consumer.run().await?;
    
    Ok(())
}
```

### Multiple Handlers

Handle multiple message types:

```rust
let handlers = topic_handlers![
    UserCreated => handle_user_created,
    OrderPlaced => handle_order_placed,
    PaymentProcessed => handle_payment,
];
```

### Retry Configuration

Configure retry behavior:

```rust
let config = ConsumerConfig::new("localhost:9092", "my-group")
    .with_max_retries(5)                                // Try up to 5 times
    .with_initial_backoff(Duration::from_millis(100))   // Start with 100ms
    .with_max_backoff(Duration::from_secs(60));        // Cap at 60s
```

**Retry behavior:**
- Exponential backoff: 100ms, 200ms, 400ms, 800ms, ...
- Capped at `max_backoff`
- After `max_retries`, the message is skipped and logged

### Graceful Shutdown

Handle Ctrl+C and other shutdown signals:

```rust
use tokio::signal;

let shutdown_handle = consumer.shutdown_handle();

tokio::spawn(async move {
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    println!("Shutting down gracefully...");
    shutdown_handle.shutdown().await;
});

consumer.run().await?;
```

## Error Handling

### Handler Errors

If your handler returns an error, the message will be retried:

```rust
async fn handle_message(msg: MyMessage) -> anyhow::Result<()> {
    // This will trigger a retry if it fails
    let result = external_api_call(&msg).await?;
    
    // Only committed if we reach here successfully
    Ok(())
}
```

### Deserialization Errors

Messages that can't be deserialized are logged and skipped:

```rust
// If the JSON doesn't match MyMessage structure, it's logged and skipped
```

### Producer Errors

Handle send failures:

```rust
match producer.send(&message).await {
    Ok(()) => println!("Message sent successfully"),
    Err(e) => eprintln!("Failed to send message: {}", e),
}
```

## Testing

### Unit Tests

Test your message handlers:

```rust
#[tokio::test]
async fn test_handler() {
    let msg = MyMessage {
        id: "test".to_string(),
        data: "test data".to_string(),
    };
    
    let result = handle_my_message(msg).await;
    assert!(result.is_ok());
}
```

### Integration Tests

Test with the macro:

```rust
#[tokio::test]
async fn test_handlers() {
    let handlers = topic_handlers![
        MyMessage => handle_my_message,
    ];
    
    assert!(handlers.contains_key("my.topic"));
}
```

### Mock Kafka

For integration tests, consider using:
- [Testcontainers](https://github.com/testcontainers/testcontainers-rs) for real Kafka
- In-memory message queues for faster tests

## Production Considerations

### Logging

Enable structured logging:

```rust
use tracing_subscriber::{fmt, EnvFilter};

tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .json()  // For production
    .init();
```

### Monitoring

The library uses `tracing` for observability. Key events to monitor:
- Message send/receive (at DEBUG level)
- Errors and retries (at WARN/ERROR level)
- Consumer startup/shutdown (at INFO level)

### Consumer Groups

- Use meaningful consumer group IDs
- Each instance with the same group ID will share the load
- Different groups process all messages independently

### Partitioning

- Use keys for messages that need ordering
- Messages with the same key go to the same partition
- Within a partition, messages are processed in order

### Performance Tuning

Producer:
```rust
config
    .set("batch.size", "16384")              // Larger batches
    .set("linger.ms", "10")                  // Wait for more messages
    .set("compression.type", "snappy")       // Enable compression
    .set("queue.buffering.max.messages", "100000");
```

Consumer:
```rust
config
    .set("fetch.min.bytes", "1")             // Min data to fetch
    .set("fetch.max.wait.ms", "500")         // Max wait time
    .set("max.partition.fetch.bytes", "1048576");
```

### High Availability

- Run multiple consumer instances (same group ID)
- Monitor consumer lag
- Set appropriate timeouts
- Handle rebalancing gracefully

### Security

For production with authentication:

```rust
config
    .set("security.protocol", "SASL_SSL")
    .set("sasl.mechanism", "PLAIN")
    .set("sasl.username", username)
    .set("sasl.password", password);
```

## Examples

See the `kafka/examples/` directory for complete examples:
- `producer.rs` - Comprehensive producer example
- `consumer.rs` - Consumer with multiple handlers and graceful shutdown

Run them:
```bash
cargo run --example producer
cargo run --example consumer
```

## Troubleshooting

### "No Kafka brokers available"
- Check that your broker address is correct
- Ensure Kafka is running: `docker ps` or check your Kafka installation

### "Consumer rebalancing"
- Normal when consumers join/leave the group
- Ensure session timeout is appropriate for your use case

### "Message processing is slow"
- Check your handler performance
- Consider increasing consumer instances
- Review retry configuration

### "Messages are being skipped"
- Check logs for deserialization errors
- Verify message format matches your struct
- Ensure all required fields are present

## Further Reading

- [rdkafka Documentation](https://docs.rs/rdkafka/)
- [Kafka Documentation](https://kafka.apache.org/documentation/)
- [Tokio Documentation](https://tokio.rs/)
