// SPDX-License-Identifier: AGPL-3.0-or-later

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::Duration;

use cid::Cid;
use clap::{Parser, Subcommand};
use fold_core::{WorldId, WorldlineId};
use fold_peer::{
    DEFAULT_MAX_OBJECT_BYTES, OBJECT_PROTOCOL, ObjectRequest, ObjectResponse, accept_response,
    split_loopback_dial, validate_loopback_listen,
};
use fold_store::LocalCas;
use libp2p::futures::StreamExt;
use libp2p::request_response::{self, Message, ProtocolSupport};
use libp2p::swarm::SwarmEvent;
use libp2p::{Multiaddr, StreamProtocol, Swarm, SwarmBuilder};
use serde::Serialize;

type ObjectBehaviour = request_response::cbor::Behaviour<ObjectRequest, ObjectResponse>;

#[derive(Debug, Parser)]
#[command(
    name = "fold-peer",
    about = "Fold NICのloopback限定二process object交換実験"
)]
struct Arguments {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// 公開してよいfixtureをlocal CASへ明示投入する。
    SeedPublicFixture {
        #[arg(long)]
        cas_root: PathBuf,
        #[arg(long)]
        file: PathBuf,
        #[arg(long)]
        acknowledge_loopback_share: bool,
        #[arg(long, default_value_t = DEFAULT_MAX_OBJECT_BYTES)]
        max_object_bytes: u64,
    },
    /// loopbackでCAS object要求を待つ。
    Serve {
        #[arg(long)]
        cas_root: PathBuf,
        #[arg(long, default_value = "/ip4/127.0.0.1/tcp/0")]
        listen: Multiaddr,
        #[arg(long, default_value_t = DEFAULT_MAX_OBJECT_BYTES)]
        max_object_bytes: u64,
    },
    /// 明示したloopback peerから一個のCID objectを取得する。
    Fetch {
        #[arg(long)]
        cas_root: PathBuf,
        #[arg(long)]
        dial: Multiaddr,
        #[arg(long)]
        cid: String,
        #[arg(long)]
        world: String,
        #[arg(long)]
        worldline: String,
        #[arg(long, default_value_t = DEFAULT_MAX_OBJECT_BYTES)]
        max_object_bytes: u64,
    },
}

#[derive(Debug, Serialize)]
struct Receipt<'a> {
    schema: &'static str,
    status: &'static str,
    action: &'a str,
    cid: Option<String>,
    size: Option<u64>,
    address: Option<String>,
    peer_id_scope: &'static str,
    network_scope: &'static str,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("fold-peer失敗: {error} ({error:?})");
        std::process::exit(2);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    match Arguments::parse().command {
        Command::SeedPublicFixture {
            cas_root,
            file,
            acknowledge_loopback_share,
            max_object_bytes,
        } => seed_public_fixture(cas_root, file, acknowledge_loopback_share, max_object_bytes),
        Command::Serve {
            cas_root,
            listen,
            max_object_bytes,
        } => serve(cas_root, listen, max_object_bytes).await,
        Command::Fetch {
            cas_root,
            dial,
            cid,
            world,
            worldline,
            max_object_bytes,
        } => fetch(cas_root, dial, cid, world, worldline, max_object_bytes).await,
    }
}

fn seed_public_fixture(
    cas_root: PathBuf,
    file: PathBuf,
    acknowledge: bool,
    max_object_bytes: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    if !acknowledge {
        return Err("--acknowledge-loopback-shareで共有許可を明示してください".into());
    }
    let metadata = std::fs::metadata(&file)?;
    if !metadata.is_file() || metadata.len() > max_object_bytes {
        return Err("fixtureが通常fileでないかsize上限を超えています".into());
    }
    let cas = LocalCas::open(cas_root, max_object_bytes)?;
    let receipt = cas.put(&std::fs::read(file)?)?;
    print_receipt(&Receipt {
        schema: "fold-peer-receipt/0",
        status: "ok",
        action: "seed-public-fixture",
        cid: Some(receipt.cid.to_string()),
        size: Some(receipt.size),
        address: None,
        peer_id_scope: "NOT_CREATED",
        network_scope: "LOCAL_FILESYSTEM_ONLY",
    })?;
    Ok(())
}

fn behaviour(max_object_bytes: u64) -> ObjectBehaviour {
    let codec = request_response::cbor::codec::Codec::<ObjectRequest, ObjectResponse>::default()
        .set_request_size_maximum(4096)
        .set_response_size_maximum(max_object_bytes.saturating_add(65_536));
    ObjectBehaviour::with_codec(
        codec,
        [(StreamProtocol::new(OBJECT_PROTOCOL), ProtocolSupport::Full)],
        request_response::Config::default().with_request_timeout(Duration::from_secs(15)),
    )
}

fn swarm(max_object_bytes: u64) -> Result<Swarm<ObjectBehaviour>, Box<dyn std::error::Error>> {
    Ok(SwarmBuilder::with_new_identity()
        .with_tokio()
        .with_tcp(
            libp2p::tcp::Config::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        )?
        .with_behaviour(|_| behaviour(max_object_bytes))?
        .build())
}

