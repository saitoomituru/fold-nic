// SPDX-License-Identifier: AGPL-3.0-or-later

//! 受信manifestの`exact bytes`に対するdetached署名検証。
//!
//! この crateは次を検査しない。
//!
//! - publisher公開鍵をどのWorld authorityへbindingするか（別Registry契約）
//! - canonical encoding（正規化は行わず、受信bytesを一切変更しない）
//!
//! 署名対象は常に受信した生byte列そのものであり、canonicalizeやreformatを
//! 検証前に行わない。CIDは`fold_store::cid_for`で再計算した値を返すだけで、
//! 呼び出し側が期待するhashと一致するかは呼び出し側の責務とする。

#![forbid(unsafe_code)]

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use fold_core::{FoldError, FoldErrorCode};
use fold_store::cid_for;
use serde::{Deserialize, Serialize};

pub const SIGNATURE_SCHEME_ED25519_V1: &str = "ed25519-v1";
pub const MANIFEST_SIGNATURE_SCHEMA: &str = "fold-manifest-signature/0";

/// publisher鍵とWorld authorityのbindingが未確定であることを示す固定値。
///
/// Registry契約が実装されるまで、この crateの検証結果には常にこの値を積む。
pub const PUBLISHER_AUTHORITY_UNVERIFIED: &str = "PUBLISHER_AUTHORITY_UNVERIFIED";

/// 受信bytesの外側に添付するdetached署名。bytes本体は変更しない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetachedManifestSignature {
    pub schema: String,
    pub scheme: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

/// exact bytes検証の結果。authority bindingの判断は含まない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSignatureVerification {
    pub schema: String,
    pub cid: String,
    pub signature_valid: bool,
    pub publisher_authority: String,
}

/// 受信した`bytes`そのものに対し、detached署名を検証する。
///
/// canonicalizeや正規化は行わず、受信bytesをそのまま署名検証へ渡す。
/// `publisher_authority`は常に[`PUBLISHER_AUTHORITY_UNVERIFIED`]を返し、
/// この公開鍵がWorldのauthorityとして正当かどうかは判断しない。
///
/// # Errors
///
/// 未対応のscheme、公開鍵または署名のhex encoding不正、長さ不正の場合に返す。
/// 署名不一致自体はErrorにせず、`signature_valid: false`として返す。
pub fn verify_manifest_signature(
    bytes: &[u8],
    signature: &DetachedManifestSignature,
) -> Result<ManifestSignatureVerification, FoldError> {
    if signature.scheme != SIGNATURE_SCHEME_ED25519_V1 {
        return Err(FoldError::new(
            FoldErrorCode::UnsupportedSignatureScheme,
            format!("未対応の署名schemeです: {}", signature.scheme),
        ));
    }

    let verifying_key = decode_public_key(&signature.public_key_hex)?;
    let parsed_signature = decode_signature(&signature.signature_hex)?;

    let signature_valid = verifying_key.verify(bytes, &parsed_signature).is_ok();

    Ok(ManifestSignatureVerification {
        schema: MANIFEST_SIGNATURE_SCHEMA.to_owned(),
        cid: cid_for(bytes).to_string(),
        signature_valid,
        publisher_authority: PUBLISHER_AUTHORITY_UNVERIFIED.to_owned(),
    })
}

fn decode_public_key(hex_value: &str) -> Result<VerifyingKey, FoldError> {
    let bytes = decode_hex(hex_value)
        .map_err(|detail| FoldError::new(FoldErrorCode::InvalidPublicKey, detail))?;
    let array: [u8; 32] = bytes.try_into().map_err(|_| {
        FoldError::new(
            FoldErrorCode::InvalidPublicKey,
            "ed25519公開鍵は32byteである必要があります",
        )
    })?;
    VerifyingKey::from_bytes(&array)
        .map_err(|error| FoldError::new(FoldErrorCode::InvalidPublicKey, error.to_string()))
}

