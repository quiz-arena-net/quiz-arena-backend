//! HTTP and ConnectRPC bootstrap.
//!
//! Serves one process's ConnectRPC services over axum, with health reporting,
//! gRPC server reflection, telemetry middleware, and a graceful shutdown that
//! drains in-flight requests. A service crate composes its own handlers, adds
//! them to a [`ConnectRouter`], and hands the result to [`serve`].

use std::{collections::HashSet, time::Duration};

use axum::Router;
use connectrpc::Router as ConnectRouter;
use connectrpc_reflection::Reflector as ConnectReflector;
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
    sync::oneshot,
};
use tower::util::option_layer;
use tracing::{info, warn};

use crate::{
    config::ServerConfig,
    telemetry::{RpcStatusInterceptor, Telemetry},
};

/// Serves `connect` until SIGINT or SIGTERM, then drains for at most 10
/// seconds. SIGTERM first allows 5 seconds for endpoint removal to propagate.
///
/// `connect` carries this process's services, already added. `service_names`
/// names those same services, for health reporting and reflection.
/// `descriptor_set` covers every service in the workspace, so reflection
/// advertises only the subset `service_names` lists.
pub async fn serve(
    connect: ConnectRouter,
    service_names: Vec<&'static str>,
    descriptor_set: &'static [u8],
    config: ServerConfig,
    telemetry: Telemetry,
) -> anyhow::Result<()> {
    let (connect, health) = connectrpc_health::install_static(connect, service_names.clone());

    let reflector = ConnectReflector::from_descriptor_set_bytes(descriptor_set)?.with_services(
        service_names.into_iter().chain([
            connectrpc_reflection::SERVER_REFLECTION_SERVICE_NAME,
            connectrpc_reflection::SERVER_REFLECTION_V1ALPHA_SERVICE_NAME,
        ]),
    );
    let connect = connectrpc_reflection::install(connect, reflector);

    // `methods` yields `package.Service/Method`, but routes and the telemetry
    // layers match on the request path, which carries the leading slash.
    let rpc_paths: HashSet<String> = connect.methods().map(|path| format!("/{path}")).collect();
    let service = connect
        .into_axum_service()
        .with_interceptor(RpcStatusInterceptor);
    let app = rpc_paths
        .iter()
        .fold(Router::new(), |router, path| {
            router.route_service(path, service.clone())
        })
        .layer(option_layer(telemetry.otel_http_layers(rpc_paths)));
    let listener = TcpListener::bind(config.listen_addr).await?;

    info!("Listening on http://{}", listener.local_addr()?);

    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    let (start_draining, drain_started) = oneshot::channel();
    let shutdown = async move {
        let (signal_name, propagation_delay) = tokio::select! {
            _ = sigint.recv() => ("SIGINT", Duration::ZERO),
            _ = sigterm.recv() => ("SIGTERM", Duration::from_secs(5)),
        };
        info!("Received {signal_name}, terminating...");

        // Report NotServing so probes stop routing new traffic here.
        health.shutdown();
        tokio::time::sleep(propagation_delay).await;
        let _ = start_draining.send(());
    };

    // Start the deadline only after the propagation delay, when accepting
    // stops.
    let drain_timeout = Duration::from_secs(10);
    let drain_deadline = async {
        let _ = drain_started.await;
        tokio::time::sleep(drain_timeout).await;
    };

    tokio::select! {
        result = axum::serve(listener, app).with_graceful_shutdown(shutdown) => result?,
        _ = drain_deadline => warn!(
            ?drain_timeout,
            "request drain timed out, continuing shutdown"
        ),
    }

    telemetry.shutdown();

    Ok(())
}
