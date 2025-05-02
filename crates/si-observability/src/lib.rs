#![cfg_attr(docsrs, feature(doc_auto_cfg))]
//! This crate provides a reusable basis for developing applications with strong, built-in
//! observability.
//!
//! **Note: This crate is intended for internal use within applications developed by
//! [Silitics]. It is open-source and you are free to use it for whatever purpose you see
//! fit, however, we will not accept any contributions other than bug fixes.**
//!
//! [Silitics]: https://silitics.com
//!
//! At Silitics, we consider observability a fundamental aspect of building reliable and
//! maintainable systems. This crate builds upon the well-established [`tracing`]
//! ecosystem, which we rely on for instrumentation and structured logging. On top of
//! [`tracing`], this crate provides a reusable basis for configuration and logging
//! initialization—reducing boilerplate, promoting best practices, and supporting useful
//! log output for users at the console and the integration with observability platforms.
//! Most functionality provided by this crate is gated by feature flags and can be
//! configured via environment variables at runtime.
//!
//! The [`Initializer`] is the primary interface for setting everything up. Here is a
//! minimal example:
//!
//! ```
//! si_observability::Initializer::new("APP").init();
//! ```
//!
//! In this example, the string `APP` is an application-defined prefix for configuration
//! environment variables. In the following, we will use `APP` as a placeholder for the
//! application-defined prefix defined via the [`Initializer`].
//!
//! Additional configurations can be [applied][Initializer::apply] to the [`Initializer`]
//! via the sealed [`Configuration`] trait.
//!
//! Upon initialization, the [`Initializer`] returns a [`FinalizeGuard`] which must be
//! kept around for the lifetime of the application. When dropped, this guard will cleanup
//! resources and flush internal buffers, e.g., containing logs.
//!
//!
//! ## Logging to Stderr
//!
//! Logging to stderr is enabled by default for informational events. Applications should
//! use informational events to communicate status information to the user. Applications
//! should **not** use [`println!`] or [`eprintln!`] for that purpose.
//!
//! Logging to stderr can be configured via the `APP_LOG` environment variable using
//! [`EnvFilter`] directives.
//!
//! Logging to stderr produces colorful output using ANSI codes in accordance with the
//! [`clicolors` specification][clicolors].
//!
//! [clicolors]: https://bixense.com/clicolors/
//!
//! The environment variable `APP_LOG_FORMAT` can be set to one of the following log
//! formats:
//!
//! - `compact`: Compact format for everyday use (the default).
//! - `full`: Verbose format with additional information like timestamps and span
//!   attributes.
//!
//! In addition, an application may make logging to stderr configurable via standardized
//! command line arguments. Command line arguments have the advantage that they are
//! discoverable by users by calling the application with `--help`. To standardize the
//! respective arguments, this crate provides pre-made integrations with [`clap`][clap].
//! Here is an example:
//!
//! ```rust
//! # use clap4 as clap;
//! # use clap::Parser;
//! use si_observability::clap4::LoggingArgs;
//!
//! #[derive(Debug, Parser)]
//! pub struct AppArgs {
//!     #[clap(flatten)]
//!     logging: LoggingArgs,
//! }
//!
//! let args = AppArgs::parse();
//!
//! si_observability::Initializer::new("APP").apply(&args.logging).init();
//! ```
//!
//! [clap]: https://crates.io/crates/clap
//!
//! Note that we consider adding arguments with the prefix `--log-` a **non-breaking**
//! change.
//!
//!
//! ## OpenTelemetry
//!
//! When the `otlp` feature is enabled, this crate supports exporting traces via [OTLP] to
//! monitoring and observability tools. While primarily intended for monitoring cloud
//! applications, this can also be useful for local debugging.
//!
//! [OTLP]: https://opentelemetry.io/docs/specs/otel/protocol/
//!
//! At runtime, OTLP export is enabled and configured via the `APP_LOG_OTLP` environment
//! variable using [`EnvFilter`] directives. Additional environment variables, e.g., for
//! the configuration of OTLP endpoints and headers, follow the [OpenTelemetry standard].
//! At the moment, trace export is limited to the `http/protobuf` protocol.
//!
//! Use the variable `OTEL_RESOURCE_ATTRIBUTES` to set OpenTelemetry resource attributes.
//! For instance:
//!
//! ```plain
//! OTEL_RESOURCE_ATTRIBUTES="service.name=my-app,service.instance.id=my-app-instance-1"
//! ```
//!
//! [OpenTelemetry standard]: https://opentelemetry.io/docs/languages/sdk-configuration/otlp-exporter/
//!
//! For local development and debugging, you can run a [Jaeger] instance as follows:
//!
//! ```sh
//! docker run --rm -p 16686:16686 -p 4318:4318 jaegertracing/jaeger:latest
//! ```
//!
//! It then suffices to set `APP_LOG_OTLP=info` to send traces to Jaeger. To view the
//! traces, go to <http://localhost:16686>.
//!
//! [Jaeger]: https://www.jaegertracing.io/
//!
//!
//! ## Feature Flags
//!
//! This crate has the following feature flags:
//!
//! - `clap4`: Support for [`clap`][clap4](version 4) CLI arguments.
//! - `otlp`: Support for exporting traces via [OTLP].

