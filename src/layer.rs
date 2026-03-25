//! Tracing [`Layer`](tracing_subscriber::layer::Layer) for `seraphim`

/// Tracing [`Layer`](tracing_subscriber::layer::Layer) that saves logs in a
/// database and serves them over a WebSocket server
#[derive(Debug, Clone)]
pub struct Seraphim {}
