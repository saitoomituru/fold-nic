# Kamii raw outcome分離、CID検証順、bounded concurrency（Lane D / #4）

作成日時: 2026-08-28 10:55:00 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: #4が指摘した3点を解消する。(1) 明示`Deny`／`Timeout`／`Crashed`が同じ拒否へ縮退し安全判定と障害診断が混同される、(2) peerが受信bytesのCIDを検証する前に申告CIDでKamiiへ問い合わせている、(3) timeout後のadapter threadが無制限に残る
- 非目標: 実Kamii process分離（#4のUNKNOWN、Lane E）、実inspection判定ロジックは実装しない

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `16260c3`（起点）
- worktree: `crates/fold-kamii/src/lib.rs`、`crates/fold-peer/src/lib.rs`、`crates/fold-gateway/src/lib.rs`
- upstream／contract: fold-nic #1 issuecomment-5447652557（Lane D）、#4

## 既存tool確認

- 探索したtool: `fold_kamii::invoke_kamii_adapter`（既存raw outcome機構）、`fold_store::cid_for`（既存CID再計算関数、既に`fold-peer`のCAS書込み後検証で使用中）
- reuse／extend／adapt／create: 新規crateは作らず、`fold-kamii`へ`KamiiOutcome::Saturated`とbounded pool（`BoundedPool`、`unsafe`なし、`Mutex`＋`Condvar`）を追加。`fold-peer`／`fold-gateway`は`gate()`（resolved-onlyの薄いwrapper）から`invoke_kamii_adapter`（raw outcome）呼び出しへ戻し、outcome種別ごとに別error code／HTTP statusへ写像する`kamii_rejection_error`をそれぞれ追加
- 判断理由: 既存の`gate()`はTimeoutとCrashedを区別せず`KamiiVerdict`だけを返す設計だったため、raw outcomeを必要とする呼び出し側は`invoke_kamii_adapter`を直接呼ぶ必要があった

## 入出力・副作用・権限

- 入力: 変わらず（`KamiiInspectionRequest`）。ただし`fold-peer`側はKamiiへ渡す`cid`を「応答が自己申告したcid」から「受信bytesから今計算した確定cid」へ変更
- 出力: `KamiiOutcome`に`Saturated`variantを追加。`fold-peer`は`P2P_KAMII_DENIED`（明示Deny／Quarantine）と`P2P_KAMII_UNAVAILABLE`（Timeout／Crashed／Saturated）を分離。`fold-gateway`は403（`GATEWAY_KAMII_DENIED`）と503 + `Retry-After`（`GATEWAY_KAMII_UNAVAILABLE`）を分離
- filesystem副作用: なし
- network: なし
- authority: 変わらず、既定adapterは常に`Allow`
- secret境界: 変更なし

## `[FACT]` 事実・観測

