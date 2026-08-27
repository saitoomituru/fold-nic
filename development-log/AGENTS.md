# AGENTS.md — Fold NIC development log

開発ログは、実装差分、tool再利用、検証、失敗、引継ぎを保存する棚です。commit messageや会話ログの代用品ではなく、次の開発者が同じ判断とtoolを再発明しないためのreceiptです。

## ファイル名

```text
YYYYMMDD-HHMM__短い題名.ja.md
```

## 必須境界

- source revisionと対象branch
- 目的・非目標
- 既存tool探索とreuse／extend／adapt／create判断
- 入出力、副作用、権限、network、secret境界
- 変更したfile
- 実行commandと結果
- 実装、統合、検証、梱包、配布、人間reviewを別状態にする
- `[FACT]`、`[RESULT]`、`[INTERPRETATION]`、`[HYPOTHESIS]`、`[UNKNOWN]`
- rollback／unmount方法

CI greenをsystem greenへ拡張せず、未収集testを実行済みにしません。
