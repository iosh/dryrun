use std::io;

use tracing_subscriber::fmt::format::FmtSpan;

use crate::config::{LogFormat, TracingConfig};

pub(crate) fn init(config: &TracingConfig) -> io::Result<()> {
    let level: tracing::Level = config.level.parse().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("invalid tracing level: {}", config.level),
        )
    })?;
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(true)
        .with_span_events(FmtSpan::CLOSE);

    match config.format {
        LogFormat::Pretty => subscriber.pretty().try_init(),
        LogFormat::Json => subscriber.json().try_init(),
    }
    .map_err(io::Error::other)
}
