use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with file output to `<workspace>/logs/` and stderr.
/// Returns the guard that must be kept alive for the lifetime of the app.
pub fn init_tracing(log_dir: &PathBuf) -> WorkerGuard {
    std::fs::create_dir_all(log_dir).ok();

    let file_appender = tracing_appender::rolling::daily(log_dir, "slicer.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true);

    let stderr_layer = fmt::layer()
        .with_writer(std::io::stderr)
        .with_target(false);

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .init();

    tracing::info!(target: "diagnostics", "诊断日志系统已初始化，日志目录: {}", log_dir.display());

    guard
}

/// Create a tracing span with a correlation_id for error tracking.
pub fn correlation_span(correlation_id: &str) -> tracing::Span {
    tracing::info_span!("error_context", correlation_id = %correlation_id)
}
