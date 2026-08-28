// SPDX-License-Identifier: AGPL-3.0-or-later

//! publisher署名の数学的検証（`fold-provenance`）と、その公開鍵をWorld authorityとして
//! 信頼するかどうかの判断を分離する境界。
//!
//! この crateが固定する不変:
//!
//! - `signature_valid == true`を`authority trusted`へ自動昇格しない
//! - `DEVHAGE`（秘密鍵まで公開済みのTEST fixture）は`loopback` + `dev profile` +
//!   明示acknowledgementの3条件が全て揃わない限り`TRUSTED_FOR_SCOPE`を返さない
//! - production profileでは`DEVHAGE`の鍵指紋を無条件で拒否する（acknowledgementでも覆せない）
//!
//! この crateが決めないこと（次checkpoint／User Gate）:
//!
//! - `LocalSelfAssertedResolver`によるlocal鍵生成・binding（次checkpoint）
//! - 公開Registry／multi-witnessの正本と運営主体（User Gate）
//! - local self assertionをpublic authorityへ昇格する条件（User Gate）

#![forbid(unsafe_code)]

use fold_provenance::SIGNATURE_SCHEME_ED25519_V1;
use serde::{Deserialize, Serialize};

pub const AUTHORITY_DECISION_SCHEMA: &str = "fold-authority-decision/0";

/// DEVHAGE公開鍵（ed25519、hex）。対応する秘密鍵`DEVHAGE_PRIVATE_KEY_SEED_HEX`と共に、
/// 既にrepository内で公開済みの`TEST fixture`である（`SECRET`ではない）。
pub const DEVHAGE_PUBLIC_KEY_HEX: &str =
    "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c";

/// DEVHAGE秘密鍵の`ed25519` seed（hex、32byte、全byte`0x07`）。
///
/// [PUBLIC-TEST-FIXTURE] このseedは`fold-provenance`の既存unit testで使われていた
/// `[7u8; 32]`と同一であり、既に秘密値ではない。loopback + dev profile +
/// 明示acknowledgementの3条件下でのみ`InsecureDevFixtureResolver`が信頼する。
/// production経路では鍵指紋（`DEVHAGE_PUBLIC_KEY_HEX`）そのものを拒否する。
pub const DEVHAGE_PRIVATE_KEY_SEED_HEX: &str =
    "0707070707070707070707070707070707070707070707070707070707070707";

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum TrustBasis {
    InsecureDevFixture,
    LocalSelfAsserted,
    RegistryAttested,
    MultiWitness,
}

/// production環境でこの鍵をauthorityとして使ってよいかの判定。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum ProductionReadiness {
    ProductionEligible,
    DevOnly,
}

/// runtimeが現在どのprofileで動作しているかの申告。呼び出し側が明示的に渡す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RuntimeProfile {
    DevLocalInsecure,
    Production,
}

/// 呼び出し側のnetwork公開範囲の申告。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum NetworkScope {
    LoopbackOnly,
    Public,
}

/// DEVHAGE fixtureを使ってよいかどうかを判断するための呼び出し側context。
///
/// 3条件（`profile == DevLocalInsecure`、`network_scope == LoopbackOnly`、
/// `explicit_acknowledgement == true`）が全て揃わない限り、
/// [`InsecureDevFixtureResolver`]は`TRUSTED_FOR_SCOPE`を返さない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevFixtureUsageContext {
    pub profile: RuntimeProfile,
    pub network_scope: NetworkScope,
    pub explicit_acknowledgement: bool,
}

/// publisher公開鍵をWorld authorityとして信頼するかどうかを判断するresolver境界。
///
/// 複数のresolverを並存させ、将来`ManifestRegistryResolver`／`MultiWitnessResolver`を
/// 追加できるようにする（この crateでは未実装）。
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

