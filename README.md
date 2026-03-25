
# ꙮ

eyes that see into the soul of your code

Tokio's [`tracing`](https://github.com/tokio-rs/tracing) crate provides rich facilities for instrumenting code, but options for recording and displaying the traces it collects are limited.
Seraphim is a visualization and analysis tool that provides two components:
- A library that provides an implementation of `tracing-subscriber`'s [`Layer`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html) trait which saves logs in an embedded database and serves them over a WebSocket server.
- A client in the form of a CLI app that connects to the library's WebSocket server to view logs.

# Features

- [ ] Captures the full richness of `tracing` [spans](https://docs.rs/tracing/latest/tracing/#spans) and [events](https://docs.rs/tracing/latest/tracing/#events) with all their associated data.
- [ ] Stores traces in a compact embedded datastore.
- [ ] Hosts a [gRPC](https://github.com/hyperium/tonic) endpoint to retrieve traces remotely.
- [ ] CLI app downloads traces, analyzes them to extract insights into program execution and displays them in a user-friendly UI.
- [ ] Ability to query traces and surface important metrics.
