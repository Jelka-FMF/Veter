use std::sync::Arc;

use tokio::sync::broadcast;
use warp::Filter;
use warp::http::Response;

use crate::metrics;
use crate::utils::{
    StreamReceiverExt,
    StreamTransmitterExt,
    authorization_auth,
    cors_any_origin,
    subprotocol_auth,
};

pub fn base_routes()
-> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    let health = warp::path!("health").map(|| "OK");

    let metrics = warp::path!("metrics").map(|| match metrics::gather_metrics() {
        Ok(buffer) => {
            Response::builder().header("content-type", prometheus::TEXT_FORMAT).body(buffer)
        }
        Err(error) => {
            tracing::error!("failed to gather metrics: {}", error);
            Response::builder().status(500).body(b"Failed to gather metrics".into())
        }
    });

    health.or(metrics)
}

pub fn state_routes(
    token: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    let (channel, _) = broadcast::channel::<String>(10);
    let channel = Arc::new(channel);

    let transmitter = warp::path!("state" / "stream")
        .and_transmit_stream(channel.clone(), "state")
        .with(cors_any_origin());

    let receiver = warp::path!("state" / "push")
        .and(subprotocol_auth(token))
        .and_receive_stream(channel.clone(), "state");

    transmitter.or(receiver)
}

pub fn interaction_routes(
    token: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    let (channel, _) = broadcast::channel::<String>(10);
    let channel = Arc::new(channel);

    let transmitter = warp::path!("interaction" / "stream")
        .and(authorization_auth(token))
        .and_transmit_stream(channel.clone(), "interaction")
        .with(cors_any_origin());

    let receiver = warp::path!("interaction" / "push")
        // .and(subprotocol_auth(code))
        .and_receive_stream(channel.clone(), "interaction");

    transmitter.or(receiver)
}
