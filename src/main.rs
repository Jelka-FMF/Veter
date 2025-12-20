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

    let base_routes = routes::base_routes();
    let state_routes = routes::state_routes(config.token.clone());
    let interaction_routes = routes::interaction_routes(config.token.clone());

    let routes = base_routes
        .or(state_routes)
        .or(interaction_routes)
        .recover(rejection_handler)
        .with(warp::trace::request());

    tracing::info!("starting server on {}:{}", config.address, config.port);
    warp::serve(routes).run((config.address, config.port)).await;
}
