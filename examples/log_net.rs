use std::{thread::sleep, time::Duration};

use tracing::info;

fn main() {
    seraphim::install_net();

    info!("Holy, holy, holy");

    sleep(Duration::from_secs(60));
}
