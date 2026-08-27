# loopback二process P2P交換

作成日時: 2026-08-27 15:48:33 +0900
作成者／agent: Codex
状態: `[DRAFT]` `[IMPLEMENTED-ALPHA]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: 同一macOS上の二process間で、一個のCAS objectをCIDとWorld文脈付きで交換できる最小垂直断面を作る。
- 非目標: peer自動発見、Internet公開、匿名routing、恒久Identity、複数peer複製、取得contentの実行。

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: 作業開始時`51eb078`
- worktree: P2P実装中
- upstream／contract: ZeroRoomLab-manifest Issue #28 comment `5434974574`、本repository Stage 0 runtime契約

## 既存tool確認

- 探索したtool: Rust `libp2p 0.56.0`、request-response CBOR codec、Noise、Yamux、既存`fold-store`
- reuse／extend／adapt／create: libp2pの既存transportをreuseし、Fold object要求／応答と受信後CAS検証を作成
- 判断理由: 独自暗号transportを作らず、object意味境界と失敗時挙動の検証へ集中するため

## 入出力・副作用・権限

- 入力: 明示したloopback multiaddr、CID、World ID、Worldline ID、共有許可済みfixture
- 出力: server／fetch JSON receipt、client側CAS object
- filesystem副作用: 指定した専用CAS root内だけへobjectを作成
- network: loopback TCPだけ。非loopback multiaddrは起動前に拒否
- authority: fixture投入時に`--acknowledge-loopback-share`を明示
- secret境界: transport PeerIdの鍵はprocess内で一時生成し、diskへ保存しない。Fold Identityではない

## 変更

- file: `crates/fold-peer/`、workspace dependency、README、Stage 0契約、capability matrix、validator
- contract／Schema差分: `/fold-nic/object-exchange/0.1.0`と`fold-peer-receipt/0`をalpha局所契約として追加

## `[FACT]` 事実・観測

- request-response CBOR codecへrequest 4 KiB、response `max_object_bytes + 64 KiB`の上限を設定した。
- listen／dialの両方をloopback IPとTCPへ限定した。
- clientは応答のWorld／Worldline／CID主張を照合し、受信byte列をlocal CASへ投入して再計算CIDを要求CIDと比較する。
- Rust workspaceは計39 unit testを通過した。
- server側CASへ投入した5,651 byteのREADME objectを、別processのclient側CASへ取得できた。seedとfetchのCIDは`bafkreid7dapgrmlkfwqumjutrrmdthqts7jybngjzbfbg7syd7pm63c5pq`で一致した。
- 通常sandboxではTCP listenが許可されず起動失敗した。loopback listenだけを許可した実機権限で再実行すると成功した。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`
- integration: x86_64 macOS 15.7.7上の`TWO_LOCAL_PROCESSES`で一個のREADME object交換に成功
- automated verification: fmt、clippy `-D warnings`、workspace test成功
- packaging: Cargo binary source
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- Onion風の恒久名称・Identity層へ進む前に、transport上のPeerIdとFold Identityを分離したままCID object交換を試せる。
- World文脈は現段階では相手がechoするrouting contextであり、署名による権限担保ではない。

## `[HYPOTHESIS]` 次の仮説

- この交換面へprovenance receiptと署名済みmanifestを追加すれば、単なるbyte共有からWorld内claimの検証へ進める。

## `[UNKNOWN]` 未解決・未試験

- 悪性peer、切断、partial response、同時多数request、長時間運転、disk quota。
- peer discovery、NAT traversal、匿名性、traffic analysis耐性。
- request処理中のCAS readは同期I/Oであり、負荷時のevent loop影響は未計測。
- World文脈とpublisher署名の暗号的bindingは未実装。

## command・検証

```console
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# 39 passed; 0 failed

cargo run -q -p fold-peer -- seed-public-fixture \
  --cas-root /private/tmp/fold-peer-stage0-server-20260827 \
  --file README.md \
  --acknowledge-loopback-share
# status=ok, size=5651

cargo run -q -p fold-peer -- serve \
  --cas-root /private/tmp/fold-peer-stage0-server-20260827
# status=ready, /ip4/127.0.0.1/tcp/58155/p2p/<ephemeral-peer-id>

cargo run -q -p fold-peer -- fetch \
  --cas-root /private/tmp/fold-peer-stage0-client-20260827 \
  --dial /ip4/127.0.0.1/tcp/58155/p2p/<ephemeral-peer-id> \
  --cid bafkreid7dapgrmlkfwqumjutrrmdthqts7jybngjzbfbg7syd7pm63c5pq \
  --world fold-nic-forge \
  --worldline stage0
# status=ok, size=5651, CID一致
```

## rollback／unmount

- `fold-peer` processを終了し、試験用に明示したCAS rootを不要なら別途削除する。恒久鍵やsystem設定は作らない。

## 引継ぎ

- 次の作業: 実二process交換、receipt採取、Actions確認、Issue checkpoint
- User Gate: 非loopback／公開network試験へ進む前に別途設計境界を確認する
- 関連Issue／commit: `saitoomituru/fold-nic#1`、checkpoint commitは作成後追記
