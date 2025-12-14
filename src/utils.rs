use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::broadcast::Sender;
use tokio_stream::once;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use warp::Filter;
use warp::sse::Event;
use warp::ws::Ws;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum HttpError {
    InvalidToken,
    Conflict,
}

impl warp::reject::Reject for HttpError {}

pub trait StreamTransmitterExt {
    fn and_transmit_stream(
        self,
        channel: Arc<Sender<String>>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send;
}

impl<F: Filter<Extract = (), Error = warp::Rejection> + Clone + Send> StreamTransmitterExt for F {
    fn and_transmit_stream(
        self,
        channel: Arc<Sender<String>>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
        self.and(warp::get()).map(move || {
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
}

pub trait StreamReceiverExt {
    fn and_receive_stream(
        self,
        channel: Arc<Sender<String>>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send;
}

impl<F: Filter<Extract = (), Error = warp::Rejection> + Clone + Send> StreamReceiverExt for F {
    fn and_receive_stream(
        self,
        channel: Arc<Sender<String>>,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
        self.and(warp::ws()).map(move |ws: Ws| {
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
}

fn header_auth(
    header: &'static str,
    expected: String,
) -> impl Filter<Extract = (), Error = warp::Rejection> + Clone + Send {
    warp::header::optional::<String>(header)
        .and_then(move |header: Option<String>| match header {
            Some(value) if value == expected => std::future::ready(Ok(())),
            _ => std::future::ready(Err(warp::reject::custom(HttpError::InvalidToken))),
        })
        .untuple_one()
}

pub fn authorization_auth(
    token: Arc<String>,
) -> impl Filter<Extract = (), Error = warp::Rejection> + Clone + Send {
    header_auth("authorization", format!("Token {}", token))
}

pub fn subprotocol_auth(
    token: Arc<String>,
) -> impl Filter<Extract = (), Error = warp::Rejection> + Clone + Send {
    let header = "sec-websocket-protocol";
    let expected = format!("auth-{}", token);

    header_auth(header, expected)
}

pub async fn rejection_handler(
    error: warp::Rejection,
) -> Result<impl warp::Reply, warp::Rejection> {
    match error.find::<HttpError>() {
        Some(HttpError::InvalidToken) => Ok(warp::reply::with_status(
            "Invalid token provided",
            warp::http::StatusCode::UNAUTHORIZED,
        )),
        Some(HttpError::Conflict) => Ok(warp::reply::with_status(
            "Another client connected",
            warp::http::StatusCode::CONFLICT,
        )),
        None => Err(error),
    }
}
