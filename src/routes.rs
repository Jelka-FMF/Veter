use std::sync::Arc;

use tokio::sync::broadcast;
use warp::Filter;

use crate::utils::{
    StreamReceiverExt,
    StreamTransmitterExt,
    authorization_auth,
    cors_any_origin,
    subprotocol_auth,
};

pub fn base_routes()
-> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    warp::path!("health").map(|| warp::reply::with_status("OK", warp::http::StatusCode::OK))
}

pub fn state_routes(
    token: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    let (channel, _) = broadcast::channel::<String>(16);
    let channel = Arc::new(channel);

    let transmitter = warp::path!("state" / "stream")
        .and_transmit_stream(channel.clone())
        .with(cors_any_origin());

    let receiver = warp::path!("state" / "push")
        .and(subprotocol_auth(token))
        .and_receive_stream(channel.clone());

    transmitter.or(receiver)
}

pub fn interaction_routes(
    token: Arc<String>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    let (channel, _) = broadcast::channel::<String>(16);
    let channel = Arc::new(channel);

    let transmitter = warp::path!("interaction" / "stream")
        .and(authorization_auth(token))
        .and_transmit_stream(channel.clone())
        .with(cors_any_origin());

    let receiver = warp::path!("interaction" / "push")
        // .and(subprotocol_auth(code))
        .and_receive_stream(channel.clone());

    transmitter.or(receiver)
}
