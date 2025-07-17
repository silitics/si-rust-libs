//! Export traces and metrics via OTLP.

use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};

use crate::Initializer;

/// Setup the [`Layer`] for exporting traces via OTLP.
pub(crate) fn setup_otlp_layer<S>(initializer: &Initializer) -> (impl Layer<S>, FinalizeGuard)
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let otlp_env_var = format!("{}_LOG_OTLP", initializer.env_var_prefix);
    let otlp_env_var_set = std::env::var_os(&otlp_env_var).is_some();

    #[cfg(feature = "otlp-traces")]
    let otlp_env_var_traces = format!("{}_LOG_OTLP_TRACES", initializer.env_var_prefix);
    #[cfg(feature = "otlp-traces")]
    let otlp_env_var_traces_set = std::env::var_os(&otlp_env_var_traces).is_some();

    #[cfg(feature = "otlp-logs")]
    let otlp_env_var_logs = format!("{}_LOG_OTLP_LOGS", initializer.env_var_prefix);
    #[cfg(feature = "otlp-logs")]
    let otlp_env_var_logs_set = std::env::var_os(&otlp_env_var_logs).is_some();

    #[cfg(feature = "otlp-traces")]
    let (tracer_layer, tracer_provider) = if otlp_env_var_set || otlp_env_var_traces_set {
        opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .build()
            .map(|exporter| {
                let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
                    .with_resource(Resource::builder().build())
                    .with_batch_exporter(exporter)
                    .build();
                let tracer = provider.tracer("rust-tracing");
                let filter = EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .with_env_var(if otlp_env_var_traces_set {
                        &otlp_env_var_traces
                    } else {
                        &otlp_env_var
                    })
                    .from_env_lossy();
                let layer = tracing_opentelemetry::layer()
                    .with_tracer(tracer)
                    .with_filter(filter);
                (Some(layer), Some(provider))
            })
            .inspect_err(|error| eprintln!("ERROR: Unable to create OTLP trace exporter. {error}"))
            .unwrap_or_default()
    } else {
        (None, None)
    };

    #[cfg(feature = "otlp-logs")]
    let (logger_layer, logger_provider) = if otlp_env_var_set || otlp_env_var_logs_set {
        opentelemetry_otlp::LogExporter::builder()
            .with_http()
            .build()
            .map(|exporter| {
                let provider = opentelemetry_sdk::logs::SdkLoggerProvider::builder()
                    .with_resource(Resource::builder().build())
                    .with_batch_exporter(exporter)
                    .build();
                // Avoid telemetry loop caused by log messages emitted by the exporter.
                // https://github.com/open-telemetry/opentelemetry-rust/issues/2877
                let filter = EnvFilter::builder()
                    .with_default_directive(LevelFilter::INFO.into())
                    .with_env_var(if otlp_env_var_logs_set {
                        &otlp_env_var_logs
                    } else {
                        &otlp_env_var
                    })
                    .from_env_lossy()
                    .add_directive("hyper=off".parse().unwrap())
                    .add_directive("opentelemetry=off".parse().unwrap())
                    .add_directive("tonic=off".parse().unwrap())
                    .add_directive("h2=off".parse().unwrap())
                    .add_directive("reqwest=off".parse().unwrap());
                (
                    Some(
                        opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                            &provider,
                        )
                        .with_filter(filter),
                    ),
                    Some(provider),
                )
            })
            .inspect_err(|error| {
                eprintln!("ERROR: Unable to create OTLP logs exporter. {error}");
            })
            .unwrap_or_default()
    } else {
        (None, None)
    };

    #[cfg(all(feature = "otlp-traces", feature = "otlp-logs"))]
    let layer = Layer::and_then(tracer_layer, logger_layer);
    #[cfg(all(feature = "otlp-traces", not(feature = "otlp-logs")))]
    let layer = tracer_layer;
    #[cfg(all(not(feature = "otlp-traces"), feature = "otlp-logs"))]
    let layer = logger_layer;

    (
        layer,
        FinalizeGuard {
            #[cfg(feature = "otlp-traces")]
            tracer_provider,
            #[cfg(feature = "otlp-logs")]
            logger_provider,
        },
    )
}

/// OTLP finalization guard.
///
/// This guard force flushes any outstanding traces.
#[derive(Debug, Default)]
pub(crate) struct FinalizeGuard {
    #[cfg(feature = "otlp-traces")]
    tracer_provider: Option<SdkTracerProvider>,
    #[cfg(feature = "otlp-logs")]
    logger_provider: Option<SdkLoggerProvider>,
}

impl Drop for FinalizeGuard {
    fn drop(&mut self) {
        #[cfg(feature = "otlp-traces")]
        if let Some(provider) = &self.tracer_provider {
            if let Err(error) = provider.force_flush() {
                eprintln!("ERROR: Unable to flush traces via OTLP. {error}");
            }
        }
        #[cfg(feature = "otlp-logs")]
        if let Some(provider) = &self.logger_provider {
            if let Err(error) = provider.force_flush() {
                eprintln!("ERROR: Unable to flush logs via OTLP. {error}");
            }
        }
    }
}
