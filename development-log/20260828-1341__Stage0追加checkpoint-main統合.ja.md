# Stage0追加checkpoint-main統合

作成日時: 2026-08-28T13:41+09:00
作成者／agent: Codex
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: `dev/stage0-transport-cache`へ散在していた追加checkpointを、検証済みsnapshotとして`main`へ回収する。
- 非目標: Stage 0完成、MAGI監査指摘の解決、production readiness、最終branch削除を主張しない。

## source・revision

- source branch: `dev/stage0-transport-cache`
- source revision: `569a2fc5f8eb63cd4749968981a1499b6d441119`
- target branch: `main`
- target revision before merge: `af730ba9d469ff52f4788993b7ed5c166037d2f8`
- Git revision: `9fa9e10973a3edc3931bef3b280618fd607fac44`
- worktree: `clean`
- upstream／contract: `docs/development/checkpoint-and-branch-policy.ja.md`、fold-nic Issue #1／#3／#4／#5／#6。

## 既存tool確認

- 探索したtool: `git merge-tree --write-tree`、既存のStage 0中間merge receipt、repository validator、GitHub Actions。
- reuse／extend／adapt／create: 既存branch運用をreuseし、`--no-ff` mergeと本receiptを追加した。
- 判断理由: commitの由来と継続branchを保持し、単なるsquashで実験／debug履歴を消さないため。

## 入出力・副作用・権限

- 入力: `main@af730ba`、`dev/stage0-transport-cache@569a2fc`。
- 出力: merge commit `9fa9e10973a3edc3931bef3b280618fd607fac44`と本receipt。
- filesystem副作用: 隔離clone内のGit index／working treeとbuild artifactだけを更新した。
- network: source checkpoint push、GitHub Actions確認、後続の`main` pushを行う。
- authority: fold-nic repository内の明示branchだけを統合し、Atlantis／Manifest／EDOHAGE-TUBOを変更していない。
- secret境界: 実鍵を探索、読出し、exportしていない。公開DEVHAGE fixtureは既存履歴のまま。

## 変更

- file: Kamii peer／gateway配線、raw outcome、bounded concurrency、provenance parser修正、`fold-authority`、Cockpit画像、開発ログ、鍵export note／AGENTS概要を統合した。
- contract／Schema差分: merge自体は新しいruntime Schemaを作らない。AGENTSでagent秘密操作、製品鍵Capability、公開fixtureを分離した。

## `[FACT]` 事実・観測

- `git merge-tree --write-tree main dev/stage0-transport-cache`はtree `a8b427a0329f75bada45344238d7d5310a669eb5`を生成し、衝突を報告しなかった。
- source revision `569a2fc`のActions run `33142453010`はPython 3.11／3.14とRust Stage 0が全て成功した。
- remote branchはFold NICが2本だけで、`dev/stage0-transport-cache`以外の未回収作業branchは観測しなかった。

## `[RESULT]` 結果

- implementation: 既存追加checkpointを変更せず`main`へ到達可能にした。
- integration: `main`へ`--no-ff`統合。Stage 0最終統合ではない。
- automated verification: source branchはRust 72件、Python 5件、fmt、clippy、validator、doctor、Actionsが成功。merge後検証は本receipt commit前後に再実行する。
- packaging: Cargo workspace／source treeのみ。
- distribution: `main`へpush後にGitHub repositoryから到達可能になる。
- human review: note／AGENTSの意味境界はユーザー会話を反映。Cockpit画像の今回の目視reviewは未実施。

## `[INTERPRETATION]` 考察

- branch分散を減らしつつ、未解決設計を完成扱いしない中間mergeとして妥当と判断した。

## `[HYPOTHESIS]` 次の仮説

- Claudeの継続作業が同branchへ新checkpointを追加した場合、今回のmerge snapshotとの差分だけを次回回収できる。

## `[UNKNOWN]` 未解決・未試験

- `UNKNOWN`へDEVHAGE根拠が入るsemantic bug、provenance／authority型混線、実Kamii process supervisor、Lane C／F／G。
- unknown authority objectのcache／serve policy、public Registry、recovery authority、TIBIHAGE production scope。
- macOS以外の実機、長時間運転、実鍵import／export／eject、GUI目視。

## command・検証

```console
git merge-tree --write-tree main dev/stage0-transport-cache
git merge --no-ff --signoff dev/stage0-transport-cache
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests -v
python3 -B scripts/foldnic_dev.py validate
python3 -B scripts/foldnic_dev.py doctor
git diff --check
```

## rollback／unmount

- `main`のmerge commitをrevertすれば統合差分を外せる。source branchと各commitは削除せず保持する。

## 引継ぎ

- 次の作業: ClaudeのGreen lane継続分を`main`との差分として管理し、MAGI局所修正から進める。
- User Gate: public authority、cache／serve enforcement、recovery、TIBIHAGE production、Kamii production ABI、Stage 0完成判定。
- 関連Issue／commit: fold-nic #1／#3／#4／#5／#6、`569a2fc`、`9fa9e10`、Actions `33142453010`。
