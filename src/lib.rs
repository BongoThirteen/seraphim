//! eyes that see into the soul of your code
//!
//! # Overview
//!
//! Seraphim is a tool for visualizing and analyzing traces collected using the
//! [`tracing`] crate.
//! It allows you to save logs from your app and later access them by
//! connecting the CLI client to the integrated network server.
//!
//! # For apps
//!
//! To enable collection of your app's log data, just add a call to the
//! [`install`] function to the top of your `main` function:
//!
//! ```rust
//! seraphim::install();
//!
//! tracing::info!("Holy, holy, holy");
//! ```
//!
//! # For devs
//!
//! To view trace data from a running app, you must connect to the WebSocket
//! server `seraphim` is running.
//! The default address is `127.0.0.1:6580`.
//!
//! ```bash
//! seraphim connect
//! ```
//!
//! To specify an alternate address to connect to, use `--address`.
//!
//! ```bash
//! seraphim connect --address ws://127.0.0.1:6580
//! ```

mod layer;
mod store;

pub use layer::Seraphim;

/// Convenient way to quickly set up `seraphim`
pub fn install() {
    todo!("add install function")
}