- `fold-kamii::invoke_kamii_adapter`はadapter thread起動前に`ADAPTER_POOL`（容量`MAX_CONCURRENT_ADAPTER_CALLS = 8`の`BoundedPool`）から枠を確保する。`timeout`以内に枠を確保できなければthreadを起動せず`KamiiOutcome::Saturated`を返す。
- adapter threadは`std::panic::catch_unwind`でpanicを捕捉し、`ADAPTER_POOL.release()`をpanicの有無に関わらず必ず呼んでから結果を送る。これによりtimeoutまたはcrashを繰り返すadapterが枠を専有し続けることを防ぐ（新規test「adapterの繰り返しpanicでも枠を使い果たさない」で、容量+2回連続panicさせても全てCrashedとして返り続ける＝枠が涸れないことを確認）。
- `BoundedPool`自体は、testの並行実行による他testとの干渉を避けるため、grepでglobal staticではなくlocal instanceを生成してtestしている（`bounded_poolは容量まで確保しそれ以上はtimeoutする`、`bounded_poolはrelease後に待機中のacquireを起こす`）。
- `fold-peer::accept_response_with_kamii_adapter`は、応答の`cid`文字列一致検査（既存）に加えて、`cid_for(&bytes)`で計算した確定CIDが`requested_cid`と一致することを検証してから、その確定CID文字列をKamiiへ渡すよう変更した。新規test「kamiiには申告cidではなく確定cidを渡す」で、adapterが受け取った`inspection.cid`が`cid_for(bytes)`と一致することを確認した。
- `fold-peer`のKamii拒否は`KamiiOutcome::Verdict(_)`なら`P2P_KAMII_DENIED`、`Timeout`／`Crashed`／`Saturated`なら`P2P_KAMII_UNAVAILABLE`を返す。既存の「timeoutしても拒否する」testはこの新codeへ更新した。
- `fold-gateway`は同様に403／503を分け、503応答には`Retry-After: 1`（固定秒数のStage 0 placeholder値）を付与する。testで`RETRY_AFTER`headerの存在を確認した。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`
- integration: `fold-peer`／`fold-gateway`両方の実行経路に反映済み
- automated verification: `cargo test -p fold-kamii` 11件（既存7＋新規4）、`cargo test -p fold-peer` 8件（既存7＋新規1、CID順序test）、`cargo test -p fold-gateway` 7件（既存5＋timeout test更新＋変更なしdeny test）。`cargo test --workspace --locked`全成功。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`通過。Python 5件、validator PASS
- packaging: 該当なし
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- CID検証順の修正により、「Kamiiが見るidentityは常にbytesから確定した値である」という不変が成立した。以前は応答の自己申告cidをそのままKamiiへ渡しており、申告と実bytesが食い違う場合でも（最終的にCAS書込み前後で検出はされるが）Kamiiには偽の対象識別子が渡っていた。
- `Deny`と`Timeout`／`Crashed`／`Saturated`の分離は、issue #2のHTTP mapping（403 vs 503）とも整合する形にした。ただしこの分離が意味を持つのは実inspection判定ロジックが入ってから（現状は常に`Allow`のplaceholder）であり、今回の変更で悪性objectの検出精度が上がったわけではない。
- bounded poolのtestは意図的にglobal static（`ADAPTER_POOL`）を直接検証せず、`BoundedPool`型を独立にtestすることで、並行実行される他testとの干渉によるflakinessを避けた。global static側の健全性は「panicの繰り返しでも枠が涸れない」という間接testで補っている。

## `[HYPOTHESIS]` 次の仮説

- `MAX_CONCURRENT_ADAPTER_CALLS`は固定値8だが、実process分離（Lane E）後はprocess数の実際の上限や、呼び出し元ごとのquotaが必要になる可能性がある。
- `Retry-After`の1秒固定値は診断GUI（Lane F、#2）と接続する際、実際のadapter復旧見込みに応じた値へ差し替える余地がある。

## `[UNKNOWN]` 未解決・未試験

- 実Kamii process protocolとauthority（#4のUNKNOWNのまま）
- retry policyと利用者再試行の責務
- publisher鍵→World authority Registryのbinding先（#5、既存User Gate）
- `Saturated`が実際に発生する高負荷状況でのGateway全体のふるまい（負荷testは未実施）

## command・検証

```console
cargo test -p fold-kamii
cargo test -p fold-peer
cargo test -p fold-gateway
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests
python3 -B scripts/foldnic_dev.py validate
git diff --check
```

## rollback／unmount

- `fold-kamii`の`Saturated`／`BoundedPool`を削除し、`fold-peer`／`fold-gateway`を`gate()`呼び出しへ戻せば元の挙動に戻せる。CID検証順の変更は`fold-peer`単独でrevert可能。

## 引継ぎ

- 次の作業: 他のGreen lane（Lane A完了、Lane B/C/E/F/G継続）
- User Gate: なし（#4のUNKNOWNは維持したまま局所実装で閉じられる範囲のみ対応）
- 関連Issue／commit: fold-nic #1、#4、commitは作成後追記
