//! CLI arguments for [`clap`] (version 4).

use clap4 as clap;
use tracing::level_filters::LevelFilter;

use crate::sealed::ConfigurationSealed;
use crate::{Configuration, StderrLogFormat};

/// Logging CLI arguments.
#[derive(Debug, Clone, clap::Parser)]
pub struct LoggingArgs {
    /// Log format.
    #[clap(long)]
    log_format: Option<LogFormatArg>,
    /// Log level.
    #[clap(long)]
    log_level: Option<LogLevelArg>,
}

impl ConfigurationSealed for LoggingArgs {}

impl Configuration for LoggingArgs {
    fn apply_to(&self, initializer: &mut crate::Initializer) {
        if let Some(log_format) = &self.log_format {
            initializer.stderr_logging_format = Some(match log_format {
                LogFormatArg::Compact => StderrLogFormat::Compact,
                LogFormatArg::Full => StderrLogFormat::Full,
            })
        }
        if let Some(log_level) = &self.log_level {
            initializer.stderr_default_level = match log_level {
                LogLevelArg::Off => LevelFilter::OFF,
                LogLevelArg::Error => LevelFilter::ERROR,
                LogLevelArg::Warn => LevelFilter::WARN,
                LogLevelArg::Info => LevelFilter::INFO,
                LogLevelArg::Debug => LevelFilter::DEBUG,
                LogLevelArg::Trace => LevelFilter::TRACE,
            };
        }
    }
}

/// Log format argument.
#[derive(Debug, Clone, clap::ValueEnum)]
enum LogFormatArg {
    /// Compact log format.
    Compact,
    /// Full log format.
    Full,
}

/// Log level argument.
#[derive(Debug, Clone, clap::ValueEnum)]
enum LogLevelArg {
    /// Turn logging off.
    Off,
    /// “Error” log level.
    Error,
    /// “Warn” log level.
    Warn,
    /// “Info” log level.
    Info,
    /// “Debug” log level.
    Debug,
    /// “Trace” log level.
    Trace,
}
