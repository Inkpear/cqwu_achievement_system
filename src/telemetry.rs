use tracing::{Subscriber, subscriber::set_global_default};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_log::LogTracer;
use tracing_subscriber::{EnvFilter, Layer, Registry, fmt::MakeWriter, layer::SubscriberExt};

use crate::configuration::LogSettings;

pub fn get_subscriber<Sink>(
    name: String,
    env_filter: String,
    sink: Sink,
    log_settings: Option<LogSettings>,
) -> (impl Subscriber + Send + Sync, Option<WorkerGuard>)
where
    Sink: for<'a> MakeWriter<'a> + Send + Sync + 'static,
{
    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(env_filter));
    let console_layer = BunyanFormattingLayer::new(name.clone(), sink).with_filter(env_filter);

    let (file_layer, guard) = if let Some(log_settings) = log_settings {
        let LogSettings {
            log_path,
            log_prefix,
            log_level,
        } = log_settings;

        let file_appender = tracing_appender::rolling::daily(log_path, log_prefix);
        let (non_blocking_file, guard) = tracing_appender::non_blocking(file_appender);

        let file_layer = BunyanFormattingLayer::new(name, non_blocking_file)
            .with_filter(EnvFilter::new(log_level));

        (Some(file_layer), Some(guard))
    } else {
        (None, None)
    };

    let subscriber = Registry::default()
        .with(JsonStorageLayer)
        .with(console_layer)
        .with(file_layer);

    (subscriber, guard)
}

pub fn init_subscriber(subscriber: impl Subscriber + Send + Sync) {
    LogTracer::init().expect("Failed to initialize ");
    set_global_default(subscriber).expect("Failed to set subscriber");
}
