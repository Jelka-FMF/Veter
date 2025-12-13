use std::sync::Arc;

use tokio::sync::broadcast;
use warp::Filter;

use crate::utils;

pub fn state_routes()
-> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    let (channel, _) = broadcast::channel::<String>(16);
    let channel = Arc::new(channel);

    let sender = utils::stream_sender(warp::path!("state" / "stream"), channel.clone());
    let receiver = utils::stream_receiver(warp::path!("state" / "push"), channel.clone());

    sender.or(receiver)
}
