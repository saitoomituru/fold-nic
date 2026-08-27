# 実験・開発ログ自動化

状態: `[IMPLEMENTED-ALPHA]`

## 設計由来

OpenSourcePITETOの実験ノート作法から、次をFold NICへ適応しました。

- 新規ログを`[DRAFT]`で開始する
- 元revisionと変更差分を残す
- 既存tool探索を記録する
- 観測、結果、解釈、仮説、内観、UNKNOWNを別棚にする
- 一回の成功を正本へ自動昇格しない
- 危険な失敗は再現推奨でなく停止条件として残す

食品、味覚、画像固有fieldは移植せず、network、Identity、authority、key、transport、securityのfieldへ置き換えました。OpenSourcePITETOは設計参照であり、Fold NICのruntime依存ではありません。

## 自動生成

```console
python3 -B scripts/foldnic_dev.py log new --kind experiment --title "題名"
python3 -B scripts/foldnic_dev.py log new --kind development --title "題名"
```

生成時に現在時刻、host timezone、Git revision、worktree状態を埋めます。取れない値は`unknown`とし、推測しません。

## 検証

```console
python3 -B scripts/foldnic_dev.py log validate
```

validatorはfilenameと必須見出しを検査します。本文の技術的真偽、実験実行、人間review、security保証は判定しません。
