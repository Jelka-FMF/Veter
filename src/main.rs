use tracing_subscriber::fmt::format::FmtSpan;
use warp::Filter;

use crate::config::Config;
use crate::utils::rejection_handler;

mod config;
mod routes;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_span_events(FmtSpan::CLOSE).init();

    let config = Config::new().expect("failed to load configuration");

    let state = routes::state_routes(config.token.clone());

    let routes = state.recover(rejection_handler).with(warp::trace::request());

    tracing::info!("starting server on {}:{}", config.address, config.port);
    warp::serve(routes).run((config.address, config.port)).await;
}