/// `DEVHAGE`公開鍵束だけを扱うbootstrap resolver。
///
/// 実際のRegistry・multi-witnessは実装せず、既知のDEVHAGE鍵指紋を、
/// 呼び出し側が明示したcontextに基づいて`TRUSTED_FOR_SCOPE`または`UNTRUSTED`へ解決する。
/// DEVHAGE以外の鍵は`UNKNOWN`（この resolverの範囲外）を返す。
pub struct InsecureDevFixtureResolver {
    context: DevFixtureUsageContext,
}

impl InsecureDevFixtureResolver {
    #[must_use]
    pub const fn new(context: DevFixtureUsageContext) -> Self {
        Self { context }
    }
}

fn is_devhage_key(key: &PublisherKeyRef) -> bool {
    key.scheme == SIGNATURE_SCHEME_ED25519_V1
        && key
            .public_key_hex
            .eq_ignore_ascii_case(DEVHAGE_PUBLIC_KEY_HEX)
}

fn decision(
    status: AuthorityStatus,
    world_id: &str,
    worldline_id: &str,
    key: &PublisherKeyRef,
    production_readiness: ProductionReadiness,
    reference: &'static str,
) -> AuthorityDecision {
    AuthorityDecision {
        schema: AUTHORITY_DECISION_SCHEMA.to_owned(),
        status,
        trust_basis: TrustBasis::InsecureDevFixture,
        world_id: world_id.to_owned(),
        worldline_id: worldline_id.to_owned(),
        key_id: key.public_key_hex.clone(),
        production_readiness,
        reference: reference.to_owned(),
    }
}

