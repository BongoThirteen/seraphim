use std::{
    error::Error,
    fs::{exists, read, write},
    sync::{Arc, Mutex},
    time::Duration,
};

use iroh::{Endpoint, SecretKey, endpoint::presets::N0, protocol::Router};
use postcard::to_stdvec;
use rand_core::{OsRng, UnwrapErr};
use tokio::main;
use tokio::{sync::broadcast::channel, time::sleep};
use tracing::info;
use tracing_subscriber::{Registry, layer::SubscriberExt, util::SubscriberInitExt};

use seraphim::{Seraphim, net::SeraphimProtocol, store::Store};

#[main]
async fn main() -> Result<(), Box<dyn Error>> {
    let (send, recv) = channel(64);
    let store = Arc::new(Mutex::new(Store::open("example.log", send)?));
    Registry::default()
        .with(Seraphim::new(store.clone()))
        .init();

    let mut ep = Endpoint::builder(N0);

    if exists("log_identity.key")? {
        ep = ep.secret_key(SecretKey::from_bytes(
            read("log_identity.key")?.as_slice().try_into()?,
        ));
    } else {
        let key = SecretKey::generate(&mut UnwrapErr(OsRng));
        write("log_identity.key", &key.to_bytes())?;
        ep = ep.secret_key(key);
    }

    let ep = ep.bind().await?;

    ep.online().await;
    let addr = ep.addr();
    let addr_bytes = to_stdvec(&addr)?;

    println!("{}", z32::encode(&addr_bytes));

    let _router = Router::builder(ep)
        .accept(seraphim::net::ALPN, SeraphimProtocol::new(store, recv))
        .spawn();

    loop {
        info!("Holy, holy, holy");
        sleep(Duration::from_secs(1)).await;
    }
}
