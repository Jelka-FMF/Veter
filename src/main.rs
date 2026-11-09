use tracing_subscriber::fmt::format::FmtSpan;
use warp::Filter;

use crate::config::Config;
use crate::metrics::init_metrics;
use crate::utils::rejection_handler;

mod config;
mod metrics;
mod routes;
mod utils;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_span_events(FmtSpan::CLOSE).init();

    let config = Config::new().expect("failed to load configuration");

    init_metrics();

    let base_routes = routes::base_routes();
    let state_routes = routes::state_routes(config.token.clone());
    let interaction_routes = routes::interaction_routes(config.token.clone());

    let routes = base_routes
        .or(state_routes)
        .or(interaction_routes)
        .recover(rejection_handler)
        .with(warp::trace::request());

    let server = warp::serve(routes);

    #[cfg(not(unix))]
    if config.socket.is_some() {
        panic!("UNIX sockets not supported on this platform");
    }

    #[cfg(unix)]
    if let Some(socket) = config.socket.as_deref() {
        use std::os::unix::fs::PermissionsExt;

        let (path, mode) = socket.rsplit_once('|').unwrap_or((socket, ""));

        if let Err(error) = tokio::net::UnixStream::connect(path).await {
            if error.kind() == std::io::ErrorKind::ConnectionRefused {
                std::fs::remove_file(path).unwrap_or_else(|error| {
                    panic!("failed to remove stale UNIX socket {path}: {error}");
                });
            }
        }

        let listener = tokio::net::UnixListener::bind(path);
        let listener = listener.unwrap_or_else(|error| {
            panic!("failed to bind to UNIX socket {path}: {error}");
        });

        if !mode.is_empty() {
            let mode = u32::from_str_radix(mode, 8)
                .unwrap_or_else(|error| panic!("failed to parse mode {mode}: {error}"));
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
                .unwrap_or_else(|error| panic!("failed to configure UNIX socket {mode}: {error}"));
        }

        tracing::info!("starting server on UNIX socket {path}");
        server.incoming(listener).run().await;
        return;
    }

    let listener = tokio::net::TcpListener::bind((config.address, config.port)).await;
    let listener = listener.unwrap_or_else(|error| {
        panic!("failed to bind to {}:{}: {error}", config.address, config.port);
    });

    tracing::info!("starting server on {}:{}", config.address, config.port);
    server.incoming(listener).run().await;
}
