//! OpenTelemetry pipeline.
//!
//! Owns the process-wide bridge between instrumentation and OTLP export so
//! module code never touches exporter machinery. Modules emit `tracing` spans
//! and events and read instruments from the global meter, and this module
//! translates: spans through [`tracing_opentelemetry`], events through
//! [`opentelemetry_appender_tracing`], metrics through the global meter
//! provider. When export is disabled only the stdout `fmt` layer is installed
//! and every instrument is a no-op.

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use axum::http::{Request, Response};
use axum_otel_metrics::{HttpMetricsLayer, HttpMetricsLayerBuilder, PathSkipper};
use opentelemetry::{global, trace::TracerProvider as _};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig as _};
use opentelemetry_resource_detectors::{
    HostResourceDetector, OsResourceDetector, ProcessResourceDetector,
};
use opentelemetry_sdk::{
    Resource, error::OTelSdkResult, logs::SdkLoggerProvider, metrics::SdkMeterProvider,
    propagation::TraceContextPropagator, trace::SdkTracerProvider,
};
use tower::{Layer, Service};
use tracing::{Instrument as _, field::Empty, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::{
    EnvFilter, Layer as _,
    filter::{LevelFilter, Targets},
    layer::SubscriberExt as _,
    util::SubscriberInitExt as _,
};

use crate::config::{OtlpProtocol, TelemetryConfig};

/// Handle to the running exporters.
///
/// Call [`shutdown`](Self::shutdown) after the server has drained so buffered
/// telemetry is flushed before exit.
pub(crate) struct Telemetry {
    providers: Option<Providers>,
}

struct Providers {
    tracer: SdkTracerProvider,
    meter: SdkMeterProvider,
    logger: SdkLoggerProvider,
}

/// Installs the global `tracing` subscriber and, when export is enabled, the
/// OpenTelemetry providers behind it.
pub(crate) fn init(config: TelemetryConfig) -> anyhow::Result<Telemetry> {
    // RUST_LOG decides what is observed at all, for stdout and export alike.
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let fmt_layer = tracing_subscriber::fmt::layer();

    if !config.enabled {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
        return Ok(Telemetry { providers: None });
    }

    let protocol = config.otlp_protocol;
    let endpoint = config.otlp_endpoint;

    // Detectors stamp process, OS, and host attributes onto every signal.
    let resource = Resource::builder()
        .with_detectors(&[
            Box::new(ProcessResourceDetector),
            Box::new(OsResourceDetector),
            Box::new(HostResourceDetector::default()),
        ])
        .with_service_name(config.service_name)
        .build();

    let span_exporter = match protocol {
        OtlpProtocol::Grpc => SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()?,
        OtlpProtocol::HttpProtobuf => SpanExporter::builder()
            .with_http()
            .with_endpoint(http_signal_url(&endpoint, "traces"))
            .build()?,
    };
    let metric_exporter = match protocol {
        OtlpProtocol::Grpc => MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()?,
        OtlpProtocol::HttpProtobuf => MetricExporter::builder()
            .with_http()
            .with_endpoint(http_signal_url(&endpoint, "metrics"))
            .build()?,
    };
    let log_exporter = match protocol {
        OtlpProtocol::Grpc => LogExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .build()?,
        OtlpProtocol::HttpProtobuf => LogExporter::builder()
            .with_http()
            .with_endpoint(http_signal_url(&endpoint, "logs"))
            .build()?,
    };

    let tracer = SdkTracerProvider::builder()
        .with_batch_exporter(span_exporter)
        .with_resource(resource.clone())
        .build();
    let meter = SdkMeterProvider::builder()
        .with_periodic_exporter(metric_exporter)
        .with_resource(resource.clone())
        .build();
    let logger = SdkLoggerProvider::builder()
        .with_batch_exporter(log_exporter)
        .with_resource(resource)
        .build();

    global::set_text_map_propagator(TraceContextPropagator::new());
    global::set_tracer_provider(tracer.clone());
    global::set_meter_provider(meter.clone());

    // The exporters emit tracing events themselves while shipping batches
    // over gRPC. Exporting those would feed the pipeline its own output,
    // so the log bridge drops the whole transport stack.
    let export_stack_off = Targets::new()
        .with_default(LevelFilter::TRACE)
        .with_target("opentelemetry", LevelFilter::OFF)
        .with_target("opentelemetry_sdk", LevelFilter::OFF)
        .with_target("opentelemetry_otlp", LevelFilter::OFF)
        .with_target("tonic", LevelFilter::OFF)
        .with_target("h2", LevelFilter::OFF)
        .with_target("hyper", LevelFilter::OFF)
        .with_target("tower", LevelFilter::OFF);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(tracing_opentelemetry::layer().with_tracer(tracer.tracer("quiz-arena-backend")))
        .with(OpenTelemetryTracingBridge::new(&logger).with_filter(export_stack_off))
        .init();

    Ok(Telemetry {
        providers: Some(Providers {
            tracer,
            meter,
            logger,
        }),
    })
}

fn http_signal_url(endpoint: &str, signal: &str) -> String {
    format!("{}/v1/{signal}", endpoint.trim_end_matches('/'))
}

fn is_reflection(path: &str) -> bool {
    path.starts_with("/grpc.reflection.")
}

/// Splits `/quiz_arena.greet.v1.GreetService/Greet` into service and method.
fn rpc_path(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix('/')?.split_once('/')
}

/// Opens the server span for each Connect RPC, following the OpenTelemetry
/// RPC semantic conventions: spans are named `package.Service/Method` and
/// carry `rpc.*` attributes parsed from the request path.
///
/// Also adopts the caller's trace context from the request headers and
/// injects the span's context into the response headers so callers can
/// correlate. Reflection calls are tooling chatter, not application traffic,
/// so they pass through untraced, as do paths that are not RPCs at all.
#[derive(Clone)]
pub(crate) struct ConnectSpanLayer;

impl<S> Layer<S> for ConnectSpanLayer {
    type Service = ConnectSpanService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ConnectSpanService { inner }
    }
}

