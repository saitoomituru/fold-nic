// SPDX-License-Identifier: AGPL-3.0-or-later

use std::error::Error;
use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

/// CLI、receipt、adapter間で意味を変えずに運べるerror code。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[non_exhaustive]
pub enum FoldErrorCode {
    InvalidWorldId,
    InvalidWorldlineId,
    InvalidLocatorScheme,
    InvalidLocatorValue,
    EmptyHashSet,
    DuplicateHash,
    PreferredHashMissing,
    InvalidParentReference,
    WorldlineMismatch,
    StaleRuleset,
    RoutingError,
    AdapterError,
    ProtocolError,
}

impl FoldErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidWorldId => "INVALID_WORLD_ID",
            Self::InvalidWorldlineId => "INVALID_WORLDLINE_ID",
            Self::InvalidLocatorScheme => "INVALID_LOCATOR_SCHEME",
            Self::InvalidLocatorValue => "INVALID_LOCATOR_VALUE",
            Self::EmptyHashSet => "EMPTY_HASH_SET",
            Self::DuplicateHash => "DUPLICATE_HASH",
            Self::PreferredHashMissing => "PREFERRED_HASH_MISSING",
            Self::InvalidParentReference => "INVALID_PARENT_REFERENCE",
            Self::WorldlineMismatch => "WORLDLINE_MISMATCH",
            Self::StaleRuleset => "STALE_RULESET",
            Self::RoutingError => "ROUTING_ERROR",
            Self::AdapterError => "ADAPTER_ERROR",
            Self::ProtocolError => "PROTOCOL_ERROR",
        }
    }
}

/// 人間向けdetailと機械可読codeを分離したprotocol error。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoldError {
    pub code: FoldErrorCode,
    pub detail: String,
}

impl FoldError {
    #[must_use]
    pub fn new(code: FoldErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

impl Display for FoldError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.detail)
    }
}

impl Error for FoldError {}
