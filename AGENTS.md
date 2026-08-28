# AGENTS.md — Fold NIC

このファイルは、人間およびAIエージェントがFold NICを編集するときの局所規約です。

## 目的

Fold NICは、Fold8GのIdentity、World、Capabilityを、名前空間、IAM、Transportへ投影する仮想意味ネットワークインターフェースです。

GNS、DNS、GitHub、特定cloud、国家、transportをauthority rootへ昇格しません。社会から隠れるための網へ縮退させず、public web gatewayと分散Identityを共存させます。

## 必読順序

1. [`README.md`](README.md)
2. [`docs/development/sphereos-atlantis-dos-pli.ja.md`](docs/development/sphereos-atlantis-dos-pli.ja.md)
3. [`docs/architecture/repository-boundary.ja.md`](docs/architecture/repository-boundary.ja.md)
4. [`workspace/components.json`](workspace/components.json)
5. ZeroRoomLab-manifestの`AGENTS.md`とIssue #28
6. SphereOS-Atlantisの`AGENTS.md`とPLI／runner境界
7. 変更対象に最も近い`AGENTS.md`、Schema、test、source

componentは固定revisionで参照します。隣接path、会話記憶、workspace membershipから正本や変更権限を推定しません。必須sourceを読めない場合は`CONTEXT-INCOMPLETE`と`stop-before-mutation`を返します。

Stage 0 runtimeを変更する場合は、[`docs/development/stage0-bare-metal-runtime.ja.md`](docs/development/stage0-bare-metal-runtime.ja.md)と
[`docs/development/checkpoint-and-branch-policy.ja.md`](docs/development/checkpoint-and-branch-policy.ja.md)も読みます。

## 日本語既定

- README、技術文書、実験ログ、開発ログ、commit、PR、Issue、code comment、CLI help、検証報告は日本語を既定とする
- code identifier、protocol field、Schema key、API、安定path、機械可読tokenは互換性を壊す翻訳をしない
- 英語が必要な場合も、意味、責務、境界、未検証事項を日本語で説明する
- 古い言い回しや不自然な漢語へ寄せず、現在日常的に使われる常用令和日本語で書く
- commitは`[layer] scope: 日本語の説明`を基本とする

## PLI／CLI／Execution Envelope

- 自然言語操作面は`Prompt Line Interface`、machine IDは`prompt-line`
- command操作面は`Command Line Interface`、machine IDは`command-line`
- LLM、provider、connector、hostはExecution Envelopeとして別管理する
- PLIとCLIを真贋、上下、完全版／簡易版へ変換しない
- interfaceからidentity、World、authority、実装状態を推定しない
- PLIからSchema、fixture、CLI、testへ投影した箇所だけを機械検証済みとする

## 実装境界

```text
Fold Identity != GNS zone key != DNS domain != GitHub account
Capability != Authority != Contract != Endorsement
workspace membership != implementation dependency
local green != system green
unknown != pass
```

- GNSは最初のadapter候補であり、Fold Identityの正本ではない
- `.fold`はICANN TLD所有を主張しない
- DNS fallbackをsilent downgradeにしない
- agentは、利用者／production／非公開Worldの実在key materialを、明示User Gateなしに探索、読出し、export、commit、log、Issue、通常receiptへ入れない
- `INSECURE_PUBLIC_TEST_KEY`として最初から秘密性を放棄した決定的fixtureは、専用fixture、production不可、source revision、security classを明示した場合に限り使用できる
- このagent操作制限は、Fold NIC製品が提供するuser-authorized key import／export／eject／rotate capabilityを禁止しない
- 鍵操作のpayload channelと監査receiptを分け、通常receiptにはraw key materialを入れず、操作、key ID、provider ref、scope、権限ref、結果を記録する
- 鍵方式、rotation、recovery、multi-witnessが未確定な間は安全性を保証済みと書かない
- resolver、GNS adapter、P2P runtimeが未実装なら`NOT_IMPLEMENTED`を維持する

