use std::path::PathBuf;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Configuration for Cauldron's logging subsystem.
pub struct LogConfig {
    /// Directory where log files are written.
    pub log_dir: PathBuf,
    /// Minimum level for console output (e.g., "info", "debug", "trace").
    pub console_level: String,
    /// Minimum level for file output.
    pub file_level: String,
    /// Whether to emit logs to a file.
    pub log_to_file: bool,
    /// Whether to emit logs to the console (stderr).
    pub log_to_console: bool,
}

impl Default for LogConfig {
    fn default() -> Self {
        let log_dir = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cauldron")
            .join("logs");

        Self {
            log_dir,
            console_level: "info".to_string(),
            file_level: "debug".to_string(),
            log_to_file: true,
            log_to_console: true,
        }
    }
}

/// Guard that keeps the non-blocking log writer alive.
/// When dropped, the writer flushes any remaining buffered log entries.
pub struct LogGuard {
    _guard: WorkerGuard,
}

/// Initialize the global tracing subscriber with console and file layers.
///
/// The `CAULDRON_LOG` environment variable can override the filter at runtime,
/// e.g. `CAULDRON_LOG=trace` or `CAULDRON_LOG=cauldron_core=debug,info`.
///
/// Returns a [`LogGuard`] that **must** be held for the lifetime of the
/// program; dropping it flushes the file appender.
pub fn init_logging(config: &LogConfig) -> Result<LogGuard, Box<dyn std::error::Error>> {
    // Ensure the log directory exists.
    std::fs::create_dir_all(&config.log_dir)?;

    // Set up a daily-rotating file appender.
    let file_appender = tracing_appender::rolling::daily(&config.log_dir, "cauldron.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Build a combined filter from env or defaults.
    let env_filter = EnvFilter::try_from_env("CAULDRON_LOG")
        .unwrap_or_else(|_| EnvFilter::new(&config.file_level));

    // File layer: JSON format for machine parsing.
    let file_layer = fmt::layer()
        .json()
        .with_writer(non_blocking)
        .with_target(true)
        .with_file(true)
        .with_line_number(true);

    if config.log_to_console {
        // Console layer: human-readable.
        let console_layer = fmt::layer()
            .with_target(true)
            .with_file(true)
            .with_line_number(true);

        tracing_subscriber::registry()
            .with(file_layer)
            .with(console_layer)
            .with(env_filter)
            .init();
    } else {
        tracing_subscriber::registry()
            .with(file_layer)
            .with(env_filter)
            .init();
    }

    Ok(LogGuard { _guard: guard })
}
