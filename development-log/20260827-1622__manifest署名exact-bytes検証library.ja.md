# manifest署名 exact bytes検証library

作成日時: 2026-08-27 16:22:46 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: 署名済みmanifestの3択（exact bytes署名／canonical encoding署名／Registry契約先行）のうち、後方互換性を壊さない最小面として「exact bytes + CID + detached ed25519署名」の検証libraryだけを先に作る
- 非目標: publisher公開鍵をどのWorld authorityへbindingするかのRegistry契約は実装しない。canonical encoding方式は採用しない。CLI／`fold-peer fetch`への配線は次checkpointへ分離する

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `ef81791`（起点）
- worktree: `crates/fold-provenance`新設
- upstream／contract: Issue #28（ZeroRoomLab-manifest）、fold-nic #1の「peer取得provenance receipt」checkpoint、AGENTS.mdのUser Gate運用

## 既存tool確認

- 探索したtool: `fold_store::cid_for`（既存CID再計算関数）、`FoldError`／`FoldErrorCode`（既存共有error型）
- reuse／extend／adapt／create: CID再計算は`fold-store`を再利用。error codeは`FoldErrorCode`へ3種追加。署名検証本体は新規crate`fold-provenance`として作成（既存crateに同種機能なし）
- 判断理由: `fold-core`はI/Oを行わないprotocol型専用、`fold-store`はCAS専用と責務分離されているため、署名検証は独立crateとして追加する方が既存境界と整合する

## 入出力・副作用・権限

- 入力: 受信bytesそのもの（canonicalize不可）、`DetachedManifestSignature`（scheme／公開鍵hex／署名hex）
- 出力: `ManifestSignatureVerification`（schema／再計算CID／signature_valid／publisher_authority固定値）
- filesystem副作用: なし（純粋関数）
- network: なし
- authority: `publisher_authority`は常に`PUBLISHER_AUTHORITY_UNVERIFIED`を返し、この crateではbindingを判断しない
- secret境界: private keyやseedは扱わない。公開鍵・署名はいずれも公開情報

## 変更

- file: `Cargo.toml`（workspace members／`ed25519-dalek`依存追加）、`crates/fold-provenance/`新設（`Cargo.toml`、`src/lib.rs`）、`crates/fold-core/src/error.rs`（`UnsupportedSignatureScheme`／`InvalidPublicKey`／`InvalidSignatureEncoding`追加）
- contract／Schema差分: `fold-manifest-signature/0`

## `[FACT]` 事実・観測

- `verify_manifest_signature`は受信bytesを変更せずed25519 detached署名をそのまま検証する。
- 署名対象と`fold_store::cid_for`が返すCIDはどちらも受信bytesから直接計算され、canonical encoding層を経由しない。
- 署名不一致（改変bytes、別鍵）はErrorではなく`signature_valid: false`として返る。scheme不一致・鍵長不正・署名長不正はErrorとして返る。
- `publisher_authority`はcode上の固定文字列であり、呼び出し側が別途authority検証を行わない限りこの値のまま運ばれる。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`（検証libraryのみ、CLI未配線）
- integration: 未実施（`fold-peer`等からの呼び出しは次checkpoint）
- automated verification: `cargo test -p fold-provenance` 6件成功、workspace全体45件成功
- packaging: 該当なし
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- 「exact bytesへ署名する」を先に固定したことで、canonical encoding方式へ後で切り替える場合も、この検証library自体は破壊されず、上位でbytesの生成方法を変えるだけで済む設計になっている。
- Registry契約（publisher鍵→World authority）を意図的に外に出したことで、この crateは「署名が数学的に正しいか」だけを閉じた責務として持てている。

## `[HYPOTHESIS]` 次の仮説

- `FoldObjectManifest`をserde_jsonでexact bytes化し、そのbytesに対して本crateで署名・検証するinsatnce testを追加すれば、実際のmanifest配布経路に接続できる。
- `fold-peer fetch --receipt-file`のprovenance receiptへ`ManifestSignatureVerification`を追加fieldとして混ぜれば、`OBJECT_SIGNATURE_NOT_PROVIDED`だった既存UNKNOWNを段階的に埋められる。

## `[UNKNOWN]` 未解決・未試験

- publisher鍵とWorld authorityのbinding（Registry契約）は未設計のまま
- 鍵rotation、鍵失効、複数署名者、しきい値署名は未検討
- `fold-peer`／CLI／receiptへの実配線と統合testは未実施
- 大きいmanifestに対する署名検証の性能特性は未計測

## command・検証

```console
cargo build --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
# fold-provenance: 6 passed; workspace合計45 passed; 0 failed
git diff --check
python3 -B scripts/foldnic_dev.py validate
python3 -B scripts/foldnic_dev.py doctor
```

## rollback／unmount

- `crates/fold-provenance`はどこからも呼び出されていない独立crateであり、workspace membersからの削除とCargo.tomlの依存行削除だけで復元不要に切り離せる。

## 引継ぎ

- 次の作業: `FoldObjectManifest`の実bytes化経路との接続、`fold-peer`への配線、Registry契約（publisher鍵→World authority binding）の設計着手
- User Gate: Registry契約の設計に入る前に、bindingをどの層（Fold NIC本体／ZeroRoomLab-manifest側）が正本として持つかを確認
- 関連Issue／commit: ZeroRoomLab-manifest #28、fold-nic #1、commitは作成後追記
