//! Example Kafka producer demonstrating type-safe message sending.
//!
//! This example shows how to:
//! - Define message types with the KafkaMessage trait
//! - Create a Producer instance
//! - Send typed messages to Kafka
//! - Use message keys for ordering
//!
//! To run this example:
//! ```bash
//! cargo run --example producer
//! ```
//!
//! Make sure you have a Kafka broker running on localhost:9092.

use kafka::{Producer, KafkaMessage};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, Level};
use tracing_subscriber;

/// Example message type for user creation events
#[derive(Debug, Serialize, Deserialize)]
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
#[derive(Debug, Serialize, Deserialize)]
struct OrderPlaced {
    order_id: String,
    user_id: String,
    amount: f64,
    items: Vec<String>,
}

impl KafkaMessage for OrderPlaced {
    const TOPIC: &'static str = "order.placed";
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Starting Kafka producer example");

    // Get broker address from environment or use default
    let brokers = std::env::var("KAFKA_BROKERS")
        .unwrap_or_else(|_| "localhost:9092".to_string());

    // Create producer
    let producer = Producer::new(&brokers)?;
    info!("Producer created successfully");

    // Send some user created events
    for i in 1..=5 {
        let user = UserCreated {
            user_id: format!("user-{}", i),
            email: format!("user{}@example.com", i),
            name: format!("User {}", i),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        info!("Sending UserCreated event for user_id: {}", user.user_id);
        producer.send(&user).await?;
        
        // Small delay between messages
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Send order events with keys for ordering guarantees
    for i in 1..=3 {
        let order = OrderPlaced {
            order_id: format!("order-{}", i),
            user_id: format!("user-{}", i),
            amount: 99.99 * i as f64,
            items: vec![
                format!("item-{}", i),
                format!("item-{}", i + 1),
            ],
        };

        info!("Sending OrderPlaced event for order_id: {}", order.order_id);
        // Use user_id as key to ensure all orders for the same user go to the same partition
        producer.send_with_key(&order, &order.user_id).await?;
        
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Flush any remaining messages
    info!("Flushing pending messages");
    producer.flush(Duration::from_secs(5))?;

    info!("All messages sent successfully!");
    Ok(())
}
