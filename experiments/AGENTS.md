# AGENTS.md — Fold NIC experiments

実験は再現条件、失敗、停止、UNKNOWNを保存する棚です。`experiments/`へ置いたことはprotocol正本、実装済み、安全保証、推奨設定への昇格を意味しません。

## ファイル名

```text
YYYYMMDD-HHMM__短い題名.ja.md
```

## 必須構造

1. 対象・範囲・除外
2. 元にした仕様／revision
3. 既存tool確認
4. 仮説と反証条件
5. 環境・node・network・clock
6. 手順
7. `[FACT]`
8. `[RESULT]`
9. `[INTERPRETATION]`
10. `[HYPOTHESIS]`
11. `[INNER]`
12. `[UNKNOWN]`
13. 停止条件
14. 昇格候補
15. source・Provenance

private key、seed、token、非公開peer、個人情報を記録しません。失敗を隠して成功例だけを残しません。GNS、DNS、cache、witnessのどこで観測した結果かを分けます。
