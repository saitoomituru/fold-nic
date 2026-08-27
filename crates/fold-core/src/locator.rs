// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::{FoldError, FoldErrorCode};

/// onion、GNS、CID、HTTPS等を同一型で運びつつ、authorityを同一視しないlocator。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FoldLocator {
    pub scheme: String,
    pub value: String,
}

impl FoldLocator {
    /// transport schemeと値からlocatorを作る。
    ///
    /// # Errors
    ///
    /// schemeまたはvalueが長さ・文字境界を満たさない場合に返す。
    pub fn new(scheme: impl Into<String>, value: impl Into<String>) -> Result<Self, FoldError> {
        let scheme = scheme.into();
        if scheme.is_empty()
            || scheme.len() > 32
            || !scheme
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(FoldError::new(
                FoldErrorCode::InvalidLocatorScheme,
                "locator schemeは小文字ASCII、数字、'-'の1〜32 byteである必要があります",
            ));
        }
        let value = value.into();
        if value.is_empty() || value.len() > 4096 || value.contains('\0') {
            return Err(FoldError::new(
                FoldErrorCode::InvalidLocatorValue,
                "locator valueはNULを含まない1〜4096 byteである必要があります",
            ));
        }
        Ok(Self { scheme, value })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport固有locatorを保持する() {
        let locator = FoldLocator::new("onion-v3", "example.onion").expect("valid locator");
        assert_eq!(locator.scheme, "onion-v3");
    }

    #[test]
    fn 不明な大文字schemeを拒否する() {
        let error = FoldLocator::new("HTTPS", "https://example.test").expect_err("invalid scheme");
        assert_eq!(error.code, FoldErrorCode::InvalidLocatorScheme);
    }
}
