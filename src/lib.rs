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

#[cfg(feature = "iroh")]
pub mod net_iroh;

pub mod net;

pub use layer::Seraphim;

/// Convenient way to quickly set up `seraphim`
///
/// Enables the [`Seraphim`] layer and starts a server
#[cfg(feature = "iroh")]
pub fn install() {
    use std::sync::{Arc, Mutex};

    use store::Store;
    use tokio::{spawn, sync::broadcast::channel};
    use tracing_subscriber::{Registry, layer::SubscriberExt, util::SubscriberInitExt};

    async fn run_server(proto: net_iroh::SeraphimProtocol) {
        use std::error::Error;

        use iroh::{Endpoint, endpoint::presets::N0, protocol::Router};

        async fn run_server_inner(proto: net_iroh::SeraphimProtocol) -> Result<(), Box<dyn Error>> {
            use postcard::to_stdvec;
            use tokio::signal::ctrl_c;

            let ep = Endpoint::bind(N0).await?;

            ep.online().await;
            let addr = ep.addr();
            let addr_bytes = to_stdvec(&addr)?;

            println!("{}", z32::encode(&addr_bytes));

            let _router = Router::builder(ep).accept(net_iroh::ALPN, proto).spawn();

            ctrl_c().await?;

            Ok(())
        }

        if let Err(err) = run_server_inner(proto).await {
            eprintln!("Error: {err}");
        }
    }

    let (send, recv) = channel(64);
    let store = Arc::new(Mutex::new(Store::in_memory(send)));
    Registry::default()
        .with(Seraphim::new(store.clone()))
        .init();

    spawn(run_server(net_iroh::SeraphimProtocol::new(store, recv)));
}

/// All-in-one logging setup
///
/// Enables the [`Seraphim`] layer, starts a server, logs to `seraphim.log`,
/// saves the `iroh` private key to `log_id.key` and writes the `iroh` endpoint
/// ID to `log_id.txt`.
#[cfg(feature = "iroh")]
pub fn install_iroh() {
    use std::sync::{Arc, Mutex};

    use store::Store;
    use tokio::{spawn, sync::broadcast::channel};
    use tracing_subscriber::{Registry, layer::SubscriberExt, util::SubscriberInitExt};

    async fn run_server(proto: net_iroh::SeraphimProtocol) {
        use std::error::Error;

        use iroh::{Endpoint, endpoint::presets::N0, protocol::Router};

        async fn run_server_inner(proto: net_iroh::SeraphimProtocol) -> Result<(), Box<dyn Error>> {
            use std::fs::{exists, read, write};

            use iroh::SecretKey;
            use postcard::to_stdvec;
            use tokio::signal::ctrl_c;

            let mut ep = Endpoint::builder(N0);

            if exists("log_id.key")? {
                ep = ep.secret_key(SecretKey::from_bytes(
                    read("log_id.key")?.as_slice().try_into()?,
                ));
            } else {
                use rand_core::{OsRng, UnwrapErr};

                let key = SecretKey::generate(&mut UnwrapErr(OsRng));
                write("log_id.key", &key.to_bytes())?;
                ep = ep.secret_key(key);
            }

            let ep = ep.bind().await?;

            ep.online().await;
            let addr = ep.addr();
            let addr_bytes = to_stdvec(&addr)?;

            let addr_str = format!("{}", z32::encode(&addr_bytes));
            write("log_id.txt", addr_str.as_bytes())?;

            let _router = Router::builder(ep).accept(net_iroh::ALPN, proto).spawn();

            ctrl_c().await?;

            Ok(())
        }

        if let Err(err) = run_server_inner(proto).await {
            eprintln!("Error: {err}");
        }
    }

    let (send, recv) = channel(64);
    let store = match Store::open("seraphim.log", send) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("Failed to open `seraphim.log` ({err:#})");
            return;
        }
    };
    let store = Arc::new(Mutex::new(store));
    Registry::default()
        .with(Seraphim::new(store.clone()))
        .init();

    spawn(run_server(net_iroh::SeraphimProtocol::new(store, recv)));
}

#[cfg(feature = "net")]
pub fn install_net() {
    use std::{
        net::TcpListener,
        sync::{Arc, Mutex},
    };

    use tokio::sync::broadcast::channel;
    use tracing_subscriber::{Registry, layer::SubscriberExt, util::SubscriberInitExt};

    use crate::{net::serve, store::Store};

    let listener = match TcpListener::bind("127.0.0.1:6580") {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("Seraphim failed to bind to `127.0.0.1:6580` ({err:#})");
            return;
        }
    };

    let (send, recv) = channel(64);
    let store = match Store::open("seraphim.log", send) {
        Ok(store) => store,
        Err(err) => {
            println!("Seraphim failed to open storage at `seraphim.log` ({err:#})");
            return;
        }
    };
    let store = Arc::new(Mutex::new(store));

    Registry::default()
        .with(Seraphim::new(store.clone()))
        .init();

    serve(store, recv, listener);
}
