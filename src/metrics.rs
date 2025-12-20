use std::sync::LazyLock;
use std::time::Instant;

use prometheus::{
    Encoder,
    Gauge,
    IntCounterVec,
    IntGaugeVec,
    TextEncoder,
    register_gauge,
    register_int_counter_vec,
    register_int_gauge_vec,
};

// Publisher Metrics

pub static PUBLISHERS_ACTIVE: LazyLock<IntGaugeVec> = LazyLock::new(|| {
    register_int_gauge_vec!("veter_publishers_active", "Number of active publisher connections", &[
        "channel"
    ])
    .unwrap()
});

pub static PUBLISHERS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "veter_publishers_total",
        "Total number of publisher connection events",
        &["channel", "status"]
    )
    .unwrap()
});

// Subscriber Metrics

pub static SUBSCRIBERS_ACTIVE: LazyLock<IntGaugeVec> = LazyLock::new(|| {
    register_int_gauge_vec!(
        "veter_subscribers_active",
        "Number of active subscriber connections",
        &["channel"]
    )
    .unwrap()
});

pub static SUBSCRIBERS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "veter_subscribers_total",
        "Total number of subscriber connection events",
        &["channel", "status"]
    )
    .unwrap()
});

// Channel Metrics

pub static CHANNEL_MESSAGES_RECEIVED_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "veter_channel_messages_received_total",
        "Total number of messages received from publishers",
        &["channel"]
    )
    .unwrap()
});

pub static CHANNEL_MESSAGES_SENT_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "veter_channel_messages_sent_total",
        "Total number of messages sent to subscribers",
        &["channel"]
    )
    .unwrap()
});

pub static CHANNEL_CLIENT_LAG_EVENTS_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "veter_channel_client_lag_events_total",
        "Total number of client lag events",
        &["channel"]
    )
    .unwrap()
});

pub static CHANNEL_CLIENT_LAG_MESSAGES_TOTAL: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "veter_channel_client_lag_messages_total",
        "Total number of messages lost due to client lag",
        &["channel"]
    )
    .unwrap()
});

// Application Metrics

pub static APP_UPTIME_SECONDS: LazyLock<Gauge> = LazyLock::new(|| {
    register_gauge!("veter_app_uptime_seconds", "Application uptime in seconds").unwrap()
});

/// Application start time
static APP_START_TIME: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Update the uptime metric
pub fn update_uptime() {
    let uptime = APP_START_TIME.elapsed().as_secs_f64();
    APP_UPTIME_SECONDS.set(uptime);
}

/// Initialize all metrics with default values
pub fn init_metrics() {
    // Update uptime before initialization
    update_uptime();

    // Initialize publisher gauges to 0
    PUBLISHERS_ACTIVE.with_label_values(&["state"]).set(0);
    PUBLISHERS_ACTIVE.with_label_values(&["interaction"]).set(0);

    // Initialize subscriber gauges to 0
    SUBSCRIBERS_ACTIVE.with_label_values(&["state"]).set(0);
    SUBSCRIBERS_ACTIVE.with_label_values(&["interaction"]).set(0);
}

/// Generate metrics output in Prometheus text format
pub fn gather_metrics() -> Result<Vec<u8>, prometheus::Error> {
    // Update uptime before gathering metrics
    update_uptime();

    // Gather all metrics and encode them
    let encoder = TextEncoder::new();
    let families = prometheus::gather();
    let mut buffer = Vec::new();
    encoder.encode(&families, &mut buffer)?;

    Ok(buffer)
}

/// Actor type for connection tracking
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConnectionActor {
    Publisher,
    Subscriber,
}

/// Guard for tracking active connections
#[derive(Debug)]
pub struct ConnectionGuard {
    channel: &'static str,
    actor: ConnectionActor,
}

impl ConnectionGuard {
    pub fn new(channel: &'static str, actor: ConnectionActor) -> Self {
        match actor {
            ConnectionActor::Publisher => {
                PUBLISHERS_ACTIVE.with_label_values(&[channel]).inc();
                PUBLISHERS_TOTAL.with_label_values(&[channel, "connected"]).inc();
            }
            ConnectionActor::Subscriber => {
                SUBSCRIBERS_ACTIVE.with_label_values(&[channel]).inc();
                SUBSCRIBERS_TOTAL.with_label_values(&[channel, "connected"]).inc();
            }
        }

        Self { channel, actor }
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        match self.actor {
            ConnectionActor::Publisher => {
                PUBLISHERS_ACTIVE.with_label_values(&[self.channel]).dec();
                PUBLISHERS_TOTAL.with_label_values(&[self.channel, "disconnected"]).inc();
            }
            ConnectionActor::Subscriber => {
                SUBSCRIBERS_ACTIVE.with_label_values(&[self.channel]).dec();
                SUBSCRIBERS_TOTAL.with_label_values(&[self.channel, "disconnected"]).inc();
            }
        }
    }
}
