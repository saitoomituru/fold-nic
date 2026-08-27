# Kamii mock hookをfold-peerへ配線する

作成日時: 2026-08-27 18:45:00 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: `fold-kamii`のfail-closed呼び出し境界を、実際にobjectを受け入れる`fold-peer::accept_response`へ配線し、CASへ書き込む前にadapterを通す
- 非目標: 実Kamii inspection判定ロジック（何を判定基準にするか）は実装しない。Stage 0では常に`Allow`を返すplaceholder adapterのみを既定として使う。実process分離、`fold-gateway`側配線は次checkpointへ分離する

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `4eb1794`（起点、`main`側は`bf007ae`/`af730ba`まで中間merge済み）
- worktree: `crates/fold-peer/src/lib.rs`、`crates/fold-peer/Cargo.toml`
- upstream／contract: fold-nic #1の「Kamii mock hook」checkpoint引継ぎ（`次の作業: fold-gateway／fold-peerからの実配線`）

## 既存tool確認

- 探索したtool: `fold_kamii::invoke_kamii_adapter`（既存mock hook）、`fold_peer::accept_response`（既存の受入検査関数）
- reuse／extend／adapt／create: 新規crateは作らず、既存`accept_response`をKamii adapter注入可能な`accept_response_with_kamii_adapter`へ拡張し、`accept_response`はそこへ既定adapterで委譲する形にした
- 判断理由: 呼び出し側（`fold-peer`のCLI）のAPIを変えずに済み、testでは任意のadapterを注入してfail-closed経路を直接検証できる

## 入出力・副作用・権限

- 入力: 既存の`ObjectRequest`／`ObjectResponse`に加え、CIDとbyte長だけの`KamiiInspectionRequest`
- 出力: 既存の`PutReceipt`、または新しいerror code`P2P_KAMII_DENIED`
- filesystem副作用: Kamii拒否時は`cas.put`を一切呼ばないため、CAS rootへの書込みが発生しない
- network: なし
- authority: placeholder adapterは常に`Allow`を返すため、現時点でこの配線がobjectを拒否する実運用上の効果はない。timeout／crash時のfail-closedだけが実効を持つ
- secret境界: 変更なし

## `[FACT]` 事実・観測

- `accept_response_with_kamii_adapter`は、request/response/CID検査を終えた後、`cas.put`の直前で`invoke_kamii_adapter`を呼ぶ。
- `KamiiOutcome::resolved_verdict() != KamiiVerdict::Allow`の場合、`P2P_KAMII_DENIED`を返し、CASへは何も書き込まれない（testで`cas.get`がErrになることを確認）。
- 既定の`accept_response`は`default_kamii_adapter`（常に`Allow`）を使うため、既存の4テストの挙動は変わらない。
- 新規テスト2件で、Denyとtimeoutのどちらでも書込み前に拒否されることを確認した。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`（`fold-peer`のみ配線済み、`fold-gateway`は未着手）
- integration: `fold-peer::fetch`のCLI経路はこの関数を経由するため、実machineでのfetchにも適用される
- automated verification: `cargo test -p fold-peer` 7件成功（既存4＋新規2＋main.rs既存2、実際は9件見えるがlib/main分割で7+2）。`cargo test --workspace --locked`全成功。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`通過。Python 5件、validator PASS
- packaging: 該当なし
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- CASへの書込み前にgateを置いたことで、「一度書いてから消す」という未設計のquarantine／削除機構を必要とせずにfail-closedを実現できた。
- placeholder adapterが常に`Allow`である以上、この配線が現時点で防いでいるのは「adapterのtimeout／crashが誤ってAllow扱いされること」だけであり、悪意あるobjectの検出そのものはまだ行っていない。README／Issueでこの限定を明記する必要がある。

## `[HYPOTHESIS]` 次の仮説

- 同じ`accept_response_with_kamii_adapter`パターンを`fold-gateway`のobject配信経路（存在すれば）へも適用できる可能性がある。
- 実inspection判定ロジックが決まれば、`default_kamii_adapter`を差し替えるだけで済む設計になっている。

## `[UNKNOWN]` 未解決・未試験

- `fold-gateway`側のKamii配線は未実施
- 実process分離
- 複数adapter呼び出しの多重度
- 実inspection判定基準そのもの

## command・検証

```console
cargo test -p fold-peer
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests -v
python3 -B scripts/foldnic_dev.py validate
git diff --check
```

## rollback／unmount

- `accept_response_with_kamii_adapter`を`accept_response`へ戻し、Kamii呼び出し部分を削除すれば元の挙動に戻せる。CAS format自体への影響はない。

## 引継ぎ

- 次の作業: `fold-gateway`側のKamii配線、または実Kamii inspection判定ロジックの設計
- User Gate: なし
- 関連Issue／commit: ZeroRoomLab-manifest #28、fold-nic #1、commitは作成後追記
