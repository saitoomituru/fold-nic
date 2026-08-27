// SPDX-License-Identifier: AGPL-3.0-or-later

#![forbid(unsafe_code)]

use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use fold_gateway::{router, validate_bind_scope};
use fold_store::LocalCas;
use serde::Serialize;

#[derive(Debug, Parser)]
#[command(name = "fold-gateway")]
#[command(about = "Fold NICのloopback限定read-only Gateway")]
struct Arguments {
    #[arg(long, default_value = "127.0.0.1:7743")]
    bind: SocketAddr,

    #[arg(long, default_value = ".fold-nic/cache/cas")]
    cas_root: PathBuf,

    #[arg(long, default_value_t = 64 * 1024 * 1024)]
    max_object_bytes: u64,
}

#[derive(Debug, Serialize)]
struct StartupReceipt {
    schema: &'static str,
    status: &'static str,
    bind: String,
    cas_root: String,
    network_scope: &'static str,
    capabilities: Vec<&'static str>,
    explicit_non_capabilities: Vec<&'static str>,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("fold-gateway起動失敗: {error}");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Arguments::parse();
    validate_bind_scope(arguments.bind)?;
    let cas = LocalCas::open(&arguments.cas_root, arguments.max_object_bytes)?;
    let listener = tokio::net::TcpListener::bind(arguments.bind).await?;
    let actual_bind = listener.local_addr()?;
    let receipt = StartupReceipt {
        schema: "fold-gateway-startup/0",
        status: "ready",
        bind: actual_bind.to_string(),
        cas_root: cas.root().display().to_string(),
        network_scope: "LOOPBACK_ONLY",
        capabilities: vec!["health", "verified-local-cas-read"],
        explicit_non_capabilities: vec!["write-endpoint", "origin-fetch", "p2p"],
    };
    println!("{}", serde_json::to_string(&receipt)?);

    axum::serve(listener, router(cas))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        eprintln!("shutdown signalを待機できません: {error}");
    }
}
