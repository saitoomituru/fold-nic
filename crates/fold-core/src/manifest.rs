// SPDX-License-Identifier: AGPL-3.0-or-later

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{FoldError, FoldErrorCode, FoldLocator, WorldRef};

/// 実digest計算から独立したalgorithm-agile hash参照。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct HashReference {
    pub algorithm: String,
    pub digest: String,
}

impl HashReference {
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}:{}", self.algorithm, self.digest)
    }
}

/// deterministic encodingとdigest計算はCAS層が担当するobject manifest。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoldObjectManifest {
    pub schema: String,
    pub profile_version: String,
    pub world_ref: WorldRef,
    pub source: Option<FoldLocator>,
    pub hash_set: Vec<HashReference>,
    pub preferred_hash: HashReference,
    pub parent_refs: Vec<String>,
}

impl FoldObjectManifest {
    /// hash集合、preferred hash、parent参照の内部整合性を検査する。
    ///
    /// # Errors
    ///
    /// hash集合が空、重複、preferred hash不足、空parent参照の場合に返す。
    pub fn validate(&self) -> Result<(), FoldError> {
        if self.hash_set.is_empty() {
            return Err(FoldError::new(
                FoldErrorCode::EmptyHashSet,
                "object manifestには1件以上のhashが必要です",
            ));
        }

        let mut hashes = BTreeSet::new();
        for hash in &self.hash_set {
            if hash.algorithm.is_empty() || hash.digest.is_empty() {
                return Err(FoldError::new(
                    FoldErrorCode::ProtocolError,
                    "hash algorithmとdigestは空にできません",
                ));
            }
            if !hashes.insert(hash) {
                return Err(FoldError::new(
                    FoldErrorCode::DuplicateHash,
                    "同一hash参照が重複しています",
                ));
            }
        }
        if !hashes.contains(&self.preferred_hash) {
            return Err(FoldError::new(
                FoldErrorCode::PreferredHashMissing,
                "preferred_hashはhash_set内に必要です",
            ));
        }
        if self.parent_refs.iter().any(String::is_empty) {
            return Err(FoldError::new(
                FoldErrorCode::InvalidParentReference,
                "parent_refsに空文字列は指定できません",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorldId, WorldlineId};

    fn world_ref() -> WorldRef {
        WorldRef::new(
            WorldId::parse("archive").expect("valid world"),
            WorldlineId::parse("main-0001").expect("valid worldline"),
        )
    }

    fn hash() -> HashReference {
        HashReference {
            algorithm: "sha2-256".to_owned(),
            digest: "zExample".to_owned(),
        }
    }

    #[test]
    fn preferred_hashが集合内なら受理する() {
        let hash = hash();
        let manifest = FoldObjectManifest {
            schema: "fold-object-manifest/0".to_owned(),
            profile_version: "ipfs-cid-v1".to_owned(),
            world_ref: world_ref(),
            source: None,
            hash_set: vec![hash.clone()],
            preferred_hash: hash,
            parent_refs: Vec::new(),
        };
        manifest.validate().expect("valid manifest");
    }

    #[test]
    fn 集合外preferred_hashを拒否する() {
        let manifest = FoldObjectManifest {
            schema: "fold-object-manifest/0".to_owned(),
            profile_version: "ipfs-cid-v1".to_owned(),
            world_ref: world_ref(),
            source: None,
            hash_set: vec![hash()],
            preferred_hash: HashReference {
                algorithm: "sha3-256".to_owned(),
                digest: "zOther".to_owned(),
            },
            parent_refs: Vec::new(),
        };
        let error = manifest.validate().expect_err("preferred hash must exist");
        assert_eq!(error.code, FoldErrorCode::PreferredHashMissing);
    }
}
