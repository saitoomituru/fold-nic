# provenance hex入力をUTF-8境界panicなしで検証する（Lane A / #3）

作成日時: 2026-08-28 10:10:00 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: #3で指摘された`fold-provenance::decode_hex`のUTF-8境界panicを修正し、`DetachedManifestSignature.schema`未検証も併せて閉じる
- 非目標: 署名対象exact bytes、key schema、publisher authorityの契約変更は行わない（#3停止条件どおり局所parser修正に限定）

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `939e3e5`（起点、docs/img追加のみのuser commit）
- worktree: `crates/fold-provenance/src/lib.rs`、`crates/fold-core/src/error.rs`
- upstream／contract: fold-nic #1 issuecomment-5447652557の指示、fold-nic #3

## 既存tool確認

- 探索したtool: `crates/fold-provenance/src/lib.rs`の`decode_hex`（既存不具合箇所そのもの）、`fold-core::FoldErrorCode`（既存の安定code enum）
- reuse／extend／adapt／create: 新規crateは作らず、既存関数を修正。`FoldErrorCode`へ`UnsupportedManifestSchema`を1件追加（`#[non_exhaustive]`のため破壊的変更ではない）
- 判断理由: 同じ責務を持つ既存箇所の局所修正であり、新しいparser／crateを起こす理由がない

## 入出力・副作用・権限

- 入力: `public_key_hex`／`signature_hex`（機械境界のhex field）、`DetachedManifestSignature.schema`
- 出力: 既存の`FoldError`。新規code`UNSUPPORTED_MANIFEST_SCHEMA`を追加
- filesystem副作用: なし
- network: なし
- authority: 変更なし
- secret境界: 変更なし（この関数は公開鍵／署名という公開値のみを扱う）

## `[FACT]` 事実・観測

- 旧`decode_hex`は`value: &str`を`&value[index..index+2]`でbyte index sliceしていた。`value.len()`はbyte長だが、多byte UTF-8文字の途中byteをsliceの境界に取ると`panic: byte index X is not a char boundary`に到達する（例: `"あ0"`は4byte、偶数長判定を通過するが`[0..2]`が「あ」の3byte表現を分断する）。
- 修正後は`value.as_bytes()`から`chunks_exact(2)`で走査し、string sliceを一切使わない。非ASCII byteは`hex_nibble`が単に`Err`を返すため、非ASCII混じりの入力でもpanicせずerrorになる。
- 新規test「非ascii混じりのhex入力はpanicせずerrorになる」で、3byte文字混在、全角英数、絵文字、空文字列、奇数長、非hex ASCII、長い非hex文字列の8caseを一括で検査し、全て`InvalidPublicKey`／`InvalidSignatureEncoding`へ落ちることを確認した。
- `verify_manifest_signature`は`signature.scheme`（署名方式）のみ検査し、`signature.schema`（署名メッセージformat版）を検証していなかった。修正で`schema != MANIFEST_SIGNATURE_SCHEMA`を`UnsupportedManifestSchema`として拒否するようにした。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`（局所修正、完了）
- integration: `fold-peer`／`fold-gateway`はこの関数を直接呼んでいないため影響なし（provenance署名検証自体は#5のLane Cで縦断配線予定）
- automated verification: `cargo test -p fold-provenance -p fold-core` 9＋8件成功。`cargo test --workspace --locked`全成功。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`通過。Python 5件、validator PASS
- packaging: 該当なし
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- byte単位の`chunks_exact`走査へ切り替えたことで、`&str`の文字境界を一切意識しない実装になった。これはASCII hex fieldという機械境界の性質上、人間向け入力全体をASCII限定する話とは別軸である（#3の指摘どおり）。
- `schema`未検証は独立した見落としだったが、同じ関数のsecurity-relevant入力検証という点で同一checkpointに含めた。

## `[HYPOTHESIS]` 次の仮説

- 同種のbyte index sliceパターンが他crateにないか、`rg`で確認する価値がある（今回は未実施、次回のLane Gで棚卸し候補）。

## `[UNKNOWN]` 未解決・未試験

- 大規模fuzzing（cargo-fuzz等）は未導入。今回のtestは手動で選んだ負例集であり、property-based testing相当の網羅性はない。

## command・検証

```console
cargo test -p fold-provenance -p fold-core
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests
python3 -B scripts/foldnic_dev.py validate
git diff --check
```

## rollback／unmount

- `decode_hex`／`hex_nibble`を旧実装へ戻し、`FoldErrorCode::UnsupportedManifestSchema`と関連checkを削除すれば元に戻せる。

## 引継ぎ

- 次の作業: Lane D（Kamii raw outcome分離、CID検証順）、Lane F（error catalog）など、他のGreen laneへ継続
- User Gate: なし
- 関連Issue／commit: fold-nic #1、#3、commitは作成後追記
