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
pub mod store;
pub mod types;

#[cfg(feature = "net")]
pub mod net;

pub use layer::Seraphim;

/// Convenient way to quickly set up `seraphim`
///
/// Enables the [`Seraphim`] layer and starts a server
#[cfg(feature = "net")]
pub fn install() {
    use std::sync::{Arc, Mutex};

    use store::Store;
    use tokio::{spawn, sync::broadcast::channel};
    use tracing_subscriber::{Registry, layer::SubscriberExt, util::SubscriberInitExt};

    let (send, recv) = channel(64);
    let store = Arc::new(Mutex::new(Store::in_memory(send)));
    Registry::default()
        .with(Seraphim::new(store.clone()))
        .init();

    spawn(run_server(net::SeraphimProtocol::new(store, recv)));
}

#[cfg(feature = "net")]
async fn run_server(proto: net::SeraphimProtocol) {
    use std::error::Error;

    use iroh::{Endpoint, endpoint::presets::N0, protocol::Router};

    async fn run_server_inner(proto: net::SeraphimProtocol) -> Result<(), Box<dyn Error>> {
        use postcard::to_stdvec;
        use tokio::signal::ctrl_c;

        let ep = Endpoint::bind(N0).await?;

        ep.online().await;
        let addr = ep.addr();
        let addr_bytes = to_stdvec(&addr)?;

        println!("{}", z32::encode(&addr_bytes));

        let _router = Router::builder(ep).accept(net::ALPN, proto).spawn();

        ctrl_c().await?;

        Ok(())
    }

    if let Err(err) = run_server_inner(proto).await {
        eprintln!("Error: {err}");
    }
}
