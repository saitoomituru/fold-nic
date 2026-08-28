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

/// 受信bytesの外側に添付するdetached署名。bytes本体は変更しない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetachedManifestSignature {
    pub schema: String,
    pub scheme: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

/// exact bytes検証の結果。
///
/// 数学的な署名検証結果（`signature_valid`）と、検証に使った鍵の参照
/// （`scheme`／`public_key_hex`）だけを保持する。この鍵をWorld authorityとして
/// 信頼してよいかの判断（`AuthorityStatus`／`TrustBasis`）はこの crateの範囲外であり、
/// この型はauthority判断を表すfieldを一切持たない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestSignatureVerification {
    pub schema: String,
    pub cid: String,
    pub signature_valid: bool,
    /// 検証に使った署名scheme。`decode_public_key`が成功した鍵にのみ紐づく。
    pub scheme: String,
    /// 検証に使った公開鍵のcanonical hex（小文字）。入力の大小文字表記は保持しない。
    pub public_key_hex: String,
}

/// 受信した`bytes`そのものに対し、detached署名を検証する。
///
/// canonicalizeや正規化は行わず、受信bytesをそのまま署名検証へ渡す。
/// 返り値はこの公開鍵がWorldのauthorityとして正当かどうかを一切含まない。
///
/// # Errors
///
/// 未対応のscheme、公開鍵または署名のhex encoding不正、長さ不正の場合に返す。
/// 署名不一致自体はErrorにせず、`signature_valid: false`として返す。
pub fn verify_manifest_signature(
    bytes: &[u8],
    signature: &DetachedManifestSignature,
) -> Result<ManifestSignatureVerification, FoldError> {
    if signature.schema != MANIFEST_SIGNATURE_SCHEMA {
        return Err(FoldError::new(
            FoldErrorCode::UnsupportedManifestSchema,
            format!("未対応のmanifest署名schemaです: {}", signature.schema),
        ));
    }
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
        scheme: signature.scheme.clone(),
        public_key_hex: encode_hex(verifying_key.as_bytes()),
    })
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(String::new(), |mut output, byte| {
        let _ = write!(output, "{byte:02x}");
        output
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

/// hex文字列をbyte列へ変換する。
///
/// `value`が非ASCII（多byte UTF-8）を含んでいても、byte単位で走査するため
/// 文字境界外のstring sliceでpanicしない。非ASCII byteは単に不正hex桁として拒否する。
fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    let raw = value.as_bytes();
    if !raw.len().is_multiple_of(2) {
        return Err("hex文字列の長さが奇数です".to_owned());
    }
    raw.as_chunks::<2>()
        .0
        .iter()
        .map(|&[high_byte, low_byte]| {
            let high = hex_nibble(high_byte)?;
            let low = hex_nibble(low_byte)?;
            Ok((high << 4) | low)
        })
        .collect()
}

/// 1 ASCII byteをhex nibbleへ変換する。ASCII以外・非hex文字はErrorにする。
fn hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!(
            "hex文字列を解釈できません: 0x{byte:02x}はhex桁ではありません"
        )),
    }
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
        assert_eq!(verification.scheme, SIGNATURE_SCHEME_ED25519_V1);
        assert_eq!(verification.public_key_hex, signature.public_key_hex);
    }

    #[test]
    fn public_key_hexは大文字入力でも小文字canonical形で返る() {
        let bytes = b"fold-object-manifest-fixture";
        let mut signature = sign(bytes);
        signature.public_key_hex = signature.public_key_hex.to_uppercase();
        signature.signature_hex = signature.signature_hex.to_uppercase();
        let verification = verify_manifest_signature(bytes, &signature).expect("verified");
        assert_eq!(
            verification.public_key_hex,
            signature.public_key_hex.to_lowercase()
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

    #[test]
    fn 未対応schemaをerrorで返す() {
        let bytes = b"fold-object-manifest-fixture";
        let mut signature = sign(bytes);
        signature.schema = "fold-manifest-signature/999".to_owned();
        let error = verify_manifest_signature(bytes, &signature).expect_err("must error");
        assert_eq!(error.code, FoldErrorCode::UnsupportedManifestSchema);
    }

    /// #3: 非ASCII混じりのhex入力でUTF-8境界panicへ到達しないことを固定する負例集。
    #[test]
    fn 非ascii混じりのhex入力はpanicせずerrorになる() {
        let bytes = b"fold-object-manifest-fixture";
        let cases = [
            "あ0",      // 3byte文字 + 1byte、偶数byte長だが文字境界外
            "0あ",      // 先頭1byte + 3byte文字
            "ａｂｃｄ", // 全角英数（hex桁に見えるが非ASCII）
            "🦀🦀",     // 4byte絵文字2個
            "",         // 空文字列
            "a",        // 奇数長
            "gg",       // ASCIIだがhex桁ではない
            "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz", // 非hexだが長さは64
        ];
        for hex_value in cases {
            let mut signature = sign(bytes);
            signature.public_key_hex = hex_value.to_owned();
            let error = verify_manifest_signature(bytes, &signature).expect_err(&format!(
                "public_key_hex={hex_value:?} must error without panic"
            ));
            assert_eq!(error.code, FoldErrorCode::InvalidPublicKey);

            let mut signature = sign(bytes);
            signature.signature_hex = hex_value.to_owned();
            let error = verify_manifest_signature(bytes, &signature).expect_err(&format!(
                "signature_hex={hex_value:?} must error without panic"
            ));
            assert_eq!(error.code, FoldErrorCode::InvalidSignatureEncoding);
        }
    }

    #[test]
    fn hexの大文字小文字混在を正しく受理する() {
        let bytes = b"fold-object-manifest-fixture";
        let mut signature = sign(bytes);
        signature.public_key_hex = signature.public_key_hex.to_uppercase();
        signature.signature_hex = signature.signature_hex.to_uppercase();
        let verification = verify_manifest_signature(bytes, &signature).expect("uppercase hex");
        assert!(verification.signature_valid);
    }
}
