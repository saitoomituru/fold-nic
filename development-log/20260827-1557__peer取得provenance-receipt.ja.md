# peer取得provenance receipt

作成日時: 2026-08-27 15:57:49 +0900
作成者／agent: Codex
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: P2P取得成功をstdoutだけで失わず、観測事実と未検証claimを分けたlocal JSON receiptへ残す
- 非目標: receipt署名、publisher署名、World authority証明、clock較正を実装しない

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `bb7d003`
- worktree: provenance receipt実装中
- upstream／contract: Stage 0 bare-metal runtime契約、`FoldReceipt`のunknown保持原則

## 既存tool確認

- 探索したtool: `FoldReceipt`、`serde_json`、Rust `OpenOptions::create_new`
- reuse／extend／adapt／create: 既存receipt原則をP2P fetch固有の局所Schemaへadapt
- 判断理由: 汎用`FoldReceipt`へ未確定fieldを拙速に固定せず、実取得面の必要最小fieldを先に検証するため

## 入出力・副作用・権限

- 入力: `fetch --receipt-file <明示path>`
- 出力: `fold-peer-fetch-receipt/0` JSON
- filesystem副作用: 指定pathへ新規fileを一度だけ作成し、既存fileは上書きしない
- network: receipt機能自体は追加networkなし
- authority: source peerは`UNVERIFIED_TRANSPORT_PEER`と明記
- secret境界: object本体、private key、token、Cookie、Authorizationを保存しない

## 変更

- file: `crates/fold-peer/src/main.rs`、README、capability matrix
- contract／Schema差分: `fold-peer-fetch-receipt/0`

## `[FACT]` 事実・観測

- receiptは要求CID、再検証CID、size、World／Worldline、一時PeerId、transport profile、host clock観測を保持する。
- publisher Identity、World authority、object署名は未検証として明示する。
- `create_new`により既存receipt fileを上書きしない。
- x86_64 macOS 15.7.7の二process試験で5,651 byteを取得し、`fold-peer-fetch-receipt/0` fileを作成できた。
- 同一receipt pathを指定した二回目の取得は`AlreadyExists`で終了し、既存fileを上書きしなかった。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`
- integration: `fold-peer fetch`のoptional file outputを実二processで確認
- automated verification: content非包含と既存file上書き拒否のunit testおよび実process試験成功
- packaging: JSON local receipt
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- transport成功とauthority証明を同じ「取得成功」へ畳まず、後続の署名済みmanifest実装へ検証境界を渡せる。

## `[HYPOTHESIS]` 次の仮説

- 署名済みmanifestをobjectとは別に検証し、その結果をreceiptへ追加すれば、peer cacheとWorld名義のclaimを分離したまま接続できる。

## `[UNKNOWN]` 未解決・未試験

- receipt自身の署名、chain、rotation、集約
- clock較正と時刻巻き戻り
- receipt pathのsymlink parentに対する専用root policy
- 同一取得の冪等receipt ID設計

## command・検証

```console
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p fold-peer
# 7 passed; 0 failed

cargo run -q -p fold-peer -- fetch \
  --cas-root /private/tmp/fold-peer-stage0-client-receipt-20260827 \
  --dial /ip4/127.0.0.1/tcp/59323/p2p/<ephemeral-peer-id> \
  --cid bafkreid7dapgrmlkfwqumjutrrmdthqts7jybngjzbfbg7syd7pm63c5pq \
  --world fold-nic-forge \
  --worldline stage0 \
  --receipt-file /private/tmp/fold-peer-fetch-receipt-20260827-1557.json
# VERIFIED_AND_STORED, size=5651
# 同一command二回目: File exists (os error 17)
```

## rollback／unmount

- `--receipt-file`を指定しなければfile副作用は発生しない。作成済みreceiptは利用者が明示管理する。

## 引継ぎ

- 次の作業: 実二process取得でreceipt fileを採取し、JSON fieldと上書き拒否を確認
- User Gate: 署名authorityの正本を固定する前に設計確認
- 関連Issue／commit: fold-nic #1、commitは作成後追記
