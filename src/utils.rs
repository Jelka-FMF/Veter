use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::broadcast::Sender;
use tokio_stream::once;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use warp::Filter;
use warp::sse::Event;
use warp::ws::Ws;

pub fn stream_sender(
    endpoint: impl Filter<Extract = (), Error = warp::Rejection> + Clone + Send,
    channel: Arc<Sender<String>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    endpoint.and(warp::get()).map(move || {
        let receiver = channel.subscribe();

        // We need to send around 2048 bytes initially
        let initial = once(Ok(Event::default().comment(" ".repeat(2048))));

        let broadcast = BroadcastStream::new(receiver).filter_map(|message| async {
            match message {
                Ok(data) => Some(Ok::<Event, warp::Error>(Event::default().data(data))),
                Err(BroadcastStreamRecvError::Lagged(n)) => {
                    tracing::warn!("client lagged by {} messages", n);
                    None
                }
            }
        });

        let stream = initial.chain(broadcast);

        warp::sse::reply(warp::sse::keep_alive().stream(stream))
    })
}

pub fn stream_receiver(
    endpoint: impl Filter<Extract = (), Error = warp::Rejection> + Clone + Send,
    channel: Arc<Sender<String>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
    endpoint.and(warp::ws()).map(move |ws: Ws| {
        let channel = channel.clone();

        ws.on_upgrade(move |socket| async move {
            let (_, mut rx) = socket.split();

            while let Some(result) = rx.next().await {
                let message = match result {
                    Ok(message) => message,
                    Err(_) => continue,
                };

                let data = match message.to_str() {
                    Ok(text) => text.to_string(),
                    Err(_) => continue,
                };

                let _ = channel.send(data);
            }
        })
    })
}
