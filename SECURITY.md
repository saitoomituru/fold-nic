# Security Policy

Fold NICはIdentity、鍵、名前解決、network transportへ触れるため、通常の不具合とsecurity問題を分けて扱います。

## 公開Issueへ書かないもの

- private key、seed、recovery secret
- access token、password、cookie
- private peer address、非公開Worldの識別情報
- 攻撃が継続中のsystemを特定できる生log

公開Issueには秘密を除いた再現条件、影響、対象revision、期待した停止条件を書いてください。秘密情報を含む報告経路が必要な場合は、repositoryのGitHub Security Advisoryを使用してください。

## 初期security状態

resolver、GNS adapter、鍵rotation、recovery、multi-witness、production配布は`NOT_IMPLEMENTED`です。開発toolとCIがgreenでも、暗号強度、耐攻撃性、運用安全性を保証しません。

次は高優先度です。

- private keyまたはcredentialのGit混入
- silent DNS downgrade
- stale／rollback／replayを正常解決として返す問題
- World AのauthorityをWorld Bへ無条件継承する問題
- receiptへ秘密値を展開する問題
