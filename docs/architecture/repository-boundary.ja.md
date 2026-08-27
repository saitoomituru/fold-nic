# Fold NICリポジトリ責務境界

状態: `[CANONICAL-CANDIDATE]` `[Layer A/B bridge]`

## 目的

Fold8Gの意味・契約正本と、network runtimeの実装・配布・security責務を分けます。

```text
ZeroRoomLab-manifest
  Fold8G protocol / World / Authority / claim boundary

fold-nic
  parser / resolver / adapter / key boundary / receipt / test

SphereOS-Atlantis
  PLI / CLI / Execution Envelope / MAGI / Sphere-DOS upstream contract

OpenSourcePITETO
  experiment-log operating pattern reference

soikoma
  frozen EtoFLD / qSIM historical artifact
```

同じworkspaceや系譜へ属することは、import、link、write authority、同一licenseを生成しません。

## stable ID候補

```text
protocol generation: fold8g
implementation vessel: fold-nic
local development world: fold-nic-forge
PLI machine ID: prompt-line
CLI machine ID: command-line
```

Fold8Gを単純な物理通信Layer番号へ固定しません。現行候補は、複数World間のIdentity、Authority、Capability、Provenance、Transportを変換・交渉するInter-World Protocolです。

## GNS境界

```text
Fold Identity
  != GNS zone key
  != ordinary DNS domain
  != GitHub account
  != transport endpoint
```

GNSは最初の実装adapter候補です。GNSが停止、変更、終了してもFold Identityとprotocol契約が残る構成を目標にします。

## public webとの共存

Fold NICは通常Webからの撤退装置ではありません。

```text
public web gateway = 人間・検索・第三者監査から読める入口
.fold / GNS          = 分散名前空間とidentity-resolution plane
Git / GitHub         = replica / witness / distribution path
anonymous transport = optional capability
```

source availability、retrieval coverage、ranking、authority verificationを一つのfailure classへ潰しません。

## 現在の実装状態

| 対象 | 状態 |
|---|---|
| repository control plane | `IMPLEMENTED-ALPHA` |
| PLI development profile | `IMPLEMENTED-ALPHA` |
| experiment／development log automation | `IMPLEMENTED-ALPHA` |
| offline validator／doctor | `IMPLEMENTED-ALPHA` |
| `.fold` parser | `NOT_IMPLEMENTED` |
| local resolver | `NOT_IMPLEMENTED` |
| GNUnet GNS adapter | `NOT_IMPLEMENTED` |
| Identity key lifecycle | `NOT_IMPLEMENTED` |
| P2P content delivery | `NOT_IMPLEMENTED` |
| production distribution | `NOT_IMPLEMENTED` |
