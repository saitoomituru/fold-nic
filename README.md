# Fold NIC

Fold NICは、Fold8GのIdentity、World、Capabilityを、名前空間、IAM、Transportへ投影するための仮想意味ネットワークインターフェースです。

物理NIC、kernel driver、ICANN TLD、GNUnet GNSの別名ではありません。`Fold8G`をprotocol generation、`Fold NIC`を端末やserviceへ接続する実装Vesselとして分離します。

```text
Fold Identity / World / Capability
  -> Fold NIC
  -> namespace / IAM / transport adapter
  -> GNS / DNS / IPv6 / LAN / future transport
```

## 現在地

状態: `EXPERIMENTAL / DEVELOPMENT-CONTROL-PLANE / RUNTIME-NOT-IMPLEMENTED`

この初期revisionに含まれるもの:

- `AGPL-3.0-or-later`の配布境界
- ZeroRoomLab-manifestとSphereOS-Atlantisを固定revisionで参照するcomponent registry
- Fold NIC向けSphereOS Atlantis DOSのPrompt Line Interface設定
- 実験ログと開発ログのテンプレート、生成、検証
- local session receiptを作る開発shell
- offline validator、doctor、unit test、GitHub Actions

まだ含まれないもの:

- `.fold` resolver runtime
- GNUnet GNS adapter
- zone key生成、署名、rotation、recovery
- P2P bootstrap、transport failover
- kernel NIC、system resolver差替え
- standalone SphereOS runtime、model inference、常駐scheduler

`unknown ≠ pass`です。開発制御面が動くことを、Fold8G runtimeの実装完了へ拡張しません。

## PLIで開始する

自然言語で設計・実験・実装を開始する場合は、次の順で読みます。

1. [`AGENTS.md`](AGENTS.md)
2. [`docs/development/sphereos-atlantis-dos-pli.ja.md`](docs/development/sphereos-atlantis-dos-pli.ja.md)
3. [`docs/architecture/repository-boundary.ja.md`](docs/architecture/repository-boundary.ja.md)
4. [`workspace/components.json`](workspace/components.json)
5. 対象に最も近い`AGENTS.md`、Schema、test、source

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
