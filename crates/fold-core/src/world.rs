// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::{FoldError, FoldErrorCode};

const MAX_ID_BYTES: usize = 128;

fn is_safe_id(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_ID_BYTES
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(byte))
}

/// 表示名ではなくroutingとpartitionに使うstable World ID。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorldId(String);

impl WorldId {
    /// machine routing用のstable World IDを検査して作る。
    ///
    /// # Errors
    ///
    /// 小文字ASCII中心の`stable ID`規約を満たさない場合に返す。
    pub fn parse(value: impl Into<String>) -> Result<Self, FoldError> {
        let value = value.into();
        if !is_safe_id(&value) {
            return Err(FoldError::new(
                FoldErrorCode::InvalidWorldId,
                "world_idは小文字ASCII、数字、'.'、'_'、'-'の1〜128 byteで、英小文字から開始する必要があります",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for WorldId {
    type Error = FoldError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<WorldId> for String {
    fn from(value: WorldId) -> Self {
        value.0
    }
}

/// 同一World内のbranch/versionを表すstable ID。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct WorldlineId(String);

impl WorldlineId {
    /// machine routing用のstable Worldline IDを検査して作る。
    ///
    /// # Errors
    ///
    /// 小文字ASCII中心の`stable ID`規約を満たさない場合に返す。
    pub fn parse(value: impl Into<String>) -> Result<Self, FoldError> {
        let value = value.into();
        if !is_safe_id(&value) {
            return Err(FoldError::new(
                FoldErrorCode::InvalidWorldlineId,
                "worldline_idは小文字ASCII、数字、'.'、'_'、'-'の1〜128 byteで、英小文字から開始する必要があります",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for WorldlineId {
    type Error = FoldError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(value)
    }
}

impl From<WorldlineId> for String {
    fn from(value: WorldlineId) -> Self {
        value.0
    }
}

/// 論理Worldと世界線をwire hostnameから分離して保持する。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WorldRef {
    pub world_id: WorldId,
    pub worldline_id: WorldlineId,
}

impl WorldRef {
    #[must_use]
    pub const fn new(world_id: WorldId, worldline_id: WorldlineId) -> Self {
        Self {
            world_id,
            worldline_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_world_refを受理する() {
        let world = WorldId::parse("construction").expect("valid world");
        let worldline = WorldlineId::parse("branch-0002").expect("valid worldline");
        assert_eq!(
            WorldRef::new(world, worldline).world_id.as_str(),
            "construction"
        );
    }

    #[test]
    fn logical_separatorをidへ混入させない() {
        let error = WorldId::parse("construction:0002").expect_err("colon must be rejected");
        assert_eq!(error.code, FoldErrorCode::InvalidWorldId);
    }

    #[test]
    fn display向けunicodeをstable_idへ混ぜない() {
        let error = WorldId::parse("建設").expect_err("display label must be separate");
        assert_eq!(error.code, FoldErrorCode::InvalidWorldId);
    }
}
