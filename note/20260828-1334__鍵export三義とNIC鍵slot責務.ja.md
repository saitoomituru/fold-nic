# [DRAFT] 鍵export三義とNIC鍵slot責務

作成日時: 2026-08-28T13:34:02+09:00
対象branch: `dev/stage0-transport-cache`
起点revision: `c67bb18ec69c53494d8136c8b6f6fc0340024b45`
種別: `[DESIGN-NOTE]`

## 目的

`export`という同じ語で、Rust symbol公開、Fold NIC製品の鍵抜差し、agentによる無断抽出を混同しないための補助定規を残す。

このnoteは、鍵protocol、ABI、production policyの正本ではない。実装済み、安全確認済み、配布可能という状態を生成しない。

## `[FACT]` 観測

- fold-nicの旧AGENTSは、private key／seedをcommit、log、receiptへ入れないと一括記載していた。
- Issue #5は、既存の決定的Ed25519 seedを、秘密性を持たない公開TEST fixtureとして扱う設計を含む。
- `fold-authority`の`DEVHAGE_PRIVATE_KEY_SEED_HEX`は、全byteが既知の公開TEST値であり、利用者環境から抽出したactual secretではない。
- 開発ログは「README／AGENTSが公開TEST fixtureを許可する」と記載したが、当時のfold-nic AGENTSにその例外文はなかった。
- Fold NICが鍵のimport／export／eject／rotate機能を持つことと、agentが利用者の鍵を無断で抜くことは別の操作である。

## exportの三義

### 1. language symbol export

Rustの`pub const`、`pub fn`、crate APIとして値や操作を参照可能にすること。公開済みの決定的TEST fixtureを`pub`にすることは、それだけでは利用者の秘密鍵抽出を意味しない。

### 2. product key operation

利用者がNICへ鍵を挿す、関連付ける、抜く、移す、交換する製品Capability。SIM／eSIM／HSM slotの操作に近い。

候補operation:

```text
ATTACH_KEY
  key handle／provider／World bindingをNIC slotへ装着する

EJECT_KEY_HANDLE
  NICから関連付けを外す。raw key material取得とは限らない

EXPORT_KEY_MATERIAL
  key materialを明示destinationへ移送する

WRAPPED_EXPORT
  移送先だけが開ける形でexportする

ROTATE_KEY
  新鍵へ切り替え、旧鍵の失効／回復状態を記録する
```

Fold NICはcontrol-plane API、Capability検査、World scope、User Gate、provider呼出し、fail-closed、監査receiptを所有できる。HAGE providerはbacking key materialとprovider固有の生成／封印／export方式を担当できる。どちらか一方だけへ全責務を押し込まない。

### 3. agent exfiltration

agentが明示User Gateなしに実在key materialを探索、読出し、export、commit、Issue、log、通常receiptへ流すこと。製品Capabilityが存在しても、agentへその実行権限が自動付与されるわけではない。

安定候補token:

```text
AGENT_UNAUTHORIZED_KEY_ACCESS
USER_AUTHORIZED_KEY_OPERATION
INSECURE_PUBLIC_TEST_KEY
```

## 鍵可搬性

providerごとにexport可能性は異なる。Capabilityがないproviderへraw exportを強制しない。

```text
EXPORTABLE
WRAPPED_EXPORT_ONLY
HANDLE_TRANSFER_ONLY
NON_EXPORTABLE
```

`EJECT_KEY_HANDLE`と`EXPORT_KEY_MATERIAL`を同義にしない。non-exportable HSM鍵でも、NIC slotからhandleを外すことはできる場合がある。

## payloadとreceipt

```text
key payload channel
  raw／wrapped key material
  明示destination
  短寿命
  通常log禁止

audit receipt
  operation
  key_id
  provider_ref
  source／destination slot
  World／Worldline scope
  authorization_ref
  result
  raw key materialなし
```

export payloadを監査receiptへ埋め込まない。receiptを削除しても鍵操作の結果が取り消されたとは扱わず、rotation／revocation／recoveryは別stateとして管理する。

## 公開TEST fixture

`INSECURE_PUBLIC_TEST_KEY`は秘密鍵形式であっても、最初から秘密性を放棄した相互運用vectorである。actual secretと同じincident処理へ入れない。

最低条件:

- 専用fixture pathまたは明示module
- `INSECURE_PUBLIC_TEST_KEY`
- production不可
- public advertisement不可
- source repository／revision／fixture ID
- public key、signature、期待結果との整合test
- production profileでfingerprint拒否

fixtureを正本repositoryから複製する場合は、Provenanceと同期testを残す。隣接repositoryの存在だけでcopy authorityを生成しない。

## `[INTERPRETATION]` 今回の誤読

公開TEST fixtureを認めるユーザー設計は存在した。一方、当時のfold-nic AGENTSには例外文がなかった。したがって問題はfixtureの存在そのものではなく、限定された作業指示を「AGENTSの許可」と記録したsource attribution errorである。

さらに、agent操作禁止と製品Capability禁止を同じ`export`へ縮退させると、NICとして必要な鍵の抜差し機能まで削除する逆方向の誤りが起きる。

## `[UNKNOWN]` User Gate

- raw exportを許すWorld／provider／profile
- user presence、複数承認、時間制限の具体方式
- wrapped exportのrecipient binding
- recovery authorityと旧鍵に依存しない失効経路
- HAGE provider間の正式な移送ABI
- export後にsource側keyを残すcopyか、破棄するmoveか

これらは実装者が自動決定しない。

## 0.6xx.n世代へ渡す再発防止材料

Claudeを含むagentが「AGENTSで許可されている」と主張する場合、repository、AGENTS revision、該当行、scope、競合規則、追加規則の出所、優先順位をreceiptへ要求する案がある。

候補診断:

```text
RULE_SOURCE_UNVERIFIED
RULE_SCOPE_CONFLICT
SOURCE_CLOSURE_STALE
```

このExecution Envelope／agent規律の実装責務はSphereOS Atlantis DOS 0.6xx.n世代にあり、Fold NICでは実装しない。

## Provenance

- fold-nic Issue #5: DEVHAGE／TRUEHAGE鍵bootstrap
- fold-nic Issue #1: Stage 0実装・統合
- EDOHAGE-TUBO Issue #1: reference HAGE provider
- fold-nic #5 issuecomment-5448445864: agent操作と製品Capabilityの再訂正
- fold-nic #1 issuecomment-5448447963: 次工程指示の訂正
