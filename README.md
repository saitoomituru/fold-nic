# Fold NIC

Fold NICは、Fold8GのIdentity、World、Capabilityを、名前空間、IAM、Transportへ投影するための仮想意味ネットワークインターフェースです。

物理NIC、kernel driver、ICANN TLD、GNUnet GNSの別名ではありません。`Fold8G`をprotocol generation、`Fold NIC`を端末やserviceへ接続する実装Vesselとして分離します。

```text
Fold Identity / World / Capability
  -> Fold NIC
  -> namespace / IAM / transport adapter
  -> GNS / DNS / IPv6 / LAN / future transport
```

## なぜこのrepositoryがあるか

発端は`quantaril.site` / `quantaril.help` / `quantaril.cloud`のdomain失効という運用事故だった
（ZeroRoomLab-manifest [#26](https://github.com/saitoomituru/ZeroRoomLab-manifest/issues/26)、
[#27](https://github.com/saitoomituru/ZeroRoomLab-manifest/issues/27)）。この事故を単なるDNS復旧で
終わらせず、「ICANN domainが死んでもproject identityが生き残る」「GitHub accountが消えても
project identityが生き残る」構造そのものを緊急鍛造する場として
ZeroRoomLab-manifest [#28](https://github.com/saitoomituru/ZeroRoomLab-manifest/issues/28)
（`[EMERGENCY][Fold8G] Quantaril Cloudを.fold分散名前空間/P2P基幹へ移行する急造ミッション`）が立った。

Fold NICはこの#28の実装受け皿である。GNS、DNS、GitHub、特定cloud、特定国家をauthority rootへ
昇格させず、Fold Identity・World・Capabilityを複数のIAM方言・transportへ可搬にする。

```text
旧SphereOS 3.x / 4.x: SphereOS/Instance -> Akasha Layer -> Vespa Cloud -> VPC/SaaS/PaaS/tunnel
Fold8G               : Fold Identity/World/Capability -> .fold namespace -> resolver/IAM adapter -> P2P/provider/物理transport
```

社会から隠れるための網ではなく、public web gatewayと分散Identityが共存する形を目指す。GNUnet GNSは
「`.fold`をこれで実装確定する」ためではなく、zone key／signed record／local resolverという概念を
安価に試験するための最初のadapter候補として扱う。

## 現在地

状態: `EXPERIMENTAL / DEVELOPMENT-CONTROL-PLANE / STAGE0-RUNTIME-PARTIAL`

進行はfold-nic [#1](https://github.com/saitoomituru/fold-nic/issues/1)
（`[Stage 0] ベアメタルTransport／Cache基盤を小さく実装する`）で追跡する。開発は
`dev/stage0-transport-cache`branchで小さなcheckpointごとに進め、安定した区切りで`main`へ
中間merge（`--no-ff`、merge記録は開発ログへ）する。Dockerを初期成立条件にせず、非root・
loopback・専用CAS rootのbare metal userspace processから始める。

Stage 0の実装順:

```text
protocol types
  -> local CAS
  -> localhost Fold Gateway
  -> conservative HTTP cache guard
  -> two-process P2P object exchange
  -> provenance receipt
  -> Kamii mock hook
  -> Stage 0統合判定とmain merge
```

この初期revisionに含まれるもの:

- `AGPL-3.0-or-later`の配布境界
- ZeroRoomLab-manifestとSphereOS-Atlantisを固定revisionで参照するcomponent registry
- Fold NIC向けSphereOS Atlantis DOSのPrompt Line Interface設定
- 実験ログと開発ログのテンプレート、生成、検証
- local session receiptを作る開発shell
- offline validator、doctor、unit test、GitHub Actions
- Rust workspaceの基本protocol型（`fold-core`: WorldRef、locator、multihash参照、manifest、receipt、stable error code）
- CID再検証付きlocal CAS（`fold-store`）とloopback限定read-only Gateway（`fold-gateway`）
- private／no-store／CookieをshareへUpgradeしないHTTP共有cache guard（`fold-http-policy`）
- 明示dialによるloopback二プロセス間object交換（`fold-peer`）と、秘密値を含まない取得provenance receipt
- manifest署名のexact bytes検証library（`fold-provenance`。publisher鍵→World authorityのbindingは未実装のまま、意図的に外に出している）
- Kamii adapter呼び出しの timeout／crash を`Allow`へ潰さないmock hook（`fold-kamii`）

まだ含まれないもの:

- `.fold` resolver runtime
- GNUnet GNS adapter
- zone key生成、署名、rotation、recovery
- publisher鍵→World authorityのRegistry契約（binding先をFold NIC本体／ZeroRoomLab-manifest側の
  どちらが正本として持つかは未確認。着手前にUser Gateで確認する）
- Kamii adapterの実process分離、HAGE shared-memory ABI
- P2P discovery／bootstrap、複数node replication、transport failover
- kernel NIC、system resolver差替え
- standalone SphereOS runtime、model inference、常駐scheduler

`unknown ≠ pass`です。開発制御面が動くことを、Fold8G runtimeの実装完了へ拡張しません。`main`への
中間mergeも、Stage 0完成・security audit済み・production配布可能を意味しません。

## PLIで開始する

自然言語で設計・実験・実装を開始する場合は、次の順で読みます。

1. [`AGENTS.md`](AGENTS.md)
2. [`docs/development/sphereos-atlantis-dos-pli.ja.md`](docs/development/sphereos-atlantis-dos-pli.ja.md)
3. [`docs/architecture/repository-boundary.ja.md`](docs/architecture/repository-boundary.ja.md)
4. [`workspace/components.json`](workspace/components.json)
5. 対象に最も近い`AGENTS.md`、Schema、test、source

ベアメタルStage 0の実装順、process分離、停止条件は
[`docs/development/stage0-bare-metal-runtime.ja.md`](docs/development/stage0-bare-metal-runtime.ja.md)を参照してください。
checkpoint commitとbranchの扱いは
[`docs/development/checkpoint-and-branch-policy.ja.md`](docs/development/checkpoint-and-branch-policy.ja.md)を正本とします。

起動表示:

```text
起動モード: Prompt Engineering Edition
操作面: Prompt Line Interface
対象World: fold-nic-forge
standalone runtime: NOT IMPLEMENTED
```

PLIはCLIの模倣ではありません。CLIもPLIの下位版ではありません。自然言語で目的、World、制約、unknownを扱い、決定的に検査できる部分だけをCLI、Schema、fixture、testへ投影します。

## 開発CLI

外部packageを入れず、Python標準libraryだけで動きます。

```console
python3 -B scripts/foldnic_dev.py validate
python3 -B scripts/foldnic_dev.py doctor
python3 -B scripts/foldnic_dev.py sphere-dos boot
python3 -B scripts/foldnic_dev.py sphere-dos status
```

Stage 0のread-only Gatewayは、host CAやorigin fetchを使わずloopbackだけへbindします。

```console
cargo run -p fold-gateway -- \
  --bind 127.0.0.1:7743 \
  --cas-root .fold-nic/cache/cas
curl http://127.0.0.1:7743/healthz
```

このGatewayは現時点でhealthと検証済みlocal CAS objectのreadだけを提供します。write endpoint、origin fetch、P2P、`.fold` TLS ingressは未実装です。

二プロセスP2P実験では、まず共有許可を明示して公開fixtureをserver側CASへ投入します。

```console
cargo run -p fold-peer -- seed-public-fixture \
  --cas-root /private/tmp/fold-peer-server \
  --file ./README.md \
  --acknowledge-loopback-share

cargo run -p fold-peer -- serve \
  --cas-root /private/tmp/fold-peer-server
```

`serve`が返した`/p2p/<PeerId>`付きaddressとseed receiptのCIDを、別processから明示します。

```console
cargo run -p fold-peer -- fetch \
  --cas-root /private/tmp/fold-peer-client \
  --dial /ip4/127.0.0.1/tcp/PORT/p2p/PEER_ID \
  --cid CID \
  --world fold-nic-forge \
  --worldline stage0 \
  --receipt-file /private/tmp/fold-peer-fetch-receipt.json
```

取得receiptは既存fileを上書きせず、CID、World文脈、一時PeerId、host clock観測、未検証事項だけを保存します。object本体、鍵、tokenは含めません。このPeerIdはprocess内だけの一時transport識別子で、Fold Identityではありません。現段階ではloopback以外を拒否し、自動発見、恒久peer鍵、公開network、複数peer複製を提供しません。

実験ログ:

```console
python3 -B scripts/foldnic_dev.py log new \
  --kind experiment \
  --title "GNS二ノード名前解決"
```

開発ログ:

```console
python3 -B scripts/foldnic_dev.py log new \
  --kind development \
  --title "名前parserの負例fixture"
```

新しいログは`[DRAFT]`です。観測、結果、解釈、仮説、UNKNOWNを分け、人間reviewなしでprotocol正本へ昇格しません。

## リポジトリ責務

| 棚 | 責務 |
|---|---|
| `ZeroRoomLab-manifest` | Fold8Gの横断契約、World・Authority・主張境界 |
| `fold-nic` | resolver、adapter、鍵境界、receipt、testの実装正本 |
| `SphereOS-Atlantis` | PLI／CLI／Execution Envelope、MAGI、Sphere-DOSの上流契約 |
| `OpenSourcePITETO` | 実験ログ作法を参照したsource。Fold NICのruntime依存ではない |
| `soikoma` | 旧EtoFLD／qSIM外骨格の凍結artifact。現行仕様をbackfillしない |

## ライセンス

コード、Schema、fixture、CLI、文書を含む本リポジトリは、別記がない限り`GNU Affero General Public License v3.0 or later`で配布します。詳細は[`LICENSE`](LICENSE)を参照してください。
