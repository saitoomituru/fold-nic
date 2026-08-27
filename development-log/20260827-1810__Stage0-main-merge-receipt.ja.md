# Stage 0 main merge receipt（中間統合）

作成日時: 2026-08-27 18:10:00 +0900
作成者／agent: Claude
状態: `[DRAFT]`
種別: `[DEVELOPMENT]`

## 目的・非目標

- 目的: `dev/stage0-transport-cache`で積み上げたStage 0の実装済みcheckpoint（protocol型からKamii mock hookまで）を、停電・local storage事故に対する保存receiptとして`main`へ統合する
- 非目標: Stage 0全体の完成宣言、production配布可能宣言、manifest署名Registry契約の確定は行わない。これらは未解決のまま`main`へも引き継ぐ

## source・revision

- source branch: `dev/stage0-transport-cache`（`4eb1794`）
- target branch: `main`（merge前`3690dc7`、merge後`bf007ae`）
- merge方式: `git merge --no-ff`（checkpoint単位のcommit historyを保持）
- upstream／contract: ZeroRoomLab-manifest #28、fold-nic #1、`docs/development/checkpoint-and-branch-policy.ja.md`のmerge記録要件

## 既存tool確認

- 探索したtool: 該当なし（Git操作そのもの）
- reuse／extend／adapt／create: 既存merge手順（policy文書のmerge項）をそのまま適用
- 判断理由: branch protectionは`main`に未設定（`gh api .../branches/main/protection`が404）だが、無審査でmergeするのではなく、policy文書のmerge記録項目をcommit message自体へ埋め込む形で判断根拠を残した

## 入出力・副作用・権限

- 入力: `dev/stage0-transport-cache`の11 commit
- 出力: `main`上のmerge commit`bf007ae`
- filesystem副作用: `main`のworking treeへ42 fileが新規追加（衝突なし、fast-forwardではなくmerge commit）
- network: `git push origin main`のみ
- authority: このmergeはGitHub上のrepository owner権限で実行。他者のreview承認は経由していない
- secret境界: 追加されたfileにprivate key、token、生content bytesを含まない（既存checkpointで確認済み）

## `[FACT]` 事実・観測

- merge前の`main`は`3690dc7`（Fold NIC開発制御面の初期構成のみ）で、Stage 0実装を一切含んでいなかった。
- mergeはconflictなしで完了した（`git merge --no-ff`が`Merge made by the 'ort' strategy.`を返し、conflict markerなし）。
- merge後の`main`（`bf007ae`）で`cargo test --workspace --locked`全成功、`cargo fmt --all -- --check`通過、`cargo clippy --workspace --all-targets --locked -- -D warnings`警告0、`python3 -B -m unittest discover -s tests`5件成功、`foldnic_dev.py validate`PASS、`git diff --check`エラーなしを個別に再実行して確認した。
- push後のGitHub Actions（`main`、`bf007ae`）はcompleted / successで完了した。

## `[RESULT]` 結果

- implementation: 該当なし（本entryはmerge記録）
- integration: `main`が`dev/stage0-transport-cache`のcheckpoint 7〜8件を含む状態になった
- automated verification: 上記`[FACT]`のとおり全件成功
- packaging: 該当なし
- distribution: `main`へpush済み（`bf007ae`）
- human review: `PENDING`（ownerによる目視reviewは未実施。branch protectionもreview必須化もされていない）

## `[INTERPRETATION]` 考察

- `main`へのmergeは、Issue #1の「Stage 0統合判定とmain merge receipt」checklistを完全に満たすものではない。manifest署名Registry契約のUser Gateと、Kamii adapterの実process分離が未解決のまま残っているため、これは**中間統合**として扱う。
- `dev/stage0-transport-cache`は削除せず、残り2項目（Registry契約、Stage 0最終統合判定）の作業branchとして維持する。今後のcheckpointは同branchへ積み、次に安定した区切りで再度`main`へmergeする想定。

## `[HYPOTHESIS]` 次の仮説

- Registry契約の設計判断（publisher鍵→World authority bindingをFold NIC本体側／ZeroRoomLab-manifest側どちらが正本として持つか）が決まれば、それ自体を新しい独立したcheckpointとして`dev/stage0-transport-cache`へ積み、その後の区切りで再度`main`へmergeできる。

## `[UNKNOWN]` 未解決・未試験

- manifest署名Registry契約のbinding先（User Gate待ち）
- Kamii adapterの実process分離
- macOS以外の実機、Apple Silicon実機
- P2P discovery、複数node、network partition耐性
- `main`上でのhuman review（`PENDING`のまま）

## command・検証

```console
git checkout main
git merge --no-ff dev/stage0-transport-cache -m "..."
cargo test --workspace --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
python3 -B -m unittest discover -s tests -v
python3 -B scripts/foldnic_dev.py validate
git diff --check
git push origin main
```

## rollback／unmount

- `git revert -m 1 bf007ae`で`main`を`3690dc7`相当へ戻せる。`dev/stage0-transport-cache`は影響を受けない。

## 引継ぎ

- 次の作業: `dev/stage0-transport-cache`でRegistry契約User Gateの確認、またはKamii実process分離の着手
- User Gate: manifest署名のpublisher鍵→World authority Registry契約binding先（継続中、未解決）
- 関連Issue／commit: ZeroRoomLab-manifest #28、fold-nic #1、merge commit `bf007ae`
