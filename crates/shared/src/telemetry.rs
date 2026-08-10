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

use axum::http::{Extensions, Request, Response};
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
use opentelemetry_otlp::{
    LogExporter, MetricExporter, Protocol, SpanExporter, WithExportConfig as _,
    WithTonicConfig as _, tonic_types::transport::ClientTlsConfig,
};
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
        self.providers.as_ref().map(|providers| {
            let traced = Arc::new(TracedPaths(rpc_paths));
            let skipped = Arc::clone(&traced);
            (
                ConnectSpanLayer { traced },
                HttpMetricsLayerBuilder::new()
                    .with_provider(providers.meter.clone())
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
            .with_tls_config(ClientTlsConfig::new().with_native_roots())
            .with_endpoint(&endpoint)
            .build()?,
        OtlpProtocol::HttpProtobuf => SpanExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(http_signal_url(&endpoint, "traces"))
            .build()?,
    };
    let metric_exporter = match protocol {
        OtlpProtocol::Grpc => MetricExporter::builder()
            .with_tonic()
            .with_tls_config(ClientTlsConfig::new().with_native_roots())
            .with_endpoint(&endpoint)
            .build()?,
        OtlpProtocol::HttpProtobuf => MetricExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(http_signal_url(&endpoint, "metrics"))
            .build()?,
    };
    let log_exporter = match protocol {
        OtlpProtocol::Grpc => LogExporter::builder()
            .with_tonic()
            .with_tls_config(ClientTlsConfig::new().with_native_roots())
            .with_endpoint(&endpoint)
            .build()?,
        OtlpProtocol::HttpProtobuf => LogExporter::builder()
            .with_http()
            .with_protocol(Protocol::HttpBinary)
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
/// The span travels to [`RpcStatusInterceptor`] in the request extensions,
/// which is how the RPC outcome reaches it.
///
/// Only mounted RPCs are traced, excluding reflection and health checks.
/// Everything else passes through untouched.
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

    fn call(&mut self, mut request: Request<B>) -> Self::Future {
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
            rpc.system.name = "connectrpc",
            rpc.response.status_code = Empty,
            rpc.method = %format_args!("{service}/{method}"),
            error.type = Empty,
            http.request.method = %request.method(),
            url.path = path,
            http.response.status_code = Empty,
        );
        request.extensions_mut().insert(RpcSpan(span.clone()));

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
/// before any protocol renders it, so the record is right for every protocol.
///
/// The span comes from the request extensions rather than from the current
/// span, because connectrpc enters a span of its own around the interceptors
/// whenever DEBUG is enabled, and a record on that span would be dropped.
///
/// Every error records `rpc.response.status_code`. Only errors the server is
/// responsible for set `error.type` and mark the span as failed, per the
/// OpenTelemetry RPC conventions: a client sending a bad argument is a
/// successful rejection, not a failure of this service.
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
        let span = rpc_span(request.ctx.extensions());
        next.run(request)
            .await
            .inspect(|_| {
                span.record("rpc.response.status_code", "OK");
            })
            .inspect_err(|error| record_rpc_error(&span, error))
    }

    async fn intercept_streaming(
        &self,
        request: StreamRequest,
        inbound: PayloadStream,
        next: NextStream<'_>,
    ) -> Result<StreamResponse, ConnectError> {
        let span = rpc_span(request.ctx.extensions());
        next.run(request, inbound)
            .await
            .inspect_err(|error| record_rpc_error(&span, error))
    }
}

/// The server span, carried from [`ConnectSpanLayer`] to
/// [`RpcStatusInterceptor`] in the request extensions.
#[derive(Clone)]
struct RpcSpan(Span);

/// The server span for this request, or a disabled span when the layer is not
/// installed, so that recording is a no-op rather than a crash.
fn rpc_span(extensions: &Extensions) -> Span {
    extensions
        .get::<RpcSpan>()
        .map_or_else(Span::none, |rpc| rpc.0.clone())
}

fn record_rpc_error(span: &Span, error: &ConnectError) {
    span.record("rpc.response.status_code", error.code.as_str());
    if server_fault(error.code) {
        span.record("error.type", error.code.as_str());
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
    use axum::{
        Router,
        body::Body,
        http::{StatusCode, header},
        routing::post,
    };
    use connectrpc::{RequestContext, Router as ConnectRouter, handler_fn};
    use opentelemetry::{Value, trace::Status};
    use opentelemetry_sdk::{
        metrics::{
            InMemoryMetricExporter,
            data::{AggregatedMetrics, MetricData},
        },
        trace::InMemorySpanExporter,
    };
    use tower::ServiceExt as _;

    use super::*;

    #[test]
    fn http_protocol_override_initializes_all_exporters() {
        const CHILD: &str = "QUIZ_ARENA_TEST_HTTP_PROTOCOL_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let config = crate::config::AppConfig::load()
                .unwrap()
                .telemetry()
                .unwrap();
            assert!(matches!(config.otlp_protocol, OtlpProtocol::HttpProtobuf));
            let runtime = tokio::runtime::Runtime::new().unwrap();
            let _guard = runtime.enter();
            let telemetry = init(config, "protocol-regression-test").unwrap();
            assert!(telemetry.is_exporting());
            telemetry.shutdown();
            return;
        }

        // Isolate environment overrides and global subscriber installation.
        let mut command = std::process::Command::new(std::env::current_exe().unwrap());
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("OTEL_") || name.starts_with("QUIZ_ARENA_") {
                command.env_remove(key);
            }
        }
        let output = command
            .args([
                "--exact",
                "telemetry::tests::http_protocol_override_initializes_all_exporters",
            ])
            .current_dir(std::env::temp_dir())
            .env(CHILD, "1")
            .env("OTEL_EXPORTER_OTLP_PROTOCOL", "grpc")
            .env("QUIZ_ARENA_TELEMETRY_OTLP_PROTOCOL", "http/protobuf")
            .env("QUIZ_ARENA_TELEMETRY_ENABLED", "true")
            .env("QUIZ_ARENA_TELEMETRY_OTLP_ENDPOINT", "http://127.0.0.1:1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    /// connectrpc opens a span of its own around the interceptors whenever
    /// DEBUG is enabled, so the current span inside the interceptor is not the
    /// server span. This pins that the outcome still lands on the server span,
    /// over a protocol that reports the error in trailers under HTTP 200 so the
    /// interceptor is the only source of the status.
    #[tokio::test]
    async fn rpc_outcomes_are_recorded_on_the_server_span_under_debug() {
        for error in [
            None,
            Some(ErrorCode::Internal),
            Some(ErrorCode::InvalidArgument),
        ] {
            assert_rpc_outcome(error).await;
        }
    }

    async fn assert_rpc_outcome(error: Option<ErrorCode>) {
        let exporter = InMemorySpanExporter::default();
        let tracer = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        let subscriber = tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(tracer.tracer("test")));
        let _guard = tracing::subscriber::set_default(subscriber);
        assert!(tracing::enabled!(tracing::Level::DEBUG));

        let telemetry = Telemetry {
            providers: Some(Providers {
                tracer: tracer.clone(),
                meter: SdkMeterProvider::builder().build(),
                logger: SdkLoggerProvider::builder().build(),
            }),
        };
        let path = "/quiz_arena.greet.v1.GreetService/Greet";
        let (spans, _metrics) = telemetry
            .otel_http_layers([path.to_owned()].into())
            .expect("export is on");
        let connect = ConnectRouter::new().route(
            "quiz_arena.greet.v1.GreetService",
            "Greet",
            handler_fn(move |_: RequestContext, _: buffa_types::Empty| async move {
                match error {
                    Some(ErrorCode::Internal) => Err(ConnectError::internal("boom")),
                    Some(ErrorCode::InvalidArgument) => {
                        Err(ConnectError::invalid_argument("bad input"))
                    }
                    None => connectrpc::Response::ok(buffa_types::Empty::default()),
                    _ => unreachable!(),
                }
            }),
        );
        let service = connect
            .into_axum_service()
            .with_interceptor(RpcStatusInterceptor);
        let app = Router::new().route_service(path, service).layer(spans);

        // One empty gRPC-Web frame: flag byte, then a zero length.
        let request = Request::post(path)
            .header(header::CONTENT_TYPE, "application/grpc-web+proto")
            .body(Body::from(vec![0, 0, 0, 0, 0]))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        tracer.force_flush().unwrap();
        let spans = exporter.get_finished_spans().unwrap();
        let span = spans
            .iter()
            .find(|span| span.name == "quiz_arena.greet.v1.GreetService/Greet")
            .expect("the server span");
        let attribute = |key: &str| {
            span.attributes
                .iter()
                .find(|attribute| attribute.key.as_str() == key)
                .map(|attribute| attribute.value.clone())
        };
        assert_eq!(
            attribute("rpc.system.name"),
            Some(Value::from("connectrpc"))
        );
        assert_eq!(
            attribute("rpc.method"),
            Some(Value::from("quiz_arena.greet.v1.GreetService/Greet"))
        );
        assert_eq!(
            attribute("rpc.response.status_code"),
            Some(Value::from(error.map_or("OK", |code| code.as_str())))
        );
        if error.is_some_and(server_fault) {
            assert!(matches!(span.status, Status::Error { .. }));
            assert_eq!(attribute("error.type"), Some(Value::from("internal")));
        } else {
            assert_eq!(span.status, Status::Unset);
            assert_eq!(attribute("error.type"), None);
        }
        for legacy in ["rpc.system", "rpc.service", "rpc.connect_rpc.error_code"] {
            assert_eq!(attribute(legacy), None);
        }
    }

    /// The metrics layer labels and filters on the route axum matched, which
    /// axum records only for real routes. This pins that a mounted RPC served
    /// from a route is metered, the contract `server` relies on when it mounts
    /// each RPC as a route rather than one fallback.
    #[tokio::test]
    async fn mounted_rpc_served_from_a_route_is_metered() {
        // Without a subscriber this thread can cache the shared `connect_rpc`
        // callsite as disabled, and the RPC status test then finds no span.
        let _guard = tracing::subscriber::set_default(tracing_subscriber::registry());
        let exporter = InMemoryMetricExporter::default();
        let meter = SdkMeterProvider::builder()
            .with_periodic_exporter(exporter.clone())
            .build();
        let telemetry = Telemetry {
            providers: Some(Providers {
                tracer: SdkTracerProvider::builder().build(),
                meter: meter.clone(),
                logger: SdkLoggerProvider::builder().build(),
            }),
        };
        let path = "/quiz_arena.greet.v1.GreetService/Greet";
        let (spans, metrics) = telemetry
            .otel_http_layers([path.to_owned()].into())
            .expect("export is on");
        let app = Router::new()
            .route(path, post(|| async { "{}" }))
            .layer(spans)
            .layer(metrics);

        let request = Request::post(path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"sender":"alice"}"#))
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        meter.force_flush().unwrap();
        let metrics = exporter.get_finished_metrics().unwrap();
        let duration = metrics
            .iter()
            .flat_map(|resource| resource.scope_metrics())
            .flat_map(|scope| scope.metrics())
            .find(|metric| metric.name() == "http.server.request.duration")
            .expect("a request duration histogram");
        let AggregatedMetrics::F64(MetricData::Histogram(histogram)) = duration.data() else {
            panic!("request duration is an f64 histogram");
        };
        let point = histogram.data_points().next().expect("one data point");
        assert_eq!(point.count(), 1);
        let route = point
            .attributes()
            .find(|attribute| attribute.key.as_str() == "http.route")
            .map(|attribute| attribute.value.clone());
        assert_eq!(route, Some(Value::from(path)));
    }

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