async fn serve(
    cas_root: PathBuf,
    listen: Multiaddr,
    max_object_bytes: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_loopback_listen(&listen)?;
    let cas = LocalCas::open(cas_root, max_object_bytes)?;
    let mut swarm = swarm(max_object_bytes)?;
    swarm.listen_on(listen)?;

    loop {
        tokio::select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    let full = address.with(libp2p::multiaddr::Protocol::P2p(*swarm.local_peer_id()));
                    print_receipt(&Receipt {
                        schema: "fold-peer-receipt/0",
                        status: "ready",
                        action: "serve",
                        cid: None,
                        size: None,
                        address: Some(full.to_string()),
                        peer_id_scope: "EPHEMERAL_PROCESS_ONLY",
                        network_scope: "LOOPBACK_ONLY",
                    })?;
                }
                SwarmEvent::Behaviour(request_response::Event::Message {
                    message: Message::Request { request, channel, .. },
                    ..
                }) => {
                    let response = response_from_cas(&cas, request);
                    if swarm.behaviour_mut().send_response(channel, response).is_err() {
                        eprintln!("応答channelが閉じていました");
                    }
                }
                SwarmEvent::Behaviour(request_response::Event::InboundFailure { error, .. }) => {
                    eprintln!("受信要求失敗: {error}");
                }
                _ => {}
            },
            signal = tokio::signal::ctrl_c() => {
                signal?;
                return Ok(());
            }
        }
    }
}

fn response_from_cas(cas: &LocalCas, request: ObjectRequest) -> ObjectResponse {
    if WorldId::parse(request.world_id.clone()).is_err()
        || WorldlineId::parse(request.worldline_id.clone()).is_err()
    {
        return ObjectResponse::Error {
            code: "P2P_INVALID_WORLD_CONTEXT".into(),
        };
    }
    let Ok(cid) = request.cid.parse::<Cid>() else {
        return ObjectResponse::Error {
            code: "P2P_INVALID_CID".into(),
        };
    };
    match cas.get(&cid) {
        Ok(bytes) => ObjectResponse::Found {
            cid: request.cid,
            world_id: request.world_id,
            worldline_id: request.worldline_id,
            bytes,
        },
        Err(error) => ObjectResponse::Error {
            code: error.code.as_str().into(),
        },
    }
}

async fn fetch(
    cas_root: PathBuf,
    dial: Multiaddr,
    cid: String,
    world: String,
    worldline: String,
    max_object_bytes: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let (peer, transport) = split_loopback_dial(&dial)?;
    cid.parse::<Cid>()?;
    WorldId::parse(world.clone())?;
    WorldlineId::parse(worldline.clone())?;
    let request = ObjectRequest {
        cid,
        world_id: world,
        worldline_id: worldline,
    };
    let cas = LocalCas::open(cas_root, max_object_bytes)?;
    let mut swarm = swarm(max_object_bytes)?;
    let request_id =
        swarm
            .behaviour_mut()
            .send_request_with_addresses(&peer, request.clone(), vec![transport]);

    let response = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if let SwarmEvent::Behaviour(event) = swarm.select_next_some().await {
                match event {
                    request_response::Event::Message {
                        message:
                            Message::Response {
                                request_id: observed,
                                response,
                            },
                        ..
                    } if observed == request_id => return Ok(response),
                    request_response::Event::OutboundFailure {
                        request_id: observed,
                        error,
                        ..
                    } if observed == request_id => {
                        return Err(format!("送信要求失敗: {error}"));
                    }
                    _ => {}
                }
            }
        }
    })
    .await
    .map_err(|_| "P2P応答が20秒以内にありません")?
    .map_err(|error| -> Box<dyn std::error::Error> { error.into() })?;

    let receipt = accept_response(&cas, &request, response)?;
    print_receipt(&Receipt {
        schema: "fold-peer-receipt/0",
        status: "ok",
        action: "fetch",
        cid: Some(receipt.cid.to_string()),
        size: Some(receipt.size),
        address: Some(dial.to_string()),
        peer_id_scope: "EPHEMERAL_PROCESS_ONLY",
        network_scope: "LOOPBACK_ONLY",
    })?;
    Ok(())
}

fn print_receipt(receipt: &Receipt<'_>) -> Result<(), serde_json::Error> {
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use fold_store::cid_for;

    use super::*;

    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_cas() -> LocalCas {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "fold-peer-main-test-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("root");
        LocalCas::open(root, 1024).expect("cas")
    }

    #[test]
    fn 不在objectは安定codeで返す() {
        let cas = temporary_cas();
        let response = response_from_cas(
            &cas,
            ObjectRequest {
                cid: cid_for(b"missing").to_string(),
                world_id: "fold-nic-forge".into(),
                worldline_id: "stage0".into(),
            },
        );
        assert_eq!(
            response,
            ObjectResponse::Error {
                code: "CAS_OBJECT_NOT_FOUND".into()
            }
        );
    }
}
