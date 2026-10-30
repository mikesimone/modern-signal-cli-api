#![recursion_limit = "512"]

mod daemon;
mod jsonrpc;
mod middleware;
mod native;
mod routes;
mod state;
mod watchdog;
mod webhooks;

use axum::extract::DefaultBodyLimit;
use axum::middleware as axum_mw;
use clap::Parser;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpStream;
use tower_http::cors::CorsLayer;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "modern-signal-cli-api", about = "Modern Signal CLI API: REST + WebSocket API for signal-cli")]
struct Cli {
    /// Connect to an existing signal-cli daemon at this address.
    /// If omitted, signal-cli is auto-spawned as a child process.
    #[arg(long)]
    signal_cli: Option<String>,

    /// Listen address for HTTP API
    #[arg(long, default_value = "127.0.0.1:8080")]
    listen: String,

    /// Path to TLS certificate file (PEM format). Enables HTTPS when set.
    #[arg(long)]
    tls_cert: Option<String>,

    /// Path to TLS private key file (PEM format). Required with --tls-cert.
    #[arg(long)]
    tls_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let cli = Cli::parse();

    // Either connect to an existing daemon or auto-spawn one.
    let _managed_daemon; // held alive so child process isn't dropped
    let signal_cli_addr = match cli.signal_cli {
        Some(addr) => addr,
        None => {
            let d = daemon::spawn().await?;
            let addr = d.addr.clone();
            _managed_daemon = d;
            addr
        }
    };

    tracing::info!("Connecting to signal-cli at {signal_cli_addr}");
    let stream = TcpStream::connect(&signal_cli_addr).await?;
    let (reader, writer) = stream.into_split();

    let (writer_tx, writer_rx) = tokio::sync::mpsc::channel::<String>(256);
    tokio::spawn(jsonrpc::writer_loop(writer_rx, writer));

    let app_state = state::AppState::new(writer_tx);

    // Spawn the reader loop
    let broadcast_tx = app_state.broadcast_tx.clone();
    let pending = app_state.pending.clone();
    let metrics = app_state.metrics.clone();
    tokio::spawn(jsonrpc::reader_loop(reader, broadcast_tx, pending, metrics));

    // Spawn webhook dispatcher
    let webhook_state = app_state.clone();
    tokio::spawn(webhooks::dispatch_loop(webhook_state));

    // Tell systemd we're up (no-op unless run under Type=notify), then start
    // the watchdog heartbeat - see watchdog.rs for why this exists.
    let _ = sd_notify::notify(false, &[sd_notify::NotifyState::Ready]);
    let watchdog_state = app_state.clone();
    tokio::spawn(watchdog::heartbeat_loop(
        watchdog_state,
        Duration::from_secs(15),
        Duration::from_secs(10),
    ));

    let app = routes::router(app_state)
        .layer(axum_mw::from_fn(middleware::request_tracing))
        .layer(CorsLayer::permissive())
        // Axum's default body limit is 2MB, which a base64-encoded video
        // attachment blows through instantly (send/attachment payloads embed
        // the whole file inline as a data URI). 300MB comfortably covers
        // Signal's own attachment size ceiling plus base64/JSON overhead.
        .layer(DefaultBodyLimit::max(300 * 1024 * 1024));

    let requested: SocketAddr = cli.listen.parse()?;

    match (cli.tls_cert, cli.tls_key) {
        (Some(cert), Some(key)) => {
            let tls_config = axum_server::tls_rustls::RustlsConfig::from_pem_file(&cert, &key).await?;
            // Probe with a regular TcpListener; if busy, fall back to OS-assigned port.
            let addr = match tokio::net::TcpListener::bind(requested).await {
                Ok(probe) => { drop(probe); requested }
                Err(_) => {
                    let fallback = SocketAddr::from(([127, 0, 0, 1], 0));
                    let probe = tokio::net::TcpListener::bind(fallback).await?;
                    let addr = probe.local_addr()?;
                    drop(probe);
                    tracing::warn!("Port {} busy, using {addr} instead", requested.port());
                    addr
                }
            };
            tracing::info!("Listening on https://{addr} (TLS)");
            tokio::select! {
                result = axum_server::bind_rustls(addr, tls_config)
                    .serve(app.into_make_service()) => { result?; }
                _ = shutdown_signal() => {
                    tracing::info!("Shutdown signal received, stopping...");
                }
            }
        }
        (None, None) => {
            let listener = match tokio::net::TcpListener::bind(requested).await {
                Ok(l) => l,
                Err(_) => {
                    let fallback = SocketAddr::from(([127, 0, 0, 1], 0));
                    let l = tokio::net::TcpListener::bind(fallback).await?;
                    tracing::warn!(
                        "Port {} busy, using {} instead",
                        requested.port(),
                        l.local_addr()?
                    );
                    l
                }
            };
            tracing::info!("Listening on http://{}", listener.local_addr()?);
            tokio::select! {
                result = axum::serve(listener, app) => { result?; }
                _ = shutdown_signal() => {
                    tracing::info!("Shutdown signal received, stopping...");
                }
            }
        }
        _ => {
            anyhow::bail!("Both --tls-cert and --tls-key must be provided together");
        }
    }

    // _managed_daemon drops here → process group killed
    Ok(())
}

/// Wait for SIGTERM or Ctrl+C, whichever comes first.
async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    let mut sigterm =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to register SIGTERM handler");
    tokio::select! {
        _ = ctrl_c => {}
        _ = sigterm.recv() => {}
    }
}
