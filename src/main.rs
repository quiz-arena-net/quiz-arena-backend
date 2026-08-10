mod config;
mod module;
mod modules;
mod proto;
mod shared;
mod telemetry;

use std::time::Duration;

use axum::Router;
use connectrpc::Router as ConnectRouter;
use connectrpc_reflection::Reflector as ConnectReflector;
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
};
use tower::util::option_layer;
use tracing::info;

pub(crate) use crate::module::{DynModule, Module, ModuleContext, ModuleRegistration};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_config = config::load()?;
    let telemetry = telemetry::init(app_config.telemetry()?)?;

    info!("Starting quiz-arena-backend v{}", VERSION);

    app_config.warn_unknown_sections();

    let server_config = app_config.server()?;

    let (connect, health) =
        connectrpc_health::install_static(ConnectRouter::new(), modules::service_names());

    // The workspace descriptor set carries every module's protos regardless
    // of enabled features. Advertise only the services actually mounted
    // (plus reflection itself, which the default list would also include).
    let reflector = ConnectReflector::from_descriptor_set_bytes(proto::FILE_DESCRIPTOR_SET)?
        .with_services(modules::service_names().into_iter().chain([
            connectrpc_reflection::SERVER_REFLECTION_SERVICE_NAME,
            connectrpc_reflection::SERVER_REFLECTION_V1ALPHA_SERVICE_NAME,
        ]));
    let connect = connectrpc_reflection::install(connect, reflector);

    let ctx = ModuleContext {
        config: app_config,
        connect_router: connect,
    };
    let connect = modules::register(ctx).await?.connect_router;

    let app = Router::new()
        .fallback_service(connect.into_axum_service())
        .layer(option_layer(telemetry.otel_http_layers()));
    let listener = TcpListener::bind(server_config.listen_addr).await?;

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

            // Report NotServing so probes stop routing new traffic here
            // while in-flight requests drain.
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
