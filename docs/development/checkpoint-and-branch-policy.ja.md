# checkpoint・branch・merge運用

状態: `[ACTIVE]`

## 目的

Fold NICは寺子屋型の独立R&Dとして、完成品だけでなく途中の再開可能な成果もremoteへ保存します。停電、local storage故障、実機事故による喪失を、未完成commitを見せる恥より重いriskとして扱います。

```text
small change
  -> local verification
  -> Japanese semantic commit
  -> remote push
  -> Issue / development-log receipt
```

remote pushは次を意味しません。

- 実装完了
- `main`統合済み
- security audit済み
- production配布可能
- 複数実機で検証済み

## commit

- 一つのcommitへ一つの意味境界を置く
- subjectは`[layer] scope: 日本語の説明`を基本とする
- DCOの`Signed-off-by`を付ける
- source変更には可能な範囲でtestまたはfixtureを同じcommitへ含める
- 未完成でも、build可能または失敗条件が明示され、次の作業者が再開できるならcheckpointにできる
- 既知のtest failureを隠してpushしない。失敗を含む実験branchでは期待failureと再現手順を開発ログへ記録する

## branch

branchを作るのは次の場合です。

- 複数commitで一つのStageを構成する
- `main`の既存検証を一時的に満たせない実験
- protocol／Schema互換性を壊す候補
- 外部dependencyや実機待ちの臨時待避

一つのStage内で細かな作業ごとにbranchを増やしません。Stage 0 transport/cache基盤は`dev/stage0-transport-cache`を共有branchとします。

臨時待避branchには、少なくとも次を開発ログへ残します。

```text
branch purpose:
source revision:
blocking condition:
merge condition:
discard condition:
last remote checkpoint:
```

## merge

mergeは単なるGit操作ではなく、候補revisionをtarget branchの契約へ統合する判断です。merge前に次を記録します。

- source branchとsource commit
- target branchとtarget commit
- merge方式
- 実行したtest／validator
- 未試験のOS、実機、network、security境界
- rollback方法

merge後はtarget branchの新commitで検証を再実行し、そのcommitをremoteへpushします。virtual merge、clean merge、CI greenだけをruntime実行済みへ昇格しません。

## Issue報告

通常の実装checkpointは、関連Issueへ以下を簡潔に追記できます。

```text
branch / commit:
implemented:
verification:
not implemented:
unknown:
next checkpoint:
```

設計級の停止条件では、秘密を含まない再現手順、観測結果、影響、回復案、停止範囲を詳細な日本語Issueへ記録して停止します。単発bug、compile error、dependency調整は自動debug対象であり、直ちに設計停止へ昇格しません。
