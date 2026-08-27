# localhost read-only Gateway

作成日時: 2026-08-27T15:27:57+09:00
作成者／agent: Codex
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: host CAやorigin fetchより先に、loopback限定でhealthと検証済みCAS objectを返す実行可能Gatewayを作る
- 非目標: write ingest、origin fetch、P2P、TLS、browser proxy、active content表示を実装しない

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `21d6b59`
- worktree: HTTP policy checkpoint後clean
- upstream／contract: fold-nic #1、Stage 0ベアメタルruntime契約

## 既存tool確認

- 探索したtool: repository内server実装、Axum／Tokio／Clapの現行crate
- reuse／extend／adapt／create: HTTP parser／routing／runtimeは既存crateを利用し、Fold固有routeとCAS境界だけ新規作成
- 判断理由: untrusted HTTP parsingを独自実装せず、macOS native userspaceで小さく起動するため

## 入出力・副作用・権限

- 入力: loopback GET `/healthz`、GET `/v0/objects/{cid}`
- 出力: JSON health／error、`application/octet-stream` object
- filesystem副作用: 起動時CAS root作成、GET時はreadのみ
- network: loopback listenerだけ。origin／peer接続なし
- authority: CID一致を再検証して返すだけでpublisher／安全性を保証しない
- secret境界: Authorization、Cookie、private keyを入力・保存しない。CAS root pathはlocal startup receiptだけに出る

## 変更

- file: `crates/fold-gateway/`
- file: `crates/fold-store/src/lib.rs`
- contract／Schema差分: loopback bind拒否、read-only route、healthの未実装表示、`nosniff`とattachment配布

## `[FACT]` 事実・観測

- default bindは`127.0.0.1:7743`
- `0.0.0.0`等のnon-loopback bindを拒否する
- objectは返す直前にCASでCID再検証される
- responseは`application/octet-stream`、`nosniff`、attachmentとし、active contentをinline実行させない
- write endpoint、origin fetch、P2Pはhealthの`explicit_non_capabilities`へ列挙する
- sandbox内ではloopback bindが`Operation not permitted`になり、実機権限で同じcommandを再実行した
- 実機起動は`127.0.0.1:17843`でreadyとなり、`/healthz`はHTTP 200、invalid CIDは機械可読error付きHTTP 400を返した

## `[RESULT]` 結果

- implementation: read-only local Gateway alpha
- integration: local CAS readと統合、HTTP policy／origin未統合
- automated verification: router 5件、実process起動、health curl、invalid CID curlを実行済み
- packaging: Cargo binary
- distribution: 開発branch checkpoint予定
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- Local CAなしでもlocalhostでprocess lifecycle、routing、CAS read、error境界を先に検証できる
- write経路をまだ公開しないことでHTTP guard統合前の無条件cache投入を避けられる

## `[HYPOTHESIS]` 次の仮説

- origin fetch adapterがHTTP policyを通したbodyだけをCASへputすれば、read-only routeを保ったままcache miss取得を追加できる

## `[UNKNOWN]` 未解決・未試験

- origin SSRF／redirect／DNS rebinding対策
- graceful shutdown以外のprocess supervision
- concurrent large readとbackpressure
- host CA／`.fold` ingress
- local user間のloopback access boundary
- private CAS partition

## command・検証

```console
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo run -p fold-gateway -- --bind 127.0.0.1:17843 --cas-root <temporary-root>
curl http://127.0.0.1:17843/healthz
curl http://127.0.0.1:17843/v0/objects/not-a-cid
python3 -B -m unittest discover -s tests -v
python3 -B scripts/foldnic_dev.py validate --json
git diff --check
```

## rollback／unmount

- Ctrl-CでGatewayを停止する
- `fold-gateway`追加commitをrevertする。CAS／policyは独立して残る
- temporary CAS rootだけを試験終了後に削除する。repository CASを自動削除しない

## 引継ぎ

- 次の作業: constrained origin fetchとHTTP policy→CAS統合、または二process P2P read交換
- User Gate: なし。loopback境界またはCAS再検証を維持できなければ停止
- 関連Issue／commit: fold-nic #1、commitは作成後追記予定
