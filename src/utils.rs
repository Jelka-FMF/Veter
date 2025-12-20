use std::sync::Arc;

use futures_util::StreamExt;
use tokio::sync::broadcast::Sender;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use warp::Filter;
use warp::http::Method;
use warp::sse::Event;
use warp::ws::Ws;

use crate::metrics::{
    CHANNEL_CLIENT_LAG_EVENTS_TOTAL,
    CHANNEL_CLIENT_LAG_MESSAGES_TOTAL,
    CHANNEL_MESSAGES_RECEIVED_TOTAL,
    CHANNEL_MESSAGES_SENT_TOTAL,
    ConnectionActor,
    ConnectionGuard,
};

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
        name: &'static str,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send;
}

impl<F: Filter<Extract = (), Error = warp::Rejection> + Clone + Send> StreamTransmitterExt for F {
    fn and_transmit_stream(
        self,
        channel: Arc<Sender<String>>,
        name: &'static str,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
        self.and(warp::get()).map(move || {
            // Subscribe to the broadcast channel
            let receiver = channel.subscribe();

            // Track active SSE connections
            let _guard = ConnectionGuard::new(name, ConnectionActor::Subscriber);

            // Cache metric instances to avoid repeated label lookups
            let sent_counter = CHANNEL_MESSAGES_SENT_TOTAL.with_label_values(&[name]);
            let lag_events_counter = CHANNEL_CLIENT_LAG_EVENTS_TOTAL.with_label_values(&[name]);
            let lag_messages_counter = CHANNEL_CLIENT_LAG_MESSAGES_TOTAL.with_label_values(&[name]);

            // Bundle state for the stream processor
            struct State {
                broadcast: BroadcastStream<String>,
                sent_counter: prometheus::core::GenericCounter<prometheus::core::AtomicU64>,
                lag_events_counter: prometheus::core::GenericCounter<prometheus::core::AtomicU64>,
                lag_messages_counter: prometheus::core::GenericCounter<prometheus::core::AtomicU64>,
                _guard: ConnectionGuard,
            }

            let state = State {
                broadcast: BroadcastStream::new(receiver),
                sent_counter,
                lag_events_counter,
                lag_messages_counter,
                _guard,
            };

            // We need to send around 2048 bytes initially
            let initial = futures_util::stream::once(async {
                Ok::<Event, warp::Error>(Event::default().comment(" ".repeat(2048)))
            });

            let stream = futures_util::stream::unfold(state, |mut state| async move {
                state.broadcast.next().await.map(|result| {
                    let event = match result {
                        Ok(data) => {
                            // Track sent messages
                            state.sent_counter.inc();

                            // Send the message as an SSE event
                            Some(Ok(Event::default().data(data)))
                        }
                        Err(BroadcastStreamRecvError::Lagged(n)) => {
                            // Client has lagged behind
                            tracing::warn!("client lagged by {} messages", n);

                            // Track lag events and messages
                            state.lag_events_counter.inc();
                            state.lag_messages_counter.inc_by(n);

                            // Skip this event
                            None
                        }
                    };
                    (event, state)
                })
            })
            .filter_map(|opt| async move { opt });

            warp::sse::reply(warp::sse::keep_alive().stream(initial.chain(stream)))
        })
    }
}

pub trait StreamReceiverExt {
    fn and_receive_stream(
        self,
        channel: Arc<Sender<String>>,
        name: &'static str,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send;
}

impl<F: Filter<Extract = (), Error = warp::Rejection> + Clone + Send> StreamReceiverExt for F {
    fn and_receive_stream(
        self,
        channel: Arc<Sender<String>>,
        name: &'static str,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone + Send {
        self.and(warp::ws()).map(move |ws: Ws| {
            let channel = channel.clone();

            ws.on_upgrade(move |socket| async move {
                // Track active WebSocket connections
                let _guard = ConnectionGuard::new(name, ConnectionActor::Publisher);

                // Cache metric instance to avoid repeated label lookups
                let received_counter = CHANNEL_MESSAGES_RECEIVED_TOTAL.with_label_values(&[name]);

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

                    // Track message received
                    received_counter.inc();

                    // Broadcast the received message
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
    header_auth("sec-websocket-protocol", format!("auth-{}", token))
}

pub fn cors_any_origin() -> warp::cors::Builder {
    warp::cors().allow_methods(&[Method::GET]).allow_any_origin()
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