鍵exportという語の三義、NIC鍵slot、provider委譲、可搬性profileの補足は、[`note/20260828-1334__鍵export三義とNIC鍵slot責務.ja.md`](note/20260828-1334__鍵export三義とNIC鍵slot責務.ja.md)を参照します。noteは設計材料であり、protocol正本や実装receiptへ自動昇格しません。

## 既存toolを先に探す

新しいscript、validator、doctor、log generator、adapterを書く前に、最低限次を実行します。

```console
find . -maxdepth 4 \( -path '*/scripts/*' -o -path '*/tools/*' -o -path '*/lib/*' \) -type f -print
rg -n "<今回の入力・出力・処理を表す語>" .
```

同じ入力、出力、副作用、権限、Execution Envelopeを持つtoolは再利用または拡張します。別実装が必要なら、license、OS、ABI、authority、offline条件、secret境界の差を開発ログへ残します。

他repositoryのコードはworkspace membershipだけでcopyしません。source revision、license、Provenance、test条件を確認します。

## 実験ログと開発ログ

- 実験は[`experiments/AGENTS.md`](experiments/AGENTS.md)に従う
- 開発変更は[`development-log/AGENTS.md`](development-log/AGENTS.md)に従う
- 新規ログは`[DRAFT]`
- `[FACT]`、`[RESULT]`、`[INTERPRETATION]`、`[HYPOTHESIS]`、`[UNKNOWN]`を混ぜない
- source revision、入力、command、環境、副作用、検証、未試験範囲を残す
- 成功例だけを選び、失敗率や停止条件を消さない
- ログからREADME、protocol、runtime statusへ自動昇格しない

## 自動検証

変更後は原則として次を実行します。

```console
python3 -B -m unittest discover -s tests -v
python3 -B scripts/foldnic_dev.py validate
python3 -B scripts/foldnic_dev.py doctor
git diff --check
```

GitHub Actionsの成功は、記載した検査がそのrevisionで通ったことを示します。GNS実機、P2P、鍵回復、system resolver、production配布、目視確認の代用にはしません。

## checkpointとremote保存

- 小さく意味のある変更ごとに日本語commitを作り、検証範囲とUNKNOWNを開発ログへ残す
- 完成を待たず、再開可能な状態ならremoteへpushする
- `main`以外のbranch上でも、配布済み・統合済みとは主張しない
- branchは作業の意味境界がある場合だけ作る。臨時待避branchには理由と廃棄／merge条件を残す
- mergeはsource branch、target branch、検証、未試験範囲、merge後のcommitを記録する
- remote pushは停電・local storage事故に対する保存receiptであり、実装完了やreview完了を意味しない

## 致命的問題の停止条件

次のいずれかを確認し、安全な局所修正で閉じられない場合は、詳細な日本語Issueを作成して停止します。

- AGPLと互換しないcode／dependencyを結合しなければ実装できない
- private key、credential、個人情報、非公開World資産がGit履歴へ入った疑い
- Identity continuity、authority、key recoveryを不可逆に破壊する仕様衝突
- 必須のManifest／Atlantis正本を解決できず、推測実装になる
- validatorが秘密を出力する、またはsilent DNS downgradeを許す
- test failureを成功扱いしないとmainへ進めない

Issueには、対象revision、再現手順、観測結果、期待結果、影響、秘密を含まない最小log、試した回復、UNKNOWN、停止範囲を記載します。秘密値そのものは書きません。

通常の未実装、単発test failure、依存未導入は直ちに致命的問題へ昇格しません。安全に修正または`NOT_IMPLEMENTED`として保持します。

## 引継ぎ票

作業終了時は会話だけに依存せず、必要に応じて開発ログへ次を残します。

```text
resolved workspace:
target repository and branch:
source revisions:
existing tool searched:
reused / extended / created:
inputs and outputs:
side effects:
commands:
validation:
unknown / human review:
```
