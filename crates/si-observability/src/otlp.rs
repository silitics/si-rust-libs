//! Export traces and metrics via OTLP.

use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Subscriber;
use tracing::level_filters::LevelFilter;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{EnvFilter, Layer};

use crate::Initializer;

/// Setup the [`Layer`] for exporting traces via OTLP.
pub(crate) fn setup_otlp_layer<S>(
    initializer: &Initializer,
) -> (Option<impl Layer<S>>, FinalizeGuard)
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    let log_env_var = format!("{}_LOG_OTLP", initializer.env_var_prefix);

    if std::env::var_os(&log_env_var).is_some() {
        let exporter = match opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .build()
        {
            Ok(exporter) => exporter,
            Err(error) => {
                eprintln!("ERROR: Unable to create OTLP exporter. {error}");
                return Default::default();
            }
        };
        let provider = opentelemetry_sdk::trace::SdkTracerProvider::builder()
            .with_resource(Resource::builder().build())
            .with_batch_exporter(exporter)
            .build();

        opentelemetry::global::set_tracer_provider(provider.clone());

        let tracer = provider.tracer("rust-tracing");
        let filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .with_env_var(&log_env_var)
            .from_env_lossy();
        let layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_filter(filter);

        (
            Some(layer),
            FinalizeGuard {
                provider: Some(provider),
            },
        )
    } else {
        Default::default()
    }
}

/// OTLP finalization guard.
///
/// This guard force flushes any outstanding traces.
#[derive(Debug, Default)]
pub(crate) struct FinalizeGuard {
    provider: Option<SdkTracerProvider>,
}

impl Drop for FinalizeGuard {
    fn drop(&mut self) {
        if let Some(provider) = &self.provider {
            if let Err(error) = provider.force_flush() {
                eprintln!("ERROR: Unable to flush traces via OTLP. {error}");
            }
        }
    }
}