fn decode_signature(hex_value: &str) -> Result<Signature, FoldError> {
    let bytes = decode_hex(hex_value)
        .map_err(|detail| FoldError::new(FoldErrorCode::InvalidSignatureEncoding, detail))?;
    let array: [u8; 64] = bytes.try_into().map_err(|_| {
        FoldError::new(
            FoldErrorCode::InvalidSignatureEncoding,
            "ed25519署名は64byteである必要があります",
        )
    })?;
    Ok(Signature::from_bytes(&array))
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("hex文字列の長さが奇数です".to_owned());
    }
    (0..value.len())
        .step_by(2)
        .map(|index| {
            u8::from_str_radix(&value[index..index + 2], 16)
                .map_err(|_| format!("hex文字列を解釈できません: {}", &value[index..index + 2]))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;

    fn encode_hex(bytes: &[u8]) -> String {
        use std::fmt::Write as _;

        bytes.iter().fold(String::new(), |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        })
    }

    fn fixed_signing_key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn sign(bytes: &[u8]) -> DetachedManifestSignature {
        let signing_key = fixed_signing_key();
        let signature = signing_key.sign(bytes);
        DetachedManifestSignature {
            schema: MANIFEST_SIGNATURE_SCHEMA.to_owned(),
            scheme: SIGNATURE_SCHEME_ED25519_V1.to_owned(),
            public_key_hex: encode_hex(signing_key.verifying_key().as_bytes()),
            signature_hex: encode_hex(&signature.to_bytes()),
        }
    }

    #[test]
    fn 正しい署名とexact_bytesを受理する() {
        let bytes = b"fold-object-manifest-fixture";
        let signature = sign(bytes);
        let verification = verify_manifest_signature(bytes, &signature).expect("verified");
        assert!(verification.signature_valid);
        assert_eq!(verification.cid, cid_for(bytes).to_string());
        assert_eq!(
            verification.publisher_authority,
            PUBLISHER_AUTHORITY_UNVERIFIED
        );
    }

    #[test]
    fn 一byteでも改変されたbytesを拒否する() {
        let signature = sign(b"fold-object-manifest-fixture");
        let verification = verify_manifest_signature(b"fold-object-manifest-fixturE", &signature)
            .expect("verification runs");
        assert!(!verification.signature_valid);
    }

    #[test]
    fn 未対応schemeをerrorで返す() {
        let mut signature = sign(b"fold-object-manifest-fixture");
        signature.scheme = "secp256k1-v1".to_owned();
        let error = verify_manifest_signature(b"fold-object-manifest-fixture", &signature)
            .expect_err("unsupported scheme must error");
        assert_eq!(error.code, FoldErrorCode::UnsupportedSignatureScheme);
    }

    #[test]
    fn 不正な公開鍵長をerrorで返す() {
        let mut signature = sign(b"fold-object-manifest-fixture");
        signature.public_key_hex = "ab".to_owned();
        let error = verify_manifest_signature(b"fold-object-manifest-fixture", &signature)
            .expect_err("short key must error");
        assert_eq!(error.code, FoldErrorCode::InvalidPublicKey);
    }

    #[test]
    fn 不正な署名長をerrorで返す() {
        let mut signature = sign(b"fold-object-manifest-fixture");
        signature.signature_hex = "ab".to_owned();
        let error = verify_manifest_signature(b"fold-object-manifest-fixture", &signature)
            .expect_err("short signature must error");
        assert_eq!(error.code, FoldErrorCode::InvalidSignatureEncoding);
    }

    #[test]
    fn 別鍵の署名を拒否する() {
        let bytes = b"fold-object-manifest-fixture";
        let mut signature = sign(bytes);
        let other_key = SigningKey::from_bytes(&[9u8; 32]);
        signature.public_key_hex = encode_hex(other_key.verifying_key().as_bytes());
        let verification = verify_manifest_signature(bytes, &signature).expect("verification runs");
        assert!(!verification.signature_valid);
    }
}
