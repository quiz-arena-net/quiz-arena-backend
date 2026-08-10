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
};
use tower::util::option_layer;
use tracing::info;

use crate::{
    config::ServerConfig,
    telemetry::{RpcStatusInterceptor, Telemetry},
};

/// Serves `connect` until SIGINT or SIGTERM, then drains and returns.
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

    // `methods` yields `package.Service/Method`, but the telemetry layers match
    // on the request path, which carries the leading slash.
    let rpc_paths: HashSet<String> = connect.methods().map(|path| format!("/{path}")).collect();
    let app = Router::new()
        .fallback_service(
            connect
                .into_axum_service()
                .with_interceptor(RpcStatusInterceptor),
        )
        .layer(option_layer(telemetry.otel_http_layers(rpc_paths)));
    let listener = TcpListener::bind(config.listen_addr).await?;

    info!("Listening on http://{}", listener.local_addr()?);

    let mut sigint = signal(SignalKind::interrupt())?;
    let mut sigterm = signal(SignalKind::terminate())?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let (signal_name, drain_delay) = tokio::select! {
                _ = sigint.recv() => ("SIGINT", None),
                _ = sigterm.recv() => ("SIGTERM", Some(Duration::from_secs(5))),
            };
            info!("Received {signal_name}, terminating...");

            // Report NotServing so probes stop routing new traffic here while
            // in-flight requests drain.
            health.shutdown();

            // Keep accepting while endpoint removal propagates
            if let Some(delay) = drain_delay {
                tokio::time::sleep(delay).await;
            }
        })
        .await?;

    telemetry.shutdown();

    Ok(())
}
