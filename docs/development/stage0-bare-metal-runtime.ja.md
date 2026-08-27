# Stage 0 ベアメタルruntime契約

状態: `[ACTIVE-DESIGN]` `[RUNTIME-NOT-IMPLEMENTED]`

参照:

- ZeroRoomLab-manifest Issue #28
- Issue #28 comment `5434635760`: 通信cache先行
- Issue #28 comment `5434974574`: v0 stack freeze

## 成立条件

Stage 0はDocker、kernel extension、system resolver差替え、host Local CA、IBD、HAGE実鍵を成立条件にしません。

```text
protocol types
  -> local CAS
  -> localhost Fold Gateway
  -> conservative HTTP cache guard
  -> two-process P2P object exchange
  -> provenance receipt
  -> Kamii mock hook
```

初回reference machine:

```text
architecture: x86_64
OS: macOS 15.7.7
runtime: Rust + Tokio
deployment: unprivileged userspace processes
network bind: loopback / high port by default
```

この実機での成功を、macOS一般、Apple Silicon、Linux、Windows、ARMへ拡張しません。

## process境界

初期実装でも責務を論理moduleへ分け、外部adapterは別processにできる境界を保ちます。

```text
fold-gateway
  -> HTTP ingress / origin fetch / cache policy

fold-store
  -> content-addressed local object store

fold-peer
  -> discovery / request-response / replication

kamii-adapter
  -> optional out-of-process inspection

hage-device
  -> later userspace shared-memory mock
```

最初から全moduleを常駐daemonへ分割する必要はありません。ただしlibrary境界、I/O型、error、receiptを分け、取得objectを自動実行しません。

## 最初の安全条件

- rootで実行しない
- defaultは`127.0.0.1`へbindする
- 1024番以下のportを要求しない
- CAS rootは明示pathとし、workspace rootやhome全体を使わない
- size上限、object上限、atomic publish、hash再検証を持つ
- path traversalとsymlink escapeを拒否する
- HTTPの`private`、`no-store`、Authorization等をshared cacheへ昇格しない
- protocol判定不能を`SHARE_CACHE`へ変換しない
- adapter timeout／crashを`ALLOW`へ変換しない
- active contentや取得binaryを自動実行しない
- private key、token、Cookie、Authorizationをreceiptへ保存しない

## logical namespaceとwire境界

`.fold`はlogical route selectorです。IANA登録済みTLDやpublic DNS所有権を主張しません。

```text
logical Fold name != wire hostname != TLS certificate name
```

Stage 0ではlocalhost pathまたは明示proxy metadataを使えます。`HOST_CA`は後続実機試験とし、CAなしfallbackが同一`https://*.fold` UXを保証するとは書きません。

## objectと署名境界

```text
Fold NIC core
  = canonical object bytes
  = multihash / CID検証
  = signed envelope検証
  = receipt routing

external signer / HAGE
  = signature生成
  = private key保持
```

Fold Identity秘密鍵とHAGE秘密値をFold NIC stateへ保存しません。Browser ingress用Local CA鍵は、導入時に別のdevice-local Gateway stateとして管理し、Fold IdentityやHAGE Identityへ昇格しません。

## cache無効化の意味

revocationとtombstoneは、すでに他peerへ複製されたbyte列の世界的削除を保証しません。

```text
revocation = bindingを今後有効として扱わないclaim
tombstone  = publisher / Worldが今後serveしないclaim
quarantine = local policyで取得・表示・実行を止めるstate
```

partition中のpeerへ即時到達するとは主張せず、最終確認時刻とfreshnessをreceiptへ残します。

## Dockerを追加する条件

Docker DesktopはStage 0の必須dependencyにしません。次が必要になった時点でLinux node再現用に追加できます。

- 3 node以上のmesh
- network partition／再接続
- disk quota付きpeer
- 壊れたpeer／悪性peer
- Linux CIと再現環境

macOS Keychain、Local CA、Darwin shared memory、launchd、HAGE macOS backendはnative実機で検証します。

## 設計停止条件

次が安全な局所修正で閉じない場合はIssueへ記録して停止します。

- object canonicalizationが一意にならず、同じIDで異なるbyte列を受理する
- cache guardを通らずAuthorization／Cookie／private responseがpeerへ流れる
- path traversal等でCAS root外へwriteできる
- external inputだけでarbitrary code executionへ到達する
- Identity鍵、HAGE鍵、Local CA鍵の責務境界を維持できない
- fail-closed Worldがplain transportへsilent downgradeする

compile error、dependency version調整、単体test failure、localhost port競合は通常debugとして扱います。
