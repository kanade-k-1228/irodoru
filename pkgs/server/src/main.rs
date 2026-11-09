mod api;
mod config;
use irodoru_engine::{Engine, Runtime};

use std::{net::SocketAddr, time::Duration};

use anyhow::Result;
use clap::Parser;
use tracing::info;

use api::AppState;
use config::{Arg, Cmd, ServeArgs};

#[tokio::main]
async fn main() -> Result<()> {
    let Arg { command, log } = Arg::parse();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(tracing_subscriber::EnvFilter::try_new(&log)?)
        .init();

    match command {
        Cmd::Serve(args) => serve(args).await,
        Cmd::Devices(args) => {
            let runtime = Runtime::discover(1, args.device, args.openvino_library.as_deref())?;
            println!("{}", serde_json::to_string_pretty(runtime.devices())?);
            Ok(())
        }
        Cmd::Openapi => {
            println!("{}", api::openapi().to_pretty_json()?);
            Ok(())
        }
        Cmd::Ready { port } => {
            std::net::TcpStream::connect_timeout(
                &SocketAddr::from(([127, 0, 0, 1], port)),
                Duration::from_secs(3),
            )?;
            Ok(())
        }
    }
}

async fn serve(args: ServeArgs) -> Result<()> {
    let config = args.engine();
    let engine = tokio::task::spawn_blocking(move || Engine::load(&config)).await??;
    let app = AppState::new(engine).router();

    let addr = SocketAddr::from(([0, 0, 0, 0], args.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "listening");
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