use core::fmt;

use tracing::level_filters::LevelFilter;
use tracing::{Event, Subscriber};
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, format};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

mod sealed;

#[cfg(feature = "clap4")]
pub mod clap4;

#[cfg(feature = "otlp")]
mod otlp;

/// Observability initializer.
#[derive(Debug, Clone)]
pub struct Initializer {
    /// Application environment variable prefix.
    env_var_prefix: String,
    /// Default level used for logging to stderr.
    stderr_default_level: LevelFilter,
    /// Format used for logging to stderr.
    stderr_logging_format: Option<StderrLogFormat>,
}

impl Initializer {
    /// Create a new [`Initializer`] with the given environment variable prefix.
    pub fn new(env_var_prefix: &str) -> Self {
        Self {
            env_var_prefix: env_var_prefix.to_owned(),
            stderr_default_level: LevelFilter::INFO,
            stderr_logging_format: None,
        }
    }

    /// Apply a configuration to the [`Initializer`].
    pub fn apply(mut self, configuration: impl Configuration) -> Self {
        configuration.apply_to(&mut self);
        self
    }

    /// Initialize observability functionality.
    pub fn init(self) -> FinalizeGuard {
        let stderr_filter = EnvFilter::builder()
            .with_default_directive(self.stderr_default_level.into())
            .with_env_var(format!("{}_LOG", &self.env_var_prefix))
            .from_env_lossy();
        let stderr_format = self.stderr_logging_format.clone().unwrap_or_else(|| {
            let format_env_var = format!("{}_LOG_FORMAT", self.env_var_prefix);
            match std::env::var(&format_env_var).as_deref() {
                Ok("full") => StderrLogFormat::Full,
                Ok("compact") => StderrLogFormat::Compact,
                Ok(_) | Err(std::env::VarError::NotUnicode(_)) => {
                    eprintln!("WARNING: Unsupported log format in '{format_env_var}' environment variable.");
                    StderrLogFormat::Compact
                }
                _ => StderrLogFormat::Compact,
            }
        });
        let stderr_formatter = match stderr_format {
            StderrLogFormat::Compact => StderrLogFormatter::Compact(
                tracing_subscriber::fmt::format()
                    .without_time()
                    .with_ansi(console::colors_enabled_stderr())
                    .with_target(false)
                    .compact(),
            ),
            StderrLogFormat::Full => StderrLogFormatter::Full(
                tracing_subscriber::fmt::format().with_ansi(console::colors_enabled_stderr()),
            ),
        };
        let stderr_layer = tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .event_format(stderr_formatter)
            .with_filter(stderr_filter);

        let registry = tracing_subscriber::registry().with(stderr_layer);

        #[cfg(feature = "otlp")]
        let (registry, otlp_guard) = {
            let (otlp_layer, otlp_guard) = otlp::setup_otlp_layer(&self);
            (registry.with(otlp_layer), otlp_guard)
        };

        registry.init();

        FinalizeGuard {
            #[cfg(feature = "otlp")]
            _otlp_guard: otlp_guard,
        }
    }
}

/// Configuration that can be applied to an [`Initializer`].
pub trait Configuration: sealed::ConfigurationSealed {
    /// Apply the configuration to the given [`Initializer`].
    fn apply_to(&self, initializer: &mut Initializer);
}

impl<C: Configuration> Configuration for &C {
    fn apply_to(&self, initializer: &mut Initializer) {
        (**self).apply_to(initializer);
    }
}

/// Format for log messages written to stderr.
#[derive(Debug, Clone)]
enum StderrLogFormat {
    /// Compact format.
    Compact,
    /// Full format.
    Full,
}

/// Formatter for log messages written to stderr.
enum StderrLogFormatter {
    /// Compact format.
    Compact(tracing_subscriber::fmt::format::Format<tracing_subscriber::fmt::format::Compact, ()>),
    /// Full format.
    Full(tracing_subscriber::fmt::format::Format),
}

impl<S, N> FormatEvent<S, N> for StderrLogFormatter
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        writer: format::Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        // Simply delegate to the internal formatter provided by `tracing_subscriber`.
        match self {
            StderrLogFormatter::Compact(formatter) => formatter.format_event(ctx, writer, event),
            StderrLogFormatter::Full(formatter) => formatter.format_event(ctx, writer, event),
        }
    }
}

/// Finalization guard.
#[derive(Debug)]
#[must_use]
pub struct FinalizeGuard {
    #[cfg(feature = "otlp")]
    _otlp_guard: otlp::FinalizeGuard,
}

impl FinalizeGuard {
    /// Finalize everything by dropping the guard.
    pub fn finalize(self) {
        drop(self)
    }
}
