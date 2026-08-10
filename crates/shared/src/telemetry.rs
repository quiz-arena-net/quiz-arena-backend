//! OpenTelemetry pipeline.
//!
//! Owns the process-wide bridge between instrumentation and OTLP export so
//! service code never touches exporter machinery. A service emits `tracing`
//! spans and events and records instruments on the meter it is handed, and this
//! module translates: spans through [`tracing_opentelemetry`], events through
//! [`opentelemetry_appender_tracing`], metrics through the meter provider. When
//! export is disabled only the stdout `fmt` layer is installed and every
//! instrument is a no-op.
//!
//! One process serves one service, so every signal it exports carries one
//! resource. Telling two services apart on the backend is a `service.name`
//! filter and nothing more, and telling two builds of one service apart is a
//! `service.version` filter, which is what a progressive rollout slices on. The
//! instrumentation scope answers neither: it marks first-party signals apart
//! from those of the libraries alongside them.

use std::{
    collections::HashSet,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use axum::http::{Request, Response};
use axum_otel_metrics::{HttpMetricsLayer, HttpMetricsLayerBuilder, PathSkipper};
use connectrpc::{
    ConnectError, ErrorCode, Interceptor,
    interceptor::{
        Next, NextStream, PayloadStream, StreamRequest, StreamResponse, UnaryRequest, UnaryResponse,
    },
};
use opentelemetry::{
    InstrumentationScope, KeyValue, global,
    logs::LoggerProvider as _,
    metrics::{Meter, MeterProvider as _},
    trace::TracerProvider as _,
};
use opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge;
use opentelemetry_http::{HeaderExtractor, HeaderInjector};
use opentelemetry_otlp::{LogExporter, MetricExporter, SpanExporter, WithExportConfig as _};
use opentelemetry_resource_detectors::{
    HostResourceDetector, OsResourceDetector, ProcessResourceDetector,
};
use opentelemetry_sdk::{
    Resource,
    error::OTelSdkResult,
    logs::{SdkLogger, SdkLoggerProvider},
    metrics::SdkMeterProvider,
    propagation::TraceContextPropagator,
    trace::SdkTracerProvider,
};
use tower::{Layer, Service};
use tracing::{Instrument as _, Span, field::Empty, info_span, warn};
use tracing_opentelemetry::OpenTelemetrySpanExt as _;
use tracing_subscriber::{
    EnvFilter,
    filter::{LevelFilter, Targets},
    layer::{Layer as TracingLayer, SubscriberExt as _},
    util::SubscriberInitExt as _,
};

use crate::config::{OtlpProtocol, TelemetryConfig};

/// Handle to the running exporters.
///
/// Call [`shutdown`](Self::shutdown) after the server has drained so buffered
/// telemetry is flushed before exit. Dropping does the same, so an early return
/// still flushes.
pub struct Telemetry {
    providers: Option<Providers>,
}

impl Telemetry {
    /// Whether signals are exported, as opposed to reaching stdout only.
    pub fn is_exporting(&self) -> bool {
        self.providers.is_some()
    }

    /// Meter for this service's instruments.
    ///
    /// Metrics are the one signal a subscriber cannot route, because the meter
    /// is chosen where the instrument is built rather than where the telemetry
    /// is emitted. So a service is handed this to build its instruments from.
    pub fn meter(&self) -> Meter {
        match &self.providers {
            Some(providers) => providers.meter.meter_with_scope(scope()),
            // Export is off, so the global provider is the no-op one and every
            // instrument built from it costs nothing.
            None => global::meter_with_scope(scope()),
        }
    }

    /// HTTP middlewares for telemetry.
    ///
    /// `rpc_paths` are the mounted RPC paths, and only those are instrumented.
    /// Anything else is a scan or a typo, and instrumenting it would let the
    /// caller choose span names. Reflection and health are mounted but are
    /// tooling chatter rather than application traffic, so they are excluded
    /// too.
    ///
    /// `None` when export is disabled, so requests pay no instrumentation cost
    /// when nothing consumes it.
    pub fn otel_http_layers(
        &self,
        rpc_paths: HashSet<String>,
    ) -> Option<(ConnectSpanLayer, HttpMetricsLayer)> {
        self.is_exporting().then(|| {
            let traced = Arc::new(TracedPaths(rpc_paths));
            let skipped = Arc::clone(&traced);
            (
                ConnectSpanLayer { traced },
                HttpMetricsLayerBuilder::new()
                    .with_skipper(PathSkipper::new_with_fn(Arc::new(move |path| {
                        !skipped.contains(path)
                    })))
                    .build(),
            )
        })
    }

    /// Flushes and shuts down the exporters.
    ///
    /// Dropping does the same, so this is the explicit form for the normal exit
    /// path, where blocking on a final export is worth naming. Failures are
    /// logged rather than returned because the process is exiting either way.
    pub fn shutdown(mut self) {
        self.shutdown_providers();
    }

    /// Shuts the providers down at most once, whether that is reached through
    /// [`shutdown`](Self::shutdown) or through [`Drop`].
    fn shutdown_providers(&mut self) {
        if let Some(providers) = self.providers.take() {
            providers.shutdown();
        }
    }
}

impl Drop for Telemetry {
    fn drop(&mut self) {
        self.shutdown_providers();
    }
}

struct Providers {
    tracer: SdkTracerProvider,
    meter: SdkMeterProvider,
    logger: SdkLoggerProvider,
}

impl Providers {
    fn shutdown(self) {
        let report = |signal: &str, outcome: OTelSdkResult| {
            if let Err(source) = outcome {
                warn!(%source, "failed to shut down {signal} exporter");
            }
        };
        // Logs go last so warnings from the earlier shutdowns still export.
        report("traces", self.tracer.shutdown());
        report("metrics", self.meter.shutdown());
        report("logs", self.logger.shutdown());
    }
}

/// Provider adapter that makes the tracing log bridge use this workspace's
/// instrumentation scope. The bridge asks its provider for an unnamed logger;
/// this adapter returns the scoped logger prepared for it instead.
#[derive(Clone)]
struct ScopedLoggerProvider(SdkLogger);

impl opentelemetry::logs::LoggerProvider for ScopedLoggerProvider {
    type Logger = SdkLogger;

    fn logger_with_scope(&self, _scope: InstrumentationScope) -> Self::Logger {
        self.0.clone()
    }
}

/// Targets whose spans and events are exported.
///
/// The exporters emit spans and events themselves while shipping batches over
/// gRPC. Exporting those would feed the pipeline its own output, so the whole
/// transport stack is silenced. Both the span layer and the log bridge apply
/// this, since either alone leaves the loop open.
fn exported_targets() -> Targets {
    Targets::new()
        .with_default(LevelFilter::TRACE)
        .with_target("opentelemetry", LevelFilter::OFF)
        .with_target("tonic", LevelFilter::OFF)
        .with_target("reqwest", LevelFilter::OFF)
        .with_target("h2", LevelFilter::OFF)
        .with_target("hyper", LevelFilter::OFF)
        .with_target("tower", LevelFilter::OFF)
}

/// Installs the global `tracing` subscriber and, when export is enabled, the
/// OpenTelemetry providers behind it.
///
/// `service_name` is the running binary's name, which becomes `service.name` on
/// the resource.
pub fn init(config: TelemetryConfig, service_name: &str) -> anyhow::Result<Telemetry> {
    // RUST_LOG decides what is observed at all, for stdout and export alike.
    let env_filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();
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
    //
    // `service.version` is the resource attribute rather than the scope version
    // because it identifies the deployed build, not the code emitting the
    // telemetry. Being on the resource puts it on every span, metric, and log,
    // which is what lets a progressive rollout slice one population from the
    // other.
    let resource = Resource::builder()
        .with_detectors(&[
            Box::new(ProcessResourceDetector),
            Box::new(OsResourceDetector),
            Box::new(HostResourceDetector::default()),
        ])
        .with_service_name(service_name.to_owned())
        .with_attribute(KeyValue::new("service.version", crate::BUILD_VERSION))
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

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer)
        .with(
            tracing_opentelemetry::layer()
                .with_tracer(tracer.tracer_with_scope(scope()))
                .with_filter(exported_targets()),
        )
        .with(
            OpenTelemetryTracingBridge::new(&ScopedLoggerProvider(
                logger.logger_with_scope(scope()),
            ))
            .with_filter(exported_targets()),
        )
        .init();

    Ok(Telemetry {
        providers: Some(Providers {
            tracer,
            meter,
            logger,
        }),
    })
}

