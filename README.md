# lib-kafka

A type-safe Rust Kafka library built on Tokio and rdkafka for microservices.

## Features

- 🎯 **Type-safe Producer**: Send messages with compile-time topic verification
- 🔄 **Consumer with Topic Routing**: HashMap-based message routing by topic
- 🎨 **Convenient Macro**: `topic_handlers!` for easy handler registration
- 📦 **JSON Serialization**: Built-in serde JSON support
- 🔁 **Retry & Backoff**: Automatic retry with exponential backoff on errors
- ✅ **Commit on Success**: Messages are committed only after successful processing
- 🛑 **Graceful Shutdown**: Clean shutdown handling with signal support
- 📊 **Tracing Integration**: Built-in tracing for observability
- 🧪 **Well-tested**: Comprehensive test suite

## Architecture

This library consists of two crates:

- **kafka-messages**: Core trait definitions for message types
- **kafka**: Producer and Consumer implementations with Tokio + rdkafka

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
kafka = { path = "kafka" }
kafka-messages = { path = "kafka-messages" }
tokio = { version = "1.35", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
anyhow = "1.0"
```

## Quick Start

### Define Message Types

```rust
use kafka_messages::KafkaMessage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct UserCreated {
    user_id: String,
    email: String,
}

impl KafkaMessage for UserCreated {
    const TOPIC: &'static str = "user.created";
}
```

### Producer Example

```rust
use kafka::Producer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let producer = Producer::new("localhost:9092")?;
    
    let message = UserCreated {
        user_id: "123".to_string(),
        email: "user@example.com".to_string(),
    };
    
    // Type-safe send - topic is determined by the message type
    producer.send(&message).await?;
    
    // Send with key for ordering guarantees
    producer.send_with_key(&message, "user-123").await?;
    
    Ok(())
}
```

### Consumer Example

```rust
use kafka::{Consumer, ConsumerConfig, topic_handlers};
use std::time::Duration;

async fn handle_user_created(msg: UserCreated) -> anyhow::Result<()> {
    println!("User created: {:?}", msg);
    // Process the message...
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Configure consumer
    let config = ConsumerConfig::new("localhost:9092", "my-service")
        .with_max_retries(3)
        .with_initial_backoff(Duration::from_millis(100));
    
    // Register handlers using the macro
    let handlers = topic_handlers![
        UserCreated => handle_user_created,
    ];
    
    // Create and run consumer
    let consumer = Consumer::new(config, handlers)?;
    consumer.subscribe()?;
    consumer.run().await?;
    
    Ok(())
}
```

### Graceful Shutdown

```rust
use tokio::signal;

// Get shutdown handle
let shutdown_handle = consumer.shutdown_handle();

// Spawn shutdown handler
tokio::spawn(async move {
    signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
    shutdown_handle.shutdown().await;
});

// Run consumer (blocks until shutdown)
consumer.run().await?;
```

## Advanced Features

### Custom Producer Configuration

```rust
use rdkafka::ClientConfig;

let mut config = ClientConfig::new();
config
    .set("bootstrap.servers", "localhost:9092")
    .set("compression.type", "snappy")
    .set("batch.size", "16384");

let producer = Producer::from_config(config)?;
```

### Error Handling

The consumer automatically:
- Retries failed messages with exponential backoff
- Commits offsets only on successful processing
- Logs errors with tracing

```rust
async fn handle_message(msg: MyMessage) -> anyhow::Result<()> {
    // If this returns an error, the message will be retried
    process_message(&msg)?;
    Ok(())
}
```

### Multiple Message Types

```rust
let handlers = topic_handlers![
    UserCreated => handle_user_created,
    OrderPlaced => handle_order_placed,
    PaymentProcessed => handle_payment,
];
```

## Examples

Run the included examples:

```bash
# Start a producer
cargo run --example producer

# Start a consumer (in another terminal)
cargo run --example consumer
```

## Testing

```bash
cargo test
```

## Configuration

### Environment Variables

- `KAFKA_BROKERS`: Kafka broker addresses (default: `localhost:9092`)
- `KAFKA_GROUP_ID`: Consumer group ID (default: `example-consumer-group`)

### Consumer Configuration

- `max_retries`: Maximum retry attempts (default: 3)
- `initial_backoff`: Initial backoff duration (default: 100ms)
- `max_backoff`: Maximum backoff duration (default: 30s)
- `auto_commit`: Enable auto-commit (default: false, manual commit recommended)

## Requirements

- Rust 1.70 or later
- Kafka 2.0 or later
- librdkafka (automatically built via cmake)

## License

MIT

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
