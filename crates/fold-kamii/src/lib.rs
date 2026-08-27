// SPDX-License-Identifier: AGPL-3.0-or-later

//! Kamii adapter呼び出し境界のStage 0 mock hook。
//!
//! 実Kamiiはout-of-process inspectionを行う別process化を想定するが、
//! この crateはprocess分離そのものは実装しない。ここで固定するのは
//! 「adapterのtimeoutまたはcrashを`Allow`へ変換しない」という呼び出し境界だけである。
//!
//! - 実subprocess起動、IPC、shared memoryはこの crateの範囲外
//! - 取得objectの実行はこの crateからもどこからも行わない
//! - adapterは`KamiiVerdict`を返すだけであり、Fold NIC coreの最終許可判定そのものではない

#![forbid(unsafe_code)]

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// inspection対象を指すmetadataのみ。生byte列をこの crateへ渡さない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KamiiInspectionRequest {
    pub cid: String,
    pub byte_len: u64,
}

/// adapterの判定そのもの。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum KamiiVerdict {
    Allow,
    Deny,
    Quarantine,
}

/// adapter呼び出しの生の結果。timeoutとcrashを区別して保持し、
/// どちらも`KamiiVerdict::Allow`へ暗黙変換しない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KamiiOutcome {
    Verdict(KamiiVerdict),
    Timeout,
    Crashed,
}

impl KamiiOutcome {
    /// 呼び出し側が実際に採用してよい判定。
    ///
    /// timeoutとcrashは区別して記録済みだが、採用時はどちらも
    /// `KamiiVerdict::Deny`へfail-closedする。
    #[must_use]
    pub const fn resolved_verdict(&self) -> KamiiVerdict {
        match self {
            Self::Verdict(verdict) => *verdict,
            Self::Timeout | Self::Crashed => KamiiVerdict::Deny,
        }
    }
}

/// out-of-process inspectionを模したadapter境界を呼び出す。
///
/// `adapter`は別threadで実行する。`timeout`以内に応答がなければ
/// [`KamiiOutcome::Timeout`]を返し、`adapter`がpanicして応答を送れなければ
/// [`KamiiOutcome::Crashed`]を返す。いずれの場合も呼び出し元へ`Allow`を
/// 渡さない（[`KamiiOutcome::resolved_verdict`]を参照）。
///
/// 実processへの分離、他adapterとの多重化は次checkpointで扱う。
pub fn invoke_kamii_adapter<F>(
    request: KamiiInspectionRequest,
    timeout: Duration,
    adapter: F,
) -> KamiiOutcome
where
    F: FnOnce(&KamiiInspectionRequest) -> KamiiVerdict + Send + 'static,
{
    let (sender, receiver) = mpsc::channel();
    // sendに失敗するのは、この呼び出しが既にtimeoutで抜けた後だけであり、
    // その場合はadapter threadを待たずに戻ってよい。
    let _handle = thread::spawn(move || {
        let verdict = adapter(&request);
        let _ = sender.send(verdict);
    });

    match receiver.recv_timeout(timeout) {
        Ok(verdict) => KamiiOutcome::Verdict(verdict),
        Err(mpsc::RecvTimeoutError::Timeout) => KamiiOutcome::Timeout,
        Err(mpsc::RecvTimeoutError::Disconnected) => KamiiOutcome::Crashed,
    }
}

/// [`invoke_kamii_adapter`]の呼び出し側向け既定timeout。
///
/// `fold-peer`／`fold-gateway`など、object受入・配信前のgateとして呼ぶ側で共有する。
pub const DEFAULT_GATE_TIMEOUT_MILLIS: u64 = 250;

/// Stage 0のplaceholder adapter。実inspection判定ロジックは`NOT_IMPLEMENTED`のまま、
/// 常に`Allow`を返す。呼び出し側のtimeout／crash fail-closed配線を検証する目的でのみ使う。
#[must_use]
pub fn allow_all_placeholder_adapter(_: &KamiiInspectionRequest) -> KamiiVerdict {
    KamiiVerdict::Allow
}

