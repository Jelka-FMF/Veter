use std::sync::Arc;

use tokio::sync::broadcast;
use warp::Filter;

use crate::utils::{StreamReceiverExt, StreamTransmitterExt, cors_any_origin, subprotocol_auth};

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
