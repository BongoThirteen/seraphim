
# ꙮ

eyes that see into the soul of your code

Tokio's [`tracing`](https://github.com/tokio-rs/tracing) crate provides rich facilities for instrumenting code, but options for recording and displaying the traces it collects are limited.
Seraphim is a visualization and analysis tool that provides two components:
- A library that provides an implementation of `tracing-subscriber`'s [`Layer`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/layer/trait.Layer.html) trait which saves logs in an embedded database and serves them over a TLS socket server or [`iroh`](https://docs.rs/iroh).
- A client in the form of a CLI app that connects to the library's server to view logs.

# Usage

Publication on [crates.io](https://crates.io) is planned but for now, you must clone this repository:
```bash
git clone https://github.com/BongoThirteen/seraphim.git
```
To run the example code that starts a server and logs one message:
```bash
cargo run --example log_net
```
To run the client binary:
```bash
cargo run 127.0.0.1:6580
```
Replace the default address `127.0.0.1:6580` with the address of your server if necessary.

# Features

- [x] Captures the full richness of `tracing` [spans](https://docs.rs/tracing/latest/tracing/#spans) and [events](https://docs.rs/tracing/latest/tracing/#events) with all their associated data.
- [x] Stores traces in a compact embedded datastore.
- [x] Hosts a server to retrieve traces remotely.
- [ ] CLI app downloads traces, analyzes them to extract insights into program execution and displays them in a user-friendly UI.
- [ ] Ability to query traces and surface important metrics.
