# Fold NIC向けSphereOS Atlantis DOS — Prompt Line Interface

状態: `[IMPLEMENTED-ALPHA]` `[PROMPT-ENGINEERING-EDITION]`

## 起動表示

```text
起動モード: Prompt Engineering Edition
操作面: Prompt Line Interface
machine ID: prompt-line
対象World: fold-nic-forge
利用可能入口: orient / experiment / implement / validate / handoff
standalone runtime: NOT IMPLEMENTED
```

このPLIは、自然言語で目的、World、Identity、Capability、制約、UNKNOWNを扱う正規操作面です。Python CLIの模倣表示ではありません。

同時に、PLIが存在することをmodel inference、daemon、scheduler、resolver、GNS、P2P runtimeの実装証拠にしません。

## source closure

```text
User request
  -> fold-nic/AGENTS.md
  -> fold-nic README / local contract
  -> workspace/components.json
  -> ZeroRoomLab-manifest AGENTS / Issue #28 / required contract
  -> SphereOS-Atlantis AGENTS / PLI / runner boundary
  -> nearest local AGENTS / Schema / test / source
```

必須sourceが欠損した場合は`CONTEXT-INCOMPLETE`で停止します。会話記憶や現在cwdから不足箇所を補完しません。

## 入口

### `orient`

対象、正本、repository、license、実装状態、未実装、secret境界を表示します。変更しません。

### `experiment`

仮説、反証、入力、環境、観測、停止条件を設計し、`experiments/`へDRAFTログを作ります。実験実行と記録作成を同一化しません。

### `implement`

Userが実装を明示した後、既存tool探索、small change、test、receiptへ進みます。設計会話だけから実装権限を生成しません。

### `validate`

CLI、Schema、fixture、testへ投影済みの条件を機械検査します。未符号化の意味妥当性や人間reviewをgreenへ変換しません。

### `handoff`

source revision、差分、command、結果、副作用、UNKNOWN、次のUser Gateを開発ログへ残します。

## PLIからCLIへの投影

```text
natural-language intent
  -> repository contract
  -> selected operation
  -> scripts/foldnic_dev.py
  -> unit test / validator / GitHub Actions
  -> local or remote receipt
```

CLIで現在利用できる操作:

```console
python3 -B scripts/foldnic_dev.py doctor
python3 -B scripts/foldnic_dev.py validate
python3 -B scripts/foldnic_dev.py log new --kind experiment --title "..."
python3 -B scripts/foldnic_dev.py log new --kind development --title "..."
python3 -B scripts/foldnic_dev.py sphere-dos boot
python3 -B scripts/foldnic_dev.py sphere-dos status
```

## local session receipt

`sphere-dos boot`は`.fold-nic/sphere-dos/`へlocal receiptを書きます。

行うこと:

- profileとcomponent registryのoffline検証
- Git revisionとworktree状態の観測
- local session receiptとcurrent pointerの生成

行わないこと:

- network access
- model call
- provider login
- secret探索
- component clone／更新
- resolver／GNS／P2P runtime起動
- standalone SphereOS runtime起動

`.fold-nic/`はGit追跡対象外です。公開が必要なreceiptは秘密を除き、別のreview済み開発ログとして作ります。
