use std::time::Duration;

use clap4::{self as clap, Parser};
use tracing::Level;

use si_observability::clap4::LoggingArgs;

#[derive(Debug, Parser)]
pub struct AppArgs {
    #[clap(flatten)]
    logging: LoggingArgs,
}

#[tracing::instrument(level = Level::INFO)]
pub fn do_stuff(iterations: u64) {
    tracing::info!(iterations, "Doing stuff for {iterations} iterations.");
    for iteration in 0..iterations {
        tracing::debug!(iteration, "Actually getting something done!");
        std::thread::sleep(Duration::from_millis(50));
        do_stuff_inner()
    }
}

#[tracing::instrument(level = Level::DEBUG)]
pub fn do_stuff_inner() {
    tracing::debug!("Doing some internal stuff!");
    std::thread::sleep(Duration::from_millis(50));
    tracing::error!("Oops! Something went wrong.");
}

pub fn main() {
    let args = AppArgs::parse();

    let _guard = si_observability::Initializer::new("APP")
        .apply(&args.logging)
        .init();

    do_stuff(5);
}
