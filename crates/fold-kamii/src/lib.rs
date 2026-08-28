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

use std::sync::{Condvar, LazyLock, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

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

/// adapter呼び出しの生の結果。timeout、crash、容量超過を区別して保持し、
/// どれも`KamiiVerdict::Allow`へ暗黙変換しない。
///
/// `Deny`は安全判定そのもの（403相当）、`Timeout`／`Crashed`／`Saturated`は
/// 障害・容量診断（503相当）であり、呼び出し側の表示層はこれらを混同しない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum KamiiOutcome {
    Verdict(KamiiVerdict),
    Timeout,
    Crashed,
    /// 同時実行上限に達し、`timeout`以内にadapter threadの枠を確保できなかった。
    Saturated,
}

impl KamiiOutcome {
    /// 呼び出し側が実際に採用してよい判定。
    ///
    /// timeout、crash、容量超過は区別して記録済みだが、採用時はどれも
    /// `KamiiVerdict::Deny`へfail-closedする。
    #[must_use]
    pub const fn resolved_verdict(&self) -> KamiiVerdict {
        match self {
            Self::Verdict(verdict) => *verdict,
            Self::Timeout | Self::Crashed | Self::Saturated => KamiiVerdict::Deny,
        }
    }
}

/// `invoke_kamii_adapter`が同時に起動するadapter threadの上限。
///
/// timeoutしたadapterが無制限にthreadを残さないよう、実行中thread数をこの値で
/// bound する。上限到達時は新規呼び出しを[`KamiiOutcome::Saturated`]として拒否する。
const MAX_CONCURRENT_ADAPTER_CALLS: usize = 8;

static ADAPTER_POOL: LazyLock<BoundedPool> =
    LazyLock::new(|| BoundedPool::new(MAX_CONCURRENT_ADAPTER_CALLS));

/// 固定容量のcounting semaphore相当。`unsafe`なしで`Mutex`＋`Condvar`だけで実装する。
struct BoundedPool {
    available: Mutex<usize>,
    condvar: Condvar,
}

impl BoundedPool {
    fn new(capacity: usize) -> Self {
        Self {
            available: Mutex::new(capacity),
            condvar: Condvar::new(),
        }
    }

    /// `timeout`以内に空き枠を確保できれば`true`、確保できなければ`false`を返す。
    fn try_acquire(&self, timeout: Duration) -> bool {
        let guard = self
            .available
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (mut guard, _) = self
            .condvar
            .wait_timeout_while(guard, timeout, |available| *available == 0)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *guard == 0 {
            false
        } else {
            *guard -= 1;
            true
        }
    }

    fn release(&self) {
        let mut guard = self
            .available
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard += 1;
        self.condvar.notify_one();
    }
}

/// out-of-process inspectionを模したadapter境界を呼び出す。
///
/// `adapter`は別threadで実行する。同時実行数は[`MAX_CONCURRENT_ADAPTER_CALLS`]で
/// boundし、枠を確保できなければ[`KamiiOutcome::Saturated`]を返す。`timeout`以内に
/// 応答がなければ[`KamiiOutcome::Timeout`]を返し、`adapter`がpanicして応答を送れ
/// なければ[`KamiiOutcome::Crashed`]を返す。いずれの場合も呼び出し元へ`Allow`を
/// 渡さない（[`KamiiOutcome::resolved_verdict`]を参照）。
///
/// adapterがpanicした場合も枠は必ず解放する（`catch_unwind`で捕捉してから解放する）。
/// これにより、timeoutまたはcrashを繰り返すadapterが枠を専有し続けることを防ぐ。
///
/// 実processへの分離は次checkpointで扱う。
pub fn invoke_kamii_adapter<F>(
    request: KamiiInspectionRequest,
    timeout: Duration,
    adapter: F,
) -> KamiiOutcome
where
    F: FnOnce(&KamiiInspectionRequest) -> KamiiVerdict + Send + 'static,
{
    let acquire_started = Instant::now();
    if !ADAPTER_POOL.try_acquire(timeout) {
        return KamiiOutcome::Saturated;
    }
    let remaining = timeout.saturating_sub(acquire_started.elapsed());

    let (sender, receiver) = mpsc::channel();
    // sendに失敗するのは、この呼び出しが既にtimeoutで抜けた後だけであり、
    // その場合はadapter threadを待たずに戻ってよい。枠はpanic時も含め必ず解放する。
    let _handle = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| adapter(&request)));
        ADAPTER_POOL.release();
        if let Ok(verdict) = result {
            let _ = sender.send(verdict);
        }
    });

    match receiver.recv_timeout(remaining) {
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
        BoundedPool, KamiiInspectionRequest, KamiiOutcome, KamiiVerdict,
        MAX_CONCURRENT_ADAPTER_CALLS, allow_all_placeholder_adapter, gate, invoke_kamii_adapter,
    };
    use std::sync::Arc;
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
    fn bounded_poolは容量まで確保しそれ以上はtimeoutする() {
        let pool = BoundedPool::new(2);
        assert!(pool.try_acquire(Duration::from_millis(50)));
        assert!(pool.try_acquire(Duration::from_millis(50)));
        assert!(!pool.try_acquire(Duration::from_millis(50)));
        pool.release();
        assert!(pool.try_acquire(Duration::from_millis(50)));
    }

    #[test]
    fn bounded_poolはrelease後に待機中のacquireを起こす() {
        let pool = Arc::new(BoundedPool::new(1));
        assert!(pool.try_acquire(Duration::from_millis(50)));
        let waiter_pool = Arc::clone(&pool);
        let handle = thread::spawn(move || waiter_pool.try_acquire(Duration::from_secs(2)));
        thread::sleep(Duration::from_millis(50));
        pool.release();
        assert!(handle.join().expect("waiter thread"));
    }

    #[test]
    fn adapterの繰り返しpanicでも枠を使い果たさない() {
        for _ in 0..(MAX_CONCURRENT_ADAPTER_CALLS + 2) {
            let outcome = invoke_kamii_adapter(request(), Duration::from_secs(1), |_| {
                panic!("adapter crash mock")
            });
            assert_eq!(outcome, KamiiOutcome::Crashed);
        }
    }

    #[test]
    fn saturatedもfail_closedへ解決する() {
        assert_eq!(
            KamiiOutcome::Saturated.resolved_verdict(),
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