/// Names this workspace's own instrumentation, the way a library names its own.
/// What distinguishes services is `service.name` on the resource, so this stays
/// constant across them and marks first-party signals apart from those of the
/// libraries alongside them.
const SCOPE: &str = "quiz-arena";

/// The instrumentation scope every signal from this process carries.
///
/// Versioned by the build rather than the crate version, because the two name
/// the same artifact here and only one of them moves per commit.
fn scope() -> InstrumentationScope {
    InstrumentationScope::builder(SCOPE)
        .with_version(crate::BUILD_VERSION)
        .build()
}

fn http_signal_url(endpoint: &str, signal: &str) -> String {
    let suffix_start = endpoint.find(['?', '#']).unwrap_or(endpoint.len());
    let (base, suffix) = endpoint.split_at(suffix_start);
    format!("{}/v1/{signal}{suffix}", base.trim_end_matches('/'))
}

/// The paths that get spans and metrics: mounted RPCs minus tooling.
///
/// Reflection and health are tooling, because clients discovering the API and
/// probes checking liveness are chatter rather than application traffic.
/// Version segments follow both prefixes, so a new version of either stays
/// covered.
struct TracedPaths(HashSet<String>);

impl TracedPaths {
    fn contains(&self, path: &str) -> bool {
        self.0.contains(path) && !is_tooling(path)
    }
}

