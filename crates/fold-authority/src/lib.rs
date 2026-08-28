// SPDX-License-Identifier: AGPL-3.0-or-later

//! publisher署名の数学的検証（`fold-provenance`）と、その公開鍵をWorld authorityとして
//! 信頼するかどうかの判断を分離する境界。
//!
//! この crateが固定する不変:
//!
//! - `signature_valid == true`を`authority trusted`へ自動昇格しない
//! - `signature_valid == false`のときは`PublisherAuthorityResolver`を呼ばない
//!   （[`resolve_if_signature_valid`]参照）
//! - resolverが対象を解決できない場合は`AuthorityStatus::Unknown`と
//!   `TrustBasis::Unresolved`を組で返し、未解決の判断根拠を確定済みの根拠
//!   （例: 特定fixtureの信頼）と混同しない
//!
//! ## Vessel境界（MAGI監査 fold-nic#5 issuecomment-5448141382）
//!
//! この crateは**neutral resolver interface**だけを持つ。DEVHAGE等の具体的な
//! bootstrap fixture、鍵生成、local key providerの実装は
//! [EDOHAGE-TUBO](https://github.com/saitoomituru/EDOHAGE-TUBO)の責務であり、
//! この crateには含めない。[`UnresolvedAuthorityResolver`]は、具体的providerが
//! まだ配線されていない場合の既定resolverとして、常に`Unknown`／`Unresolved`を返す。
//!
//! この crateが決めないこと（User Gate）:
//!
//! - 公開Registry／multi-witnessの正本と運営主体
//! - local self assertionをpublic authorityへ昇格する条件
//! - unknown／untrusted authority objectをcacheまたはserveしてよい条件

#![forbid(unsafe_code)]

use fold_provenance::ManifestSignatureVerification;
use serde::{Deserialize, Serialize};

pub const AUTHORITY_DECISION_SCHEMA: &str = "fold-authority-decision/0";

/// 署名検証済みの公開鍵を指す参照。値そのものが公開情報であり、秘密は含まない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublisherKeyRef {
    pub scheme: String,
    pub public_key_hex: String,
}

/// authority判断の結果状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum AuthorityStatus {
    TrustedForScope,
    Untrusted,
    Unknown,
    Revoked,
}

/// 信頼判断の根拠。将来のresolver追加に備え`#[non_exhaustive]`にする。
///
/// `Unresolved`は「まだ判断できていない」ことを表す唯一の根拠であり、
/// 特定providerのfixture根拠（例: DEVHAGE）を、判断不能な状態へ混ぜない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum TrustBasis {
    Unresolved,
    InsecureDevFixture,
    LocalSelfAsserted,
    RegistryAttested,
    MultiWitness,
}

/// production環境でこの鍵をauthorityとして使ってよいかの判定。
///
/// `Unresolved`はresolverが対象を解決できなかった場合に使う。「開発でしか
/// 使えないと確定した」ことを意味する`DevOnly`とは別状態として扱う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ProductionReadiness {
    Unresolved,
    ProductionEligible,
    DevOnly,
}

/// publisher公開鍵をWorld authorityとして信頼するかどうかを判断するresolver境界。
///
/// 複数のresolverを並存させ、将来`ManifestRegistryResolver`／`MultiWitnessResolver`、
/// およびEDOHAGE-TUBO側のDEVHAGE／local key provider実装を追加できるようにする
/// （この crateでは具体providerを実装しない）。
pub trait PublisherAuthorityResolver {
    fn resolve(
        &self,
        key: &PublisherKeyRef,
        world_id: &str,
        worldline_id: &str,
    ) -> AuthorityDecision;
}

/// authority判断の結果。machine token（`reference`）とGUI表示文を分離し、
/// 表示文はこの crateでは生成しない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorityDecision {
    pub schema: String,
    pub status: AuthorityStatus,
    pub trust_basis: TrustBasis,
    pub world_id: String,
    pub worldline_id: String,
    pub key_id: String,
    pub production_readiness: ProductionReadiness,
    /// 判断根拠を示す安定machine token。GUI表示文はこの crateで生成しない。
    pub reference: String,
}

/// 具体的なauthority providerがまだ配線されていない場合の既定resolver。
///
/// どの鍵に対しても常に`AuthorityStatus::Unknown` / `TrustBasis::Unresolved` /
/// `ProductionReadiness::Unresolved`を返す。この resolverだけでは、いかなる鍵も
/// `TRUSTED_FOR_SCOPE`にならない（fail-closedな既定値）。
#[derive(Debug, Clone, Copy, Default)]
pub struct UnresolvedAuthorityResolver;

