# authority型とInsecureDevFixtureResolver（Lane B / #5、1件目）

作成日時: 2026-08-28 11:30:00 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: #5が求める「publisher署名の数学的検証とWorld authority判断の分離」の最初のcheckpointとして、`AuthorityStatus`／`TrustBasis`／`PublisherKeyRef`／`AuthorityDecision`型と`PublisherAuthorityResolver` trait、`InsecureDevFixtureResolver`（DEVHAGE bootstrap resolver）を実装する
- 非目標: `LocalSelfAssertedResolver`、local鍵生成CLI、provenance署名検証との接続（peer receiptへのsignature/authority分離記録）は次checkpointへ分離する。公開Registry／multi-witness、local self assertionのpublic authority昇格条件はUser Gateのまま未着手

## source・revision

- branch: `dev/stage0-transport-cache`
- Git revision: `b2c408c`（起点）
- worktree: `crates/fold-authority/`新設、`Cargo.toml`（workspace members）
- upstream／contract: fold-nic #1 issuecomment-5447652557（Lane B）、#5

## 既存tool確認

- 探索したtool: `fold-provenance::SIGNATURE_SCHEME_ED25519_V1`（既存署名scheme定数）、`fold-provenance`の既存fixed test seed`[7u8; 32]`
- reuse／extend／adapt／create: 新規crate`fold-authority`を作成。既存crateに「署名検証結果からauthorityを判断する」責務を持つものがなく、`fold-provenance`（exact bytes署名検証専任）とは責務が異なるため分離した。署名scheme定数は`fold-provenance`から再利用し、重複定義しない
- 判断理由: #5のarchitecture図（`exact bytes署名検証 -> signature_valid -> publisher key_id -> PublisherAuthorityResolver`）が既に責務分離を示しており、それに沿ったcrate境界にした

## 入出力・副作用・権限

- 入力: `PublisherKeyRef`（scheme／公開鍵hex）、World ID／Worldline ID文字列
- 出力: `AuthorityDecision`（status／trust_basis／production_readiness／machine token`reference`）
- filesystem副作用: なし
- network: なし
- authority: `InsecureDevFixtureResolver`はDEVHAGE鍵指紋（`DEVHAGE_PUBLIC_KEY_HEX`）だけを扱い、それ以外の鍵は`UNKNOWN`（この resolverの範囲外）を返す。production profileではDEVHAGE鍵を無条件で`UNTRUSTED`にする
- secret境界: `DEVHAGE_PRIVATE_KEY_SEED_HEX`は既存test seedと同一の`[PUBLIC-TEST-FIXTURE]`であり、README／AGENTS.mdの「公開済みTEST fixtureだけはprivate keyをrepositoryへ入れてよい」規約に従い明示コメント付きで公開している

## `[FACT]` 事実・観測

- `DEVHAGE_PUBLIC_KEY_HEX`は、`fold-provenance`のtestで使われている`SigningKey::from_bytes(&[7u8; 32])`から導出した公開鍵と一致する。これを一時的な外部crateで計算し（`/tmp`の使い捨てcrate、repositoryへは含めない）、定数として固定した。新規test「devhage公開鍵はseedから導出した値と一致する」で、`DEVHAGE_PRIVATE_KEY_SEED_HEX`から実際に`ed25519-dalek`で再導出した値と一致することを確認し、copy-paste driftを防いだ。
- `InsecureDevFixtureResolver::resolve`は次の順で判定する: (1) DEVHAGE鍵か確認（違えば`UNKNOWN`／`NOT_DEVHAGE_KEY`）、(2) `RuntimeProfile::Production`なら無条件`UNTRUSTED`／`HAGE_PRODUCTION_USING_DEV_KEY`、(3) `NetworkScope::LoopbackOnly` かつ `explicit_acknowledgement == true` の両方が揃わなければ`UNTRUSTED`／`DEVHAGE_USAGE_CONDITIONS_NOT_MET`、(4) 全て満たせば`TRUSTED_FOR_SCOPE`／`HAGE_BOOTSTRAP_KEY_ACTIVE`。
- `reference` fieldには#5で提案されたwarning token（`HAGE_BOOTSTRAP_KEY_ACTIVE`、`HAGE_PRODUCTION_USING_DEV_KEY`）をそのまま採用し、GUI表示文はこの crateで生成しない（machine tokenと表示文の分離）。

## `[RESULT]` 結果

- implementation: `IMPLEMENTED_ALPHA`（型とDEVHAGE resolverのみ、provenance／peer receiptとの接続は未実施）
- integration: 未実施（`fold-peer`／`fold-provenance`からはまだ呼ばれない）
- automated verification: `cargo test -p fold-authority` 7件成功。`cargo test --workspace --locked`全成功（fold-authority追加でtotal増）。`cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`通過。Python 5件、validator PASS
- packaging: 該当なし
- distribution: checkpoint push前
- human review: `PENDING`

## `[INTERPRETATION]` 考察

- `AuthorityStatus`／`TrustBasis`／`ProductionReadiness`を`#[non_exhaustive]`にしたことで、次checkpointで`LOCAL_SELF_ASSERTED`／`REGISTRY_ATTESTED`等を使うresolverを追加しても、この列挙型自体の破壊的変更にはならない。
- `RuntimeProfile`／`NetworkScope`は`Serialize`を付けていない。呼び出し側の実行時申告であり、receiptへ保存する対象は`AuthorityDecision`の方だと判断したため。

## `[HYPOTHESIS]` 次の仮説

- `LocalSelfAssertedResolver`は、local secret pathからpublic keyを読み、`TrustBasis::LocalSelfAsserted`／`AuthorityStatus::TrustedForScope`を返す形で同じtrait実装として追加できる見込み。
- `fold-provenance::verify_manifest_signature`の結果（`ManifestSignatureVerification`）と`AuthorityDecision`を、`fold-peer`のprovenance receiptへ別fieldとして積む形（Lane C）で接続できる。

## `[UNKNOWN]` 未解決・未試験

- `LocalSelfAssertedResolver`、local鍵生成CLI（次checkpoint）
- provenance署名検証・peer receiptとの縦断接続（Lane C）
- 公開Registry／multi-witnessの正本と運営主体（User Gate）
- local self assertionをpublic authorityへ昇格する条件（User Gate）
- TIBIHAGE／RSA互換profile（#5の範囲だが未着手）

## command・検証

```console
cargo test -p fold-authority
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests
python3 -B scripts/foldnic_dev.py validate
git diff --check
```

## rollback／unmount

- `crates/fold-authority`はどこからも呼び出されていない独立crateであり、workspace membersからの削除とcrate directory削除だけで復元不要に切り離せる。

## 引継ぎ

- 次の作業: `LocalSelfAssertedResolver`とlocal鍵生成、または他のGreen laneへ継続
- User Gate: 公開Registry／multi-witnessの正本と運営主体、local self assertionのpublic authority昇格条件（いずれも今回未着手、#5記載どおり）
- 関連Issue／commit: fold-nic #1、#5、commitは作成後追記
