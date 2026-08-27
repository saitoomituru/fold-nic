# Kamii mock hook

作成日時: 2026-08-27 17:30:00 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: Kamii adapter呼び出し境界のうち、Stage 0安全境界の「adapter timeout／crashをALLOWへ潰さない」を最小libraryとして固定する
- 非目標: 実subprocess分離、IPC、shared memory、実際のKamii inspection判定ロジックは実装しない。Fold NIC coreの最終許可判定への配線（Gateway／peerからの呼び出し）は次checkpointへ分離する

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `a8b6164`（起点）
- worktree: `crates/fold-kamii`新設
- upstream／contract: ZeroRoomLab-manifest #28、fold-nic #1の「Kamii mock hook」checkpoint、`docs/development/stage0-bare-metal-runtime.ja.md`のprocess境界（`kamii-adapter -> optional out-of-process inspection`）

## 既存tool確認

- 探索したtool: `fold-core::FoldErrorCode::AdapterError`（既存の汎用adapter error code）、`fold-provenance`（新規独立crateの前例）
- reuse／extend／adapt／create: 新規crate`fold-kamii`を作成。既存crateにadapter呼び出し境界を持つものがなく、責務分離のため独立させた。`FoldErrorCode`は今回追加せず、adapter呼び出し自体はErrorではなく`KamiiOutcome`という専用の判定型で表現した（timeout／crashはprotocol errorではなく、fail-closedへ解決すべき正常な観測結果のため）
- 判断理由: `fold-core`はI/Oなしのprotocol型専用であり、adapter呼び出しのthread境界を持ち込まない方が既存境界と整合する

## 入出力・副作用・権限

- 入力: `KamiiInspectionRequest`（CID文字列とbyte長のみ。生byte列は渡さない）
- 出力: `KamiiOutcome`（`Verdict` / `Timeout` / `Crashed`）
- filesystem副作用: なし
- network: なし
- authority: この crateは許可判定を行わない。`resolved_verdict()`はtimeout／crashを常に`Deny`へfail-closedするが、呼び出し側が実際にDenyを強制するかは呼び出し側の責務
- secret境界: private key、token、生content bytesを扱わない

## 変更

- file: `Cargo.toml`（workspace membersへ`crates/fold-kamii`追加）、`crates/fold-kamii/`新設（`Cargo.toml`、`src/lib.rs`）
- contract／Schema差分: なし（新規crateの公開型のみ）

## `[FACT]` 事実・観測

- `invoke_kamii_adapter`は`std::thread::spawn`でadapter closureを別threadへ隔離し、`mpsc::Receiver::recv_timeout`で待つ。
- adapterがtimeout以内に`KamiiVerdict`を返せば`KamiiOutcome::Verdict(_)`、timeout超過で`KamiiOutcome::Timeout`、adapter threadがpanicして送信できなければ`KamiiOutcome::Crashed`を返す。
- `KamiiOutcome::resolved_verdict()`は`Timeout`と`Crashed`のどちらも`KamiiVerdict::Deny`へ変換し、`Allow`を返す経路がない。
- timeout発生時、adapter threadはdetachされたまま存続する可能性がある（`JoinHandle`を待たない）。これは既知のUNKNOWNとして残す。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`（mock hookのみ、実process分離なし）
- integration: 未実施（Gateway／peerからの呼び出しは次checkpoint）
- automated verification: `cargo test -p fold-kamii` 5件成功（allow／deny／timeout／crash／serde往復）。`cargo test --workspace --locked`全crate成功。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`通過。`python3 -B -m unittest discover -s tests` 5件成功。`foldnic_dev.py validate`／`doctor` PASS
- packaging: 該当なし
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- 「adapterを模したclosureを渡す」形にしたことで、実Kamiiが実subprocessへ置き換わっても、呼び出し側の型（`KamiiOutcome`／`resolved_verdict`）は変えずに済む設計になっている。
- timeoutとcrashを`KamiiOutcome`の別variantとして残したのは、fail-closedという結論を共有しつつ、観測ログ上はどちらが起きたか区別できるようにするため。

## `[HYPOTHESIS]` 次の仮説

- `fold-gateway`または`fold-peer`が取得objectを配信する前に`invoke_kamii_adapter`を呼び、`resolved_verdict() == Deny`ならreceiptへ`KAMII_DENIED`相当のstateを残す形で配線できる。
- 実process分離（`kamii-adapter`を別binaryにする）へ進む際も、`KamiiInspectionRequest`／`KamiiVerdict`をIPC境界のSchemaとしてそのまま再利用できる可能性がある。

## `[UNKNOWN]` 未解決・未試験

- 実subprocess起動、IPC（stdin/stdout、UDS等）、shared memoryは未実装
- timeout発生後もadapter threadが存続する場合のresource解放・上限は未設計
- 複数adapter呼び出しの多重度・queueingは未検討
- Gateway／peerからの実配線と統合testは未実施
- 実Kamii inspection判定ロジックそのもの（何を判定基準にするか）は本checkpointの範囲外

## command・検証

```console
cargo test -p fold-kamii
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests -v
python3 -B scripts/foldnic_dev.py validate
python3 -B scripts/foldnic_dev.py doctor
git diff --check
```

## rollback／unmount

- `crates/fold-kamii`はどこからも呼び出されていない独立crateであり、workspace membersからの削除とcrate directory削除だけで復元不要に切り離せる。

## 引継ぎ

- 次の作業: `fold-gateway`／`fold-peer`からの実配線、実process分離の設計着手
- User Gate: なし（本checkpointはmock hookのみで、User Gateが必要な設計決定を含まない）
- 関連Issue／commit: ZeroRoomLab-manifest #28、fold-nic #1、commitは作成後追記
