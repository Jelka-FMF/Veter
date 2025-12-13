use tracing_subscriber::fmt::format::FmtSpan;
use warp::Filter;

use crate::config::Config;

mod config;
mod routes;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_span_events(FmtSpan::CLOSE).init();

    let config = Config::new().unwrap();

    let state = routes::state_routes();

    let routes = state.with(warp::trace::request());

    tracing::info!("starting server on {}:{}", config.address, config.port);
    warp::serve(routes).run((config.address, config.port)).await;
}
