// SPDX-License-Identifier: AGPL-3.0-or-later

//! `Fold NIC`のloopback限定object交換プロトコル。

#![forbid(unsafe_code)]

use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;

use cid::Cid;
use fold_core::{WorldId, WorldRef, WorldlineId};
use fold_kamii::{KamiiInspectionRequest, KamiiVerdict, invoke_kamii_adapter};
use fold_store::{LocalCas, PutReceipt};
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use serde::{Deserialize, Serialize};

pub const OBJECT_PROTOCOL: &str = "/fold-nic/object-exchange/0.1.0";
pub const DEFAULT_MAX_OBJECT_BYTES: u64 = 8 * 1024 * 1024;

/// Kamii adapter呼び出しをfail-closedで待つ上限。Stage 0 mock hook向けの暫定値。
pub const KAMII_GATE_TIMEOUT_MILLIS: u64 = 250;

/// Stage 0のplaceholder adapter。実inspection判定ロジックは`NOT_IMPLEMENTED`のまま、
/// 常に`Allow`を返す。timeout／crashをfail-closedする配線自体を検証する目的でのみ使う。
fn default_kamii_adapter(_: &KamiiInspectionRequest) -> KamiiVerdict {
    KamiiVerdict::Allow
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectRequest {
    pub cid: String,
    pub world_id: String,
    pub worldline_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectResponse {
    Found {
        cid: String,
        world_id: String,
        worldline_id: String,
        bytes: Vec<u8>,
    },
    Error {
        code: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerProtocolError {
    pub code: &'static str,
    pub detail: String,
}

impl PeerProtocolError {
    #[must_use]
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl Display for PeerProtocolError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code, self.detail)
    }
}

impl Error for PeerProtocolError {}

/// listen先が明示的なloopback TCPだけであることを検査する。
///
/// # Errors
///
/// loopback IP、TCP portの順でない場合に返す。
pub fn validate_loopback_listen(address: &Multiaddr) -> Result<(), PeerProtocolError> {
    let protocols = address.iter().collect::<Vec<_>>();
    match protocols.as_slice() {
        [Protocol::Ip4(ip), Protocol::Tcp(_)] if ip.is_loopback() => Ok(()),
        [Protocol::Ip6(ip), Protocol::Tcp(_)] if ip.is_loopback() => Ok(()),
        _ => Err(PeerProtocolError::new(
            "P2P_NON_LOOPBACK_ADDRESS",
            "listen先はloopback IPとTCP portだけで指定してください",
        )),
    }
}

/// dial先末尾のPeerIdを分離し、transport部分もloopback限定で検査する。
///
/// # Errors
///
/// `/p2p/<PeerId>`がない、またはtransportがloopback TCPでない場合に返す。
pub fn split_loopback_dial(address: &Multiaddr) -> Result<(PeerId, Multiaddr), PeerProtocolError> {
    let mut transport = address.clone();
    let Some(Protocol::P2p(peer)) = transport.pop() else {
        return Err(PeerProtocolError::new(
            "P2P_PEER_ID_REQUIRED",
            "dial先末尾に/p2p/<PeerId>が必要です",
        ));
    };
    validate_loopback_listen(&transport)?;
    Ok((peer, transport))
}

/// 応答のWorld文脈とCIDを検査し、検査済みbyte列だけをlocal CASへ入れる。
///
/// Kamiiの`Stage 0` placeholder adapter（常に`Allow`）を経由する。実inspection判定ロジックは
/// 未実装だが、adapter呼び出しのtimeout／crash fail-closed配線はこの経路で有効になる。
///
/// # Errors
///
/// remote error、文脈不一致、CID不一致、Kamii拒否、CAS書込み失敗時に返す。
pub fn accept_response(
    cas: &LocalCas,
    request: &ObjectRequest,
    response: ObjectResponse,
) -> Result<PutReceipt, PeerProtocolError> {
    accept_response_with_kamii_adapter(cas, request, response, default_kamii_adapter)
}

/// [`accept_response`]と同じ検査に加え、Kamii adapterを差し替えられる版。
///
/// 実process分離やtest向けにadapterを注入する用途を想定する。adapterのtimeoutまたは
/// crashは[`fold_kamii::KamiiOutcome::resolved_verdict`]によりfail-closedへ倒され、
/// `Allow`以外はCASへ書き込む前に拒否する。
///
/// # Errors
///
/// remote error、文脈不一致、CID不一致、Kamii拒否、CAS書込み失敗時に返す。
pub fn accept_response_with_kamii_adapter<F>(
    cas: &LocalCas,
    request: &ObjectRequest,
    response: ObjectResponse,
    kamii_adapter: F,
) -> Result<PutReceipt, PeerProtocolError>
where
    F: FnOnce(&KamiiInspectionRequest) -> KamiiVerdict + Send + 'static,
{
    let requested_cid = request
        .cid
        .parse::<Cid>()
        .map_err(|_| PeerProtocolError::new("P2P_INVALID_CID", "要求CIDを解釈できません"))?;
    let _world = WorldRef::new(
        WorldId::parse(request.world_id.clone())
            .map_err(|error| PeerProtocolError::new("P2P_INVALID_WORLD", error.to_string()))?,
        WorldlineId::parse(request.worldline_id.clone())
            .map_err(|error| PeerProtocolError::new("P2P_INVALID_WORLDLINE", error.to_string()))?,
    );

    let ObjectResponse::Found {
        cid,
        world_id,
        worldline_id,
        bytes,
    } = response
    else {
        let ObjectResponse::Error { code } = response else {
            unreachable!();
        };
        return Err(PeerProtocolError::new(
            "P2P_REMOTE_ERROR",
            format!("remote peer error: {code}"),
        ));
    };

    if world_id != request.world_id || worldline_id != request.worldline_id {
        return Err(PeerProtocolError::new(
            "P2P_WORLD_CONTEXT_MISMATCH",
            "応答のWorldまたはWorldlineが要求と一致しません",
        ));
    }
    if cid != request.cid {
        return Err(PeerProtocolError::new(
            "P2P_CID_MISMATCH",
            "応答が主張するCIDが要求と一致しません",
        ));
    }

    let byte_len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    let inspection = KamiiInspectionRequest {
        cid: cid.clone(),
        byte_len,
    };
    let outcome = invoke_kamii_adapter(
        inspection,
        Duration::from_millis(KAMII_GATE_TIMEOUT_MILLIS),
        kamii_adapter,
    );
    if outcome.resolved_verdict() != KamiiVerdict::Allow {
        return Err(PeerProtocolError::new(
            "P2P_KAMII_DENIED",
            format!("Kamii adapterがobjectをCASへ書き込む前に拒否しました: {outcome:?}"),
        ));
    }

    let receipt = cas
        .put(&bytes)
        .map_err(|error| PeerProtocolError::new("P2P_CAS_REJECTED", error.to_string()))?;
    if receipt.cid != requested_cid {
        return Err(PeerProtocolError::new(
            "P2P_CID_MISMATCH",
            "受信byte列から再計算したCIDが要求と一致しません",
        ));
    }
    Ok(receipt)
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
            "fold-peer-test-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("root");
        LocalCas::open(root, 1024).expect("cas")
    }

    fn request(bytes: &[u8]) -> ObjectRequest {
        ObjectRequest {
            cid: cid_for(bytes).to_string(),
            world_id: "fold-nic-forge".into(),
            worldline_id: "stage0".into(),
        }
    }

    #[test]
    fn loopback_tcpだけを受理する() {
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/0".parse().expect("address");
        validate_loopback_listen(&address).expect("loopback");
        let public: Multiaddr = "/ip4/192.0.2.1/tcp/7000".parse().expect("address");
        assert_eq!(
            validate_loopback_listen(&public).expect_err("reject").code,
            "P2P_NON_LOOPBACK_ADDRESS"
        );
    }

    #[test]
    fn dial先にpeer_idを要求する() {
        let address: Multiaddr = "/ip4/127.0.0.1/tcp/7000".parse().expect("address");
        assert_eq!(
            split_loopback_dial(&address).expect_err("reject").code,
            "P2P_PEER_ID_REQUIRED"
        );
    }

    #[test]
    fn 正しい応答だけをcasへ採用する() {
        let bytes = b"public fixture";
        let request = request(bytes);
        let receipt = accept_response(
            &temporary_cas(),
            &request,
            ObjectResponse::Found {
                cid: request.cid.clone(),
                world_id: request.world_id.clone(),
                worldline_id: request.worldline_id.clone(),
                bytes: bytes.to_vec(),
            },
        )
        .expect("accepted");
        assert_eq!(receipt.cid.to_string(), request.cid);
    }

    #[test]
    fn cid主張とbyte列の不一致を拒否する() {
        let request = request(b"expected");
        let error = accept_response(
            &temporary_cas(),
            &request,
            ObjectResponse::Found {
                cid: request.cid.clone(),
                world_id: request.world_id.clone(),
                worldline_id: request.worldline_id.clone(),
                bytes: b"tampered".to_vec(),
            },
        )
        .expect_err("reject");
        assert_eq!(error.code, "P2P_CID_MISMATCH");
    }

    #[test]
    fn world文脈不一致を拒否する() {
        let bytes = b"fixture";
        let request = request(bytes);
        let error = accept_response(
            &temporary_cas(),
            &request,
            ObjectResponse::Found {
                cid: request.cid.clone(),
                world_id: "other-world".into(),
                worldline_id: request.worldline_id.clone(),
                bytes: bytes.to_vec(),
            },
        )
        .expect_err("reject");
        assert_eq!(error.code, "P2P_WORLD_CONTEXT_MISMATCH");
    }

    #[test]
    fn kamiiがdenyを返せば書込み前に拒否する() {
        let cas = temporary_cas();
        let bytes = b"quarantine candidate";
        let request = request(bytes);
        let error = accept_response_with_kamii_adapter(
            &cas,
            &request,
            ObjectResponse::Found {
                cid: request.cid.clone(),
                world_id: request.world_id.clone(),
                worldline_id: request.worldline_id.clone(),
                bytes: bytes.to_vec(),
            },
            |_| KamiiVerdict::Deny,
        )
        .expect_err("reject");
        assert_eq!(error.code, "P2P_KAMII_DENIED");
        assert!(cas.get(&cid_for(bytes)).is_err());
    }

    #[test]
    fn kamiiがtimeoutしても書込み前に拒否する() {
        let cas = temporary_cas();
        let bytes = b"timeout candidate";
        let request = request(bytes);
        let error = accept_response_with_kamii_adapter(
            &cas,
            &request,
            ObjectResponse::Found {
                cid: request.cid.clone(),
                world_id: request.world_id.clone(),
                worldline_id: request.worldline_id.clone(),
                bytes: bytes.to_vec(),
            },
            |_| {
                std::thread::sleep(Duration::from_secs(2));
                KamiiVerdict::Allow
            },
        )
        .expect_err("reject");
        assert_eq!(error.code, "P2P_KAMII_DENIED");
        assert!(cas.get(&cid_for(bytes)).is_err());
    }
}