impl PublisherAuthorityResolver for InsecureDevFixtureResolver {
    fn resolve(
        &self,
        key: &PublisherKeyRef,
        world_id: &str,
        worldline_id: &str,
    ) -> AuthorityDecision {
        if !is_devhage_key(key) {
            return decision(
                AuthorityStatus::Unknown,
                world_id,
                worldline_id,
                key,
                ProductionReadiness::DevOnly,
                "NOT_DEVHAGE_KEY",
            );
        }

        if matches!(self.context.profile, RuntimeProfile::Production) {
            return decision(
                AuthorityStatus::Untrusted,
                world_id,
                worldline_id,
                key,
                ProductionReadiness::DevOnly,
                "HAGE_PRODUCTION_USING_DEV_KEY",
            );
        }

        let loopback_only = matches!(self.context.network_scope, NetworkScope::LoopbackOnly);
        if !(loopback_only && self.context.explicit_acknowledgement) {
            return decision(
                AuthorityStatus::Untrusted,
                world_id,
                worldline_id,
                key,
                ProductionReadiness::DevOnly,
                "DEVHAGE_USAGE_CONDITIONS_NOT_MET",
            );
        }

        decision(
            AuthorityStatus::TrustedForScope,
            world_id,
            worldline_id,
            key,
            ProductionReadiness::DevOnly,
            "HAGE_BOOTSTRAP_KEY_ACTIVE",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AuthorityStatus, DEVHAGE_PUBLIC_KEY_HEX, DevFixtureUsageContext,
        InsecureDevFixtureResolver, NetworkScope, PublisherAuthorityResolver, PublisherKeyRef,
        RuntimeProfile, SIGNATURE_SCHEME_ED25519_V1,
    };

    fn devhage_key() -> PublisherKeyRef {
        PublisherKeyRef {
            scheme: SIGNATURE_SCHEME_ED25519_V1.to_owned(),
            public_key_hex: DEVHAGE_PUBLIC_KEY_HEX.to_owned(),
        }
    }

    fn other_key() -> PublisherKeyRef {
        PublisherKeyRef {
            scheme: SIGNATURE_SCHEME_ED25519_V1.to_owned(),
            public_key_hex: "00".repeat(32),
        }
    }

    #[test]
    fn 三条件が揃えばtrusted_for_scopeを返す() {
        let resolver = InsecureDevFixtureResolver::new(DevFixtureUsageContext {
            profile: RuntimeProfile::DevLocalInsecure,
            network_scope: NetworkScope::LoopbackOnly,
            explicit_acknowledgement: true,
        });
        let result = resolver.resolve(&devhage_key(), "fold-nic-forge", "stage0");
        assert_eq!(result.status, AuthorityStatus::TrustedForScope);
        assert_eq!(result.reference, "HAGE_BOOTSTRAP_KEY_ACTIVE");
    }

    #[test]
    fn acknowledgementなしはuntrustedを返す() {
        let resolver = InsecureDevFixtureResolver::new(DevFixtureUsageContext {
            profile: RuntimeProfile::DevLocalInsecure,
            network_scope: NetworkScope::LoopbackOnly,
            explicit_acknowledgement: false,
        });
        let result = resolver.resolve(&devhage_key(), "fold-nic-forge", "stage0");
        assert_eq!(result.status, AuthorityStatus::Untrusted);
        assert_eq!(result.reference, "DEVHAGE_USAGE_CONDITIONS_NOT_MET");
    }

    #[test]
    fn public_network_scopeはuntrustedを返す() {
        let resolver = InsecureDevFixtureResolver::new(DevFixtureUsageContext {
            profile: RuntimeProfile::DevLocalInsecure,
            network_scope: NetworkScope::Public,
            explicit_acknowledgement: true,
        });
        let result = resolver.resolve(&devhage_key(), "fold-nic-forge", "stage0");
        assert_eq!(result.status, AuthorityStatus::Untrusted);
        assert_eq!(result.reference, "DEVHAGE_USAGE_CONDITIONS_NOT_MET");
    }

    #[test]
    fn production_profileはacknowledgementがあっても拒否する() {
        let resolver = InsecureDevFixtureResolver::new(DevFixtureUsageContext {
            profile: RuntimeProfile::Production,
            network_scope: NetworkScope::LoopbackOnly,
            explicit_acknowledgement: true,
        });
        let result = resolver.resolve(&devhage_key(), "fold-nic-forge", "stage0");
        assert_eq!(result.status, AuthorityStatus::Untrusted);
        assert_eq!(result.reference, "HAGE_PRODUCTION_USING_DEV_KEY");
    }

    #[test]
    fn devhage以外の鍵はunknownを返す() {
        let resolver = InsecureDevFixtureResolver::new(DevFixtureUsageContext {
            profile: RuntimeProfile::DevLocalInsecure,
            network_scope: NetworkScope::LoopbackOnly,
            explicit_acknowledgement: true,
        });
        let result = resolver.resolve(&other_key(), "fold-nic-forge", "stage0");
        assert_eq!(result.status, AuthorityStatus::Unknown);
        assert_eq!(result.reference, "NOT_DEVHAGE_KEY");
    }

    #[test]
    fn devhage公開鍵はseedから導出した値と一致する() {
        use ed25519_dalek::SigningKey;

        let seed_bytes: Vec<u8> = (0..super::DEVHAGE_PRIVATE_KEY_SEED_HEX.len())
            .step_by(2)
            .map(|index| {
                u8::from_str_radix(&super::DEVHAGE_PRIVATE_KEY_SEED_HEX[index..index + 2], 16)
                    .expect("valid hex fixture")
            })
            .collect();
        let seed: [u8; 32] = seed_bytes.try_into().expect("32byte seed");
        let signing_key = SigningKey::from_bytes(&seed);
        let derived_hex = signing_key.verifying_key().as_bytes().iter().fold(
            String::new(),
            |mut output, byte| {
                use std::fmt::Write as _;
                let _ = write!(output, "{byte:02x}");
                output
            },
        );
        assert_eq!(derived_hex, DEVHAGE_PUBLIC_KEY_HEX);
    }

    #[test]
    fn decisionはjsonへ往復できる() {
        let resolver = InsecureDevFixtureResolver::new(DevFixtureUsageContext {
            profile: RuntimeProfile::DevLocalInsecure,
            network_scope: NetworkScope::LoopbackOnly,
            explicit_acknowledgement: true,
        });
        let result = resolver.resolve(&devhage_key(), "fold-nic-forge", "stage0");
        let json = serde_json::to_string(&result).expect("serialize");
        let back: super::AuthorityDecision = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, result);
    }
}
