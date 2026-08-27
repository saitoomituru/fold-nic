// SPDX-License-Identifier: AGPL-3.0-or-later

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs::OpenOptions, io::Write};

#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;

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
        /// 既存fileを上書きせず、秘密を含まない取得receiptを保存する。
        #[arg(long)]
        receipt_file: Option<PathBuf>,
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

#[derive(Debug, Serialize)]
struct FetchProvenanceReceipt<'a> {
    schema: &'static str,
    action: &'static str,
    status: &'static str,
    world_id: &'a str,
    worldline_id: &'a str,
    requested_cid: &'a str,
    verified_cid: &'a str,
    size: u64,
    source_peer_id: String,
    source_authority: &'static str,
    transport_profile: &'static str,
    network_scope: &'static str,
    observed_at_unix_millis: u128,
    clock_source: &'static str,
    clock_calibration: &'static str,
    unknowns: Vec<&'static str>,
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
            receipt_file,
        } => {
            fetch(
                cas_root,
                dial,
                cid,
                world,
                worldline,
                max_object_bytes,
                receipt_file,
            )
            .await
        }
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
    receipt_file: Option<PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if receipt_file.as_ref().is_some_and(|path| path.exists()) {
        return Err("receipt fileが既に存在するため取得を開始しません".into());
    }
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
    let peer_text = peer.to_string();
    let verified_cid = receipt.cid.to_string();
    let provenance = FetchProvenanceReceipt {
        schema: "fold-peer-fetch-receipt/0",
        action: "FETCH_PEER_OBJECT",
        status: "VERIFIED_AND_STORED",
        world_id: &request.world_id,
        worldline_id: &request.worldline_id,
        requested_cid: &request.cid,
        verified_cid: &verified_cid,
        size: receipt.size,
        source_peer_id: peer_text,
        source_authority: "UNVERIFIED_TRANSPORT_PEER",
        transport_profile: "LIBP2P_TCP_NOISE_YAMUX_CBOR",
        network_scope: "LOOPBACK_ONLY",
        observed_at_unix_millis: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
        clock_source: "HOST_SYSTEM_CLOCK",
        clock_calibration: "UNVERIFIED",
        unknowns: vec![
            "PUBLISHER_IDENTITY_UNVERIFIED",
            "WORLD_AUTHORITY_UNVERIFIED",
            "OBJECT_SIGNATURE_NOT_PROVIDED",
        ],
    };
    if let Some(path) = receipt_file {
        write_receipt_new(&path, &provenance)?;
    }
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

fn write_receipt_new(
    path: &std::path::Path,
    receipt: &FetchProvenanceReceipt<'_>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(path)?;
    let mut json = serde_json::to_vec_pretty(receipt)?;
    json.push(b'\n');
    file.write_all(&json)?;
    file.sync_all()?;
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

    #[test]
    fn provenance_receiptはcontentを含めず既存fileを上書きしない() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "fold-peer-receipt-test-{}-{nonce}.json",
            std::process::id()
        ));
        let receipt = FetchProvenanceReceipt {
            schema: "fold-peer-fetch-receipt/0",
            action: "FETCH_PEER_OBJECT",
            status: "VERIFIED_AND_STORED",
            world_id: "fold-nic-forge",
            worldline_id: "stage0",
            requested_cid: "bafk-example",
            verified_cid: "bafk-example",
            size: 7,
            source_peer_id: "peer-example".into(),
            source_authority: "UNVERIFIED_TRANSPORT_PEER",
            transport_profile: "LIBP2P_TCP_NOISE_YAMUX_CBOR",
            network_scope: "LOOPBACK_ONLY",
            observed_at_unix_millis: 0,
            clock_source: "FIXTURE",
            clock_calibration: "UNVERIFIED",
            unknowns: vec!["OBJECT_SIGNATURE_NOT_PROVIDED"],
        };
        write_receipt_new(&path, &receipt).expect("first write");
        let json = fs::read_to_string(&path).expect("read");
        assert!(json.contains("OBJECT_SIGNATURE_NOT_PROVIDED"));
        assert!(!json.contains("object_bytes"));
        assert!(write_receipt_new(&path, &receipt).is_err());
    }
}