/// [`invoke_kamii_adapter`]を[`DEFAULT_GATE_TIMEOUT_MILLIS`]で呼び、
/// fail-closedへ解決済みの[`KamiiVerdict`]だけを返す。
///
/// 呼び出し側は`Timeout`と`Crashed`を区別する必要がない場合にこちらを使う。
pub fn gate<F>(request: KamiiInspectionRequest, adapter: F) -> KamiiVerdict
where
    F: FnOnce(&KamiiInspectionRequest) -> KamiiVerdict + Send + 'static,
{
    invoke_kamii_adapter(
        request,
        Duration::from_millis(DEFAULT_GATE_TIMEOUT_MILLIS),
        adapter,
    )
    .resolved_verdict()
}

#[cfg(test)]
mod tests {
    use super::{
        KamiiInspectionRequest, KamiiOutcome, KamiiVerdict, allow_all_placeholder_adapter, gate,
        invoke_kamii_adapter,
    };
    use std::thread;
    use std::time::Duration;

    fn request() -> KamiiInspectionRequest {
        KamiiInspectionRequest {
            cid: "bafyreitest0000000000000000000000000000000000000000000000000".to_owned(),
            byte_len: 5_651,
        }
    }

    #[test]
    fn adapterがallowを返せばverdictはallow() {
        let outcome =
            invoke_kamii_adapter(request(), Duration::from_secs(1), |_| KamiiVerdict::Allow);
        assert_eq!(outcome, KamiiOutcome::Verdict(KamiiVerdict::Allow));
        assert_eq!(outcome.resolved_verdict(), KamiiVerdict::Allow);
    }

    #[test]
    fn adapterがdenyを返せばverdictはdeny() {
        let outcome =
            invoke_kamii_adapter(request(), Duration::from_secs(1), |_| KamiiVerdict::Deny);
        assert_eq!(outcome, KamiiOutcome::Verdict(KamiiVerdict::Deny));
        assert_eq!(outcome.resolved_verdict(), KamiiVerdict::Deny);
    }

    #[test]
    fn adapterがtimeoutすればresolved_verdictはallowにならない() {
        let outcome = invoke_kamii_adapter(request(), Duration::from_millis(20), |_| {
            thread::sleep(Duration::from_secs(2));
            KamiiVerdict::Allow
        });
        assert_eq!(outcome, KamiiOutcome::Timeout);
        assert_eq!(outcome.resolved_verdict(), KamiiVerdict::Deny);
    }

    #[test]
    fn adapterがpanicしてもresolved_verdictはallowにならない() {
        let outcome = invoke_kamii_adapter(request(), Duration::from_secs(1), |_| {
            panic!("adapter crash mock")
        });
        assert_eq!(outcome, KamiiOutcome::Crashed);
        assert_eq!(outcome.resolved_verdict(), KamiiVerdict::Deny);
    }

    #[test]
    fn allow_all_placeholder_adapterは常にallowを返す() {
        assert_eq!(
            allow_all_placeholder_adapter(&request()),
            KamiiVerdict::Allow
        );
    }

    #[test]
    fn gateはresolved_verdictだけを返す() {
        assert_eq!(
            gate(request(), allow_all_placeholder_adapter),
            KamiiVerdict::Allow
        );
        assert_eq!(gate(request(), |_| KamiiVerdict::Deny), KamiiVerdict::Deny);
        assert_eq!(
            gate(request(), |_| {
                thread::sleep(Duration::from_secs(2));
                KamiiVerdict::Allow
            }),
            KamiiVerdict::Deny
        );
    }

    #[test]
    fn verdictはjsonへ往復できる() {
        let json = serde_json::to_string(&KamiiVerdict::Quarantine).expect("serialize");
        assert_eq!(json, "\"QUARANTINE\"");
        let back: KamiiVerdict = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, KamiiVerdict::Quarantine);
    }
}
