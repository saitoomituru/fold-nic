# Fold NICへの参加

Fold NICは`AGPL-3.0-or-later`の贈与コモンズとして開発します。参加前に[`AGENTS.md`](AGENTS.md)を読んでください。

## 基本手順

1. Issueまたは実験ログで目的、対象、非目標、停止条件を明示する。
2. 既存script、validator、fixtureを検索する。
3. 小さく意味のまとまった変更を行う。
4. unit test、validator、doctor、`git diff --check`を実行する。
5. 実装済み、未実装、未試験、UNKNOWNを分ける。
6. 再開可能なcheckpointごとにremoteへpushし、停電・local storage事故に備える。

commitは次を基本形とします。

```text
[eng] resolver: 名前parserの負例を追加
[docs] pli: Fold NIC開発入口を更新
[meta] repository: 自動検証を追加
```

Developer Certificate of Origin 1.1に従い、commitへ`Signed-off-by`を付けてください。署名は、その貢献を提出する権利があり、本リポジトリのライセンスで公開できるという自己申告です。著作権を単一主体へ譲渡するCLAではありません。

```console
git commit -s
```

## branchとmerge

通常は`main`または一つの目的を持つ開発branchで進め、作業単位ごとにbranchを増やしません。

branchを作る理由は、長めの実験、互換性を壊す候補、臨時待避、複数commitをまとめて検証する必要がある場合です。merge前後には、source／target、検証済みrevision、未試験範囲、rollback方法を開発ログまたはPRへ残します。

詳細は[`docs/development/checkpoint-and-branch-policy.ja.md`](docs/development/checkpoint-and-branch-policy.ja.md)を参照してください。

## pull request

PRには次を書きます。

- 何を変え、なぜ必要か
- source、revision、lineage
- 入出力、副作用、権限、network利用
- 実行した検証と結果
- 未試験範囲、UNKNOWN、人間review
- rollbackまたはunmount方法
