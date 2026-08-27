# Kamii mock hookをfold-gatewayへ配線する

作成日時: 2026-08-27 19:05:00 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: `fold-peer`のingest側に続き、`fold-gateway`のegress側（`GET /v0/objects/{cid}`）にもKamii mock hookを配線し、CAS objectを配信する前にadapterのtimeout／crashをfail-closedする
- 非目標: 実inspection判定ロジックは実装しない。既定adapterは常に`Allow`のまま。実process分離は次checkpointへ分離する

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `29ac27b`（起点）
- worktree: `crates/fold-gateway/src/lib.rs`、`crates/fold-gateway/Cargo.toml`
- upstream／contract: fold-nic #1、`development-log/20260827-1845__Kamii-fold-peer配線.ja.md`の引継ぎ（次の作業: `fold-gateway`側のKamii配線）

## 既存tool確認

- 探索したtool: `fold_kamii::invoke_kamii_adapter`、`fold-peer`の`accept_response_with_kamii_adapter`パターン
- reuse／extend／adapt／create: 新規crateは作らず、既存`router(cas)`はそのままの互換APIとして残し、adapter注入可能な`router_with_kamii_adapter(cas, adapter)`を追加した。axumの`State`をtupleから`GatewayState { cas, kamii_adapter }`という小さな構造体へ変更した
- 判断理由: `fold-peer`側と同じ「既定関数へ委譲し、adapterを注入できる版を別関数として公開する」形にすることで、両crateで一貫したtest可能性を持たせた

## 入出力・副作用・権限

- 入力: 既存の`cid_text` path parameterに加え、CID文字列とbyte長だけの`KamiiInspectionRequest`
- 出力: 既存の200/404/400/409に加え、Kamii拒否時は403 `GATEWAY_KAMII_DENIED`
- filesystem副作用: なし（読み取りのみ、CASへの書込みはこの経路にない）
- network: なし
- authority: 既定adapterは常に`Allow`のため、現時点でこの配線が防ぐのはadapter呼び出し自体のtimeout／crashだけ
- secret境界: 変更なし

## `[FACT]` 事実・観測

- `get_object`はCAS読み取り成功後、`tokio::task::spawn_blocking`内で`invoke_kamii_adapter`を呼び、`resolved_verdict() != Allow`なら403を返す。
- `spawn_blocking`を使うのは、`invoke_kamii_adapter`が内部で`std::thread`をspawnし`recv_timeout`で最大250ms同期的に待つため、tokio worker threadを塞がないようにするため（既存の`cas.get`呼び出しと同じ理由）。
- 新規test「kamiiがdenyを返せば配信を拒否する」「kamiiがtimeoutしても配信を許可しない」の両方で403を確認した。
- 既存5 testは既定adapter（常に`Allow`）のままなので挙動は変わらない。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`（`fold-gateway`のegress側も配線済み。ingest側は`fold-peer`で既完了）
- integration: `fold-gateway`のCLI（`cargo run -p fold-gateway`）経路はこの関数を経由する
- automated verification: `cargo test -p fold-gateway` 7件成功（既存5＋新規2）。`cargo test --workspace --locked`全成功。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`通過。Python 5件、validator PASS
- packaging: 該当なし
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- ingest（`fold-peer`）とegress（`fold-gateway`）の両方にKamii gateを置いたことで、「どちらの経路でCASへ入ったobjectでも、配信前にfail-closed gateを通る」という対称性ができた。`seed-public-fixture`のようにP2Pを経由せずCASへ直接投入されたobjectも、egress側のgateで一律に扱える。
- `GatewayState`という小さな構造体を導入したことで、axum `State`の型がtupleより読みやすくなった。これは既存API（`router(cas)`）を壊さない範囲の内部実装変更。

## `[HYPOTHESIS]` 次の仮説

- 実inspection判定ロジックが決まれば、`fold-peer`と`fold-gateway`の両方で`default_kamii_adapter`を同じ実装へ差し替えるだけで済む。
- 将来Kamiiを実processへ分離する際も、`KamiiAdapterFn`という関数ポインタ境界をIPC呼び出しのラッパーへ置き換えるだけで済む可能性がある。

## `[UNKNOWN]` 未解決・未試験

- 実inspection判定ロジックそのもの
- 実process分離
- `fold-peer`と`fold-gateway`でadapter実装を共有する将来設計（現状は同じcode片を別々に持つ重複がある）

## command・検証

```console
cargo test -p fold-gateway
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests -v
python3 -B scripts/foldnic_dev.py validate
git diff --check
```

## rollback／unmount

- `router_with_kamii_adapter`と`GatewayState`を削除し、`router`を元の`with_state(cas)`へ戻せば元の挙動に戻る。

## 引継ぎ

- 次の作業: `fold-peer`と`fold-gateway`で重複しているKamii配線helper（`default_kamii_adapter`、定数、型）を共通crateへ集約するかどうかの検討。実inspection判定ロジックの設計
- User Gate: なし
- 関連Issue／commit: ZeroRoomLab-manifest #28、fold-nic #1、commitは作成後追記