#[derive(Clone)]
pub(crate) struct ConnectSpanService<S> {
    inner: S,
}

impl<S, B, B2> Service<Request<B>> for ConnectSpanService<S>
where
    S: Service<Request<B>, Response = Response<B2>>,
    S::Future: Send + 'static,
    S::Error: 'static,
    B2: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<B>) -> Self::Future {
        let path = request.uri().path();
        let rpc = (!is_reflection(path)).then(|| rpc_path(path)).flatten();
        let Some((service, method)) = rpc else {
            return Box::pin(self.inner.call(request));
        };

        let span = info_span!(
            "connect_rpc",
            otel.name = %format_args!("{service}/{method}"),
            otel.kind = "server",
            otel.status_code = Empty,
            rpc.system = "connect_rpc",
            rpc.service = service,
            rpc.method = method,
            http.request.method = %request.method(),
            url.path = path,
            http.response.status_code = Empty,
        );

        // Continues the caller's trace when the headers carry one. Fails only
        // when the env filter disabled the span, and then there is nothing to
        // parent, so the error holds no information.
        let parent = global::get_text_map_propagator(|propagator| {
            propagator.extract(&HeaderExtractor(request.headers()))
        });
        let _ = span.set_parent(parent);

        let response = self.inner.call(request);
        let handle = span.clone();
        Box::pin(
            async move {
                let mut response = response.await?;

                let status = response.status();
                handle.record("http.response.status_code", i64::from(status.as_u16()));
                if status.is_server_error() {
                    handle.record("otel.status_code", "ERROR");
                }
                global::get_text_map_propagator(|propagator| {
                    propagator.inject_context(
                        &handle.context(),
                        &mut HeaderInjector(response.headers_mut()),
                    );
                });

                Ok(response)
            }
            .instrument(span),
        )
    }
}

impl Telemetry {
    /// HTTP middlewares for telemetry.
    ///
    /// Reflection calls are tooling chatter, not application traffic, so they
    /// are excluded from spans and metrics alike.
    ///
    /// `None` when export is disabled, so requests pay no instrumentation cost
    /// when nothing consumes it.
    pub(crate) fn otel_http_layers(&self) -> Option<(ConnectSpanLayer, HttpMetricsLayer)> {
        self.providers.as_ref()?;
        Some((
            ConnectSpanLayer,
            HttpMetricsLayerBuilder::new()
                .with_skipper(PathSkipper::new(is_reflection))
                .build(),
        ))
    }

    /// Flushes and shuts down the exporters.
    ///
    /// Failures are logged rather than returned because the process is exiting
    /// either way.
    pub(crate) fn shutdown(self) {
        let Some(providers) = self.providers else {
            return;
        };
        let report = |signal: &str, outcome: OTelSdkResult| {
            if let Err(source) = outcome {
                warn!(%source, "failed to shut down {signal} exporter");
            }
        };
        // Logs go last so warnings from the earlier shutdowns still export.
        report("traces", providers.tracer.shutdown());
        report("metrics", providers.meter.shutdown());
        report("logs", providers.logger.shutdown());
    }
}
