use tokio::{main, signal::ctrl_c};
use tracing::info;

#[main]
async fn main() {
    seraphim::aio_iroh();

    info!("Holy, holy, holy");

    ctrl_c().await.unwrap();
}