fn is_tooling(path: &str) -> bool {
    path.starts_with("/grpc.reflection.") || path.starts_with("/grpc.health.")
}

/// Splits `/quiz_arena.greet.v1.GreetService/Greet` into service and method.
fn rpc_path(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix('/')?.split_once('/')
}

/// Opens the server span for each Connect RPC, following the OpenTelemetry RPC
/// semantic conventions: spans are named `package.Service/Method` and carry
/// `rpc.*` attributes parsed from the request path.
///
/// Also adopts the caller's trace context from the request headers and injects
/// the span's context into the response headers so callers can correlate.
///
/// Only mounted RPCs are traced, see [`TracedPaths`]. Everything else passes
/// through untouched.
#[derive(Clone)]
pub struct ConnectSpanLayer {
    traced: Arc<TracedPaths>,
}

impl<S> Layer<S> for ConnectSpanLayer {
    type Service = ConnectSpanService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        ConnectSpanService {
            inner,
            traced: Arc::clone(&self.traced),
        }
    }
}

#[derive(Clone)]
pub struct ConnectSpanService<S> {
    inner: S,
    traced: Arc<TracedPaths>,
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
        let rpc = self.traced.contains(path).then(|| rpc_path(path)).flatten();
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
            rpc.connect_rpc.error_code = Empty,
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

/// Records how each RPC ended on its server span, whatever the wire protocol.
///
/// The span layer sees only HTTP, and gRPC and gRPC-Web report failures in
/// trailers under HTTP 200. An interceptor sees the handler's own outcome
/// before any protocol renders it, and runs inside the span, so the record
/// lands on the right span for every protocol.
///
/// Every error records `rpc.connect_rpc.error_code`. Only errors the server is
/// responsible for mark the span as failed, per the OpenTelemetry RPC
/// conventions: a client sending a bad argument is a successful rejection, not
/// a failure of this service.
///
/// Streams are recorded at establishment only. A failure mid-stream is rendered
/// by the protocol without passing back through here.
pub struct RpcStatusInterceptor;

#[connectrpc::async_trait]
impl Interceptor for RpcStatusInterceptor {
    async fn intercept_unary(
        &self,
        request: UnaryRequest,
        next: Next<'_>,
    ) -> Result<UnaryResponse, ConnectError> {
        next.run(request).await.inspect_err(record_rpc_error)
    }

    async fn intercept_streaming(
        &self,
        request: StreamRequest,
        inbound: PayloadStream,
        next: NextStream<'_>,
    ) -> Result<StreamResponse, ConnectError> {
        next.run(request, inbound)
            .await
            .inspect_err(record_rpc_error)
    }
}

fn record_rpc_error(error: &ConnectError) {
    let span = Span::current();
    span.record("rpc.connect_rpc.error_code", error.code.as_str());
    if server_fault(error.code) {
        span.record("otel.status_code", "ERROR");
    }
}

/// Codes that mean this service failed, rather than rejected the request.
fn server_fault(code: ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::Unknown
            | ErrorCode::DeadlineExceeded
            | ErrorCode::Unimplemented
            | ErrorCode::Internal
            | ErrorCode::Unavailable
            | ErrorCode::DataLoss
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_mounted_non_tooling_paths_are_traced() {
        let traced = TracedPaths(
            [
                "/quiz_arena.greet.v1.GreetService/Greet",
                "/grpc.reflection.v1.ServerReflection/ServerReflectionInfo",
                "/grpc.health.v1.Health/Check",
            ]
            .map(str::to_owned)
            .into(),
        );

        assert!(traced.contains("/quiz_arena.greet.v1.GreetService/Greet"));
        assert!(!traced.contains("/grpc.reflection.v1.ServerReflection/ServerReflectionInfo"));
        assert!(!traced.contains("/grpc.health.v1.Health/Check"));
        // Not mounted, so a scan rather than an RPC.
        assert!(!traced.contains("/random/value"));
    }

    #[test]
    fn http_signal_path_is_appended_to_base_endpoint() {
        assert_eq!(
            http_signal_url("https://collector.example", "traces"),
            "https://collector.example/v1/traces"
        );
    }

    #[test]
    fn http_signal_path_is_inserted_before_query_and_fragment() {
        assert_eq!(
            http_signal_url("https://collector.example/custom?tenant=acme", "traces"),
            "https://collector.example/custom/v1/traces?tenant=acme"
        );
        assert_eq!(
            http_signal_url("https://collector.example/custom/#fragment", "metrics"),
            "https://collector.example/custom/v1/metrics#fragment"
        );
    }
}
