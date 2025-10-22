//! Example Kafka consumer demonstrating type-safe message handling.
//!
//! This example shows how to:
//! - Define message types with the KafkaMessage trait
//! - Create message handlers
//! - Use the topic_handlers! macro to register handlers
//! - Create and run a Consumer
//! - Handle graceful shutdown
//!
//! To run this example:
//! ```bash
//! cargo run --example consumer
//! ```
//!
//! Make sure you have a Kafka broker running on localhost:9092.
//! Run the producer example in another terminal to generate messages.

use kafka::{Consumer, ConsumerConfig, topic_handlers, KafkaMessage};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::signal;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Example message type for user creation events
#[derive(Debug, Clone, Serialize, Deserialize)]
struct UserCreated {
    user_id: String,
    email: String,
    name: String,
    timestamp: u64,
}

impl KafkaMessage for UserCreated {
    const TOPIC: &'static str = "user.created";
}

/// Example message type for order placement events
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderPlaced {
    order_id: String,
    user_id: String,
    amount: f64,
    items: Vec<String>,
}

impl KafkaMessage for OrderPlaced {
    const TOPIC: &'static str = "order.placed";
}

/// Handler for UserCreated messages
async fn handle_user_created(msg: UserCreated) -> anyhow::Result<()> {
    info!(
        "📧 User created - ID: {}, Email: {}, Name: {}",
        msg.user_id, msg.email, msg.name
    );
    
    // Simulate some processing work
    tokio::time::sleep(Duration::from_millis(50)).await;
    
    info!("✅ Successfully processed user creation for {}", msg.user_id);
    Ok(())
}

/// Handler for OrderPlaced messages
async fn handle_order_placed(msg: OrderPlaced) -> anyhow::Result<()> {
    info!(
        "🛒 Order placed - ID: {}, User: {}, Amount: ${:.2}",
        msg.order_id, msg.user_id, msg.amount
    );
    
    info!("   Items: {:?}", msg.items);
    
    // Simulate some processing work
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    info!("✅ Successfully processed order {}", msg.order_id);
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Kafka consumer example");

    // Get configuration from environment or use defaults
    let brokers = std::env::var("KAFKA_BROKERS")
        .unwrap_or_else(|_| "localhost:9092".to_string());
    let group_id = std::env::var("KAFKA_GROUP_ID")
        .unwrap_or_else(|_| "example-consumer-group".to_string());

    // Create consumer configuration
    let config = ConsumerConfig::new(&brokers, &group_id)
        .with_max_retries(3)
        .with_initial_backoff(Duration::from_millis(100))
        .with_max_backoff(Duration::from_secs(30));

    info!("Consumer config - Brokers: {}, Group: {}", brokers, group_id);

    // Create handlers using the topic_handlers! macro
    let handlers = topic_handlers![
        UserCreated => handle_user_created,
        OrderPlaced => handle_order_placed,
    ];

    // Create consumer
    let consumer = Consumer::new(config, handlers)?;
    info!("Consumer created successfully");

    // Subscribe to topics
    consumer.subscribe()?;
    info!("Subscribed to topics");

    // Get shutdown handle for graceful shutdown
    let shutdown_handle = consumer.shutdown_handle();

    // Spawn shutdown handler
    tokio::spawn(async move {
        // Wait for Ctrl+C
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Received shutdown signal (Ctrl+C)");
        shutdown_handle.shutdown().await;
    });

    // Run consumer (blocks until shutdown)
    info!("Starting to consume messages... Press Ctrl+C to stop");
    consumer.run().await?;

    info!("Consumer shut down gracefully");
    Ok(())
}
