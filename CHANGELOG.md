# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-10-22

### Added

#### kafka-messages crate
- Initial release of `kafka-messages` crate
- `KafkaMessage` trait for type-safe message definitions
- Trait requires `Serialize`, `Deserialize`, `Send`, and `Sync`
- Compile-time topic association via `const TOPIC: &'static str`

#### kafka crate
- Initial release of `kafka` crate
- Type-safe `Producer` with generic `send<T: KafkaMessage>(&T)` method
- Support for sending messages with keys via `send_with_key()`
- Custom producer configuration support via `from_config()`
- Producer flush capability for graceful shutdown

#### Consumer Features
- `Consumer` with HashMap-based topic routing
- `topic_handlers!` macro for convenient handler registration
- Automatic JSON serialization/deserialization
- Retry logic with exponential backoff
- Configurable retry behavior (max retries, initial/max backoff)
- Automatic offset commit on successful message processing
- Manual commit mode (auto-commit disabled by default)
- Graceful shutdown support via `ShutdownHandle`

#### Error Handling
- Custom `KafkaError` enum with specific error types:
  - Kafka errors
  - Serialization/deserialization errors
  - Handler errors
  - Shutdown errors
- Proper error propagation and logging

#### Observability
- Integrated `tracing` support throughout
- Debug-level logging for message flow
- Info-level logging for lifecycle events
- Warn/error logging for failures and retries

#### Documentation
- Comprehensive README with quick start guide
- Getting Started guide with detailed examples
- Inline documentation for all public APIs
- Doc tests for all major features
- Integration tests demonstrating usage

#### Examples
- `producer.rs` - Complete producer example with multiple message types
- `consumer.rs` - Consumer example with graceful shutdown handling
- Both examples include tracing setup and error handling

#### Testing
- Integration test suite covering:
  - Message trait implementation
  - JSON serialization/deserialization
  - Handler macro functionality
  - Handler execution
  - Error handling

### Dependencies
- tokio 1.48.0 - Async runtime
- rdkafka 0.36.2 - Kafka client
- serde 1.0.228 - Serialization framework
- serde_json 1.0.145 - JSON serialization
- tracing 0.1.41 - Structured logging
- anyhow 1.0.100 - Error handling
- thiserror 1.0.69 - Error derive macros

### Security
- No known vulnerabilities in dependencies
- CodeQL analysis clean
- Manual commit mode by default for better message processing guarantees
