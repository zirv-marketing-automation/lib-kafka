//! Integration tests for the Kafka library.

use kafka::{topic_handlers, KafkaMessage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestMessage {
    id: String,
    value: i32,
}

impl KafkaMessage for TestMessage {
    const TOPIC: &'static str = "test.topic";
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AnotherMessage {
    name: String,
}

impl KafkaMessage for AnotherMessage {
    const TOPIC: &'static str = "another.topic";
}

#[tokio::test]
async fn test_kafka_message_trait() {
    // Verify that the TOPIC constant is accessible
    assert_eq!(TestMessage::TOPIC, "test.topic");
    assert_eq!(AnotherMessage::TOPIC, "another.topic");
}

#[tokio::test]
async fn test_message_serialization() {
    let msg = TestMessage {
        id: "test-123".to_string(),
        value: 42,
    };

    // Serialize
    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("test-123"));
    assert!(json.contains("42"));

    // Deserialize
    let deserialized: TestMessage = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "test-123");
    assert_eq!(deserialized.value, 42);
}

#[tokio::test]
async fn test_topic_handlers_macro() {
    async fn handle_test(msg: TestMessage) -> anyhow::Result<()> {
        assert_eq!(msg.id, "test");
        Ok(())
    }

    async fn handle_another(msg: AnotherMessage) -> anyhow::Result<()> {
        assert_eq!(msg.name, "another");
        Ok(())
    }

    let handlers = topic_handlers![
        TestMessage => handle_test,
        AnotherMessage => handle_another,
    ];

    // Verify handlers are registered for the correct topics
    assert!(handlers.contains_key("test.topic"));
    assert!(handlers.contains_key("another.topic"));
    assert_eq!(handlers.len(), 2);
}

#[tokio::test]
async fn test_handler_execution() {
    async fn handle_message(msg: TestMessage) -> anyhow::Result<()> {
        assert!(!msg.id.is_empty());
        Ok(())
    }

    let handlers = topic_handlers![
        TestMessage => handle_message,
    ];

    // Get the handler for our topic
    let handler = handlers.get("test.topic").unwrap();

    // Create a test message
    let msg = TestMessage {
        id: "test-456".to_string(),
        value: 100,
    };
    let payload = serde_json::to_vec(&msg).unwrap();

    // Execute the handler
    let result = handler(&payload).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_handler_with_invalid_json() {
    async fn handle_message(_msg: TestMessage) -> anyhow::Result<()> {
        Ok(())
    }

    let handlers = topic_handlers![
        TestMessage => handle_message,
    ];

    let handler = handlers.get("test.topic").unwrap();

    // Invalid JSON should result in deserialization error
    let invalid_json = b"not valid json";
    let result = handler(invalid_json).await;
    assert!(result.is_err());
}