impl PublisherAuthorityResolver for UnresolvedAuthorityResolver {
    fn resolve(
        &self,
        key: &PublisherKeyRef,
        world_id: &str,
        worldline_id: &str,
    ) -> AuthorityDecision {
        AuthorityDecision {
            schema: AUTHORITY_DECISION_SCHEMA.to_owned(),
            status: AuthorityStatus::Unknown,
            trust_basis: TrustBasis::Unresolved,
            world_id: world_id.to_owned(),
            worldline_id: worldline_id.to_owned(),
            key_id: key.public_key_hex.clone(),
            production_readiness: ProductionReadiness::Unresolved,
            reference: "NO_AUTHORITY_PROVIDER_CONFIGURED".to_owned(),
        }
    }
}

/// 署名が数学的に有効な場合だけresolverを呼ぶ境界関数。
///
/// `verification.signature_valid == false`のときは`resolver`を一切呼ばずに`None`を返す。
/// これにより「署名が壊れているbytesに対してもauthority判断が動いてしまう」という
/// 混同を防ぐ（MAGI監査 fold-nic#5 issuecomment-5448141382の指摘）。
#[must_use]
pub fn resolve_if_signature_valid(
    verification: &ManifestSignatureVerification,
    world_id: &str,
    worldline_id: &str,
    resolver: &dyn PublisherAuthorityResolver,
) -> Option<AuthorityDecision> {
    if !verification.signature_valid {
        return None;
    }
    let key = PublisherKeyRef {
        scheme: verification.scheme.clone(),
        public_key_hex: verification.public_key_hex.clone(),
    };
    Some(resolver.resolve(&key, world_id, worldline_id))
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorityStatus, ManifestSignatureVerification, ProductionReadiness,
        PublisherAuthorityResolver, PublisherKeyRef, TrustBasis, UnresolvedAuthorityResolver,
        resolve_if_signature_valid,
    };

    struct PanicResolver;

    impl PublisherAuthorityResolver for PanicResolver {
        fn resolve(
            &self,
            _key: &PublisherKeyRef,
            _world_id: &str,
            _worldline_id: &str,
        ) -> super::AuthorityDecision {
            panic!("resolverはsignature_valid=falseで呼ばれてはいけません");
        }
    }

    fn verification(signature_valid: bool) -> ManifestSignatureVerification {
        ManifestSignatureVerification {
            schema: "fold-manifest-signature/0".to_owned(),
            cid: "bafyreitest0000000000000000000000000000000000000000000000000".to_owned(),
            signature_valid,
            scheme: "ed25519-v1".to_owned(),
            public_key_hex: "00".repeat(32),
        }
    }

    #[test]
    fn signature_invalidならresolverを呼ばない() {
        let result = resolve_if_signature_valid(
            &verification(false),
            "fold-nic-forge",
            "stage0",
            &PanicResolver,
        );
        assert!(result.is_none());
    }

    #[test]
    fn signature_validならresolverを呼び結果を返す() {
        let result = resolve_if_signature_valid(
            &verification(true),
            "fold-nic-forge",
            "stage0",
            &UnresolvedAuthorityResolver,
        )
        .expect("resolver must be called");
        assert_eq!(result.status, AuthorityStatus::Unknown);
        assert_eq!(result.trust_basis, TrustBasis::Unresolved);
    }

    #[test]
    fn unresolved_resolverは常にunknownとunresolvedを組で返す() {
        let resolver = UnresolvedAuthorityResolver;
        let key = PublisherKeyRef {
            scheme: "ed25519-v1".to_owned(),
            public_key_hex: "11".repeat(32),
        };
        let result = resolver.resolve(&key, "fold-nic-forge", "stage0");
        assert_eq!(result.status, AuthorityStatus::Unknown);
        assert_eq!(result.trust_basis, TrustBasis::Unresolved);
        assert_eq!(result.production_readiness, ProductionReadiness::Unresolved);
        assert_eq!(result.reference, "NO_AUTHORITY_PROVIDER_CONFIGURED");
    }

    #[test]
    fn decisionはjsonへ往復できる() {
        let resolver = UnresolvedAuthorityResolver;
        let key = PublisherKeyRef {
            scheme: "ed25519-v1".to_owned(),
            public_key_hex: "22".repeat(32),
        };
        let result = resolver.resolve(&key, "fold-nic-forge", "stage0");
        let json = serde_json::to_string(&result).expect("serialize");
        let back: super::AuthorityDecision = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, result);
    }
}
