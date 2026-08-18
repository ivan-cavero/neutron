pub mod client;
pub mod metrics;
pub mod output;
pub mod scenarios;

pub use client::{BenchCollector, BotState};

use std::sync::OnceLock;

/// Initialize the global tracing/log subscriber exactly once per process.
///
/// Every azalea bot App carries a `bevy_log::LogPlugin` (azalea-client's
/// default `log` feature), and each App tries to install the *global* `log`
/// logger + `tracing` subscriber. The 2nd..Nth App therefore conflicts with
/// the 1st: on older bevy_log this panics ("a logger was already initialized"
/// -> process exit 101), on newer it errors and leaves the bots broken. We
/// own the global logger here (once) and disable `LogPlugin` in the bot Apps
/// (see `run_bot_thread`), so nothing fights over global logging state.
pub fn init_logging() {
    static ONCE: OnceLock<()> = OnceLock::new();
    ONCE.get_or_init(|| {
        use bevy_log::tracing_subscriber::util::SubscriberInitExt;
        use bevy_log::tracing_subscriber::EnvFilter;
        let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
        let _ = bevy_log::tracing_subscriber::fmt()
            .with_env_filter(filter)
            .try_init();
    });
}
