// SPDX-License-Identifier: AGPL-3.0-or-later

use serde::{Deserialize, Serialize};

use crate::{FoldLocator, HashReference, WorldRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CacheResult {
    Miss,
    Local,
    Peer,
    Origin,
    Bypassed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReceiptAction {
    Serve,
    StoreLocal,
    Share,
    BypassSharedCache,
    Quarantine,
    Block,
    Ask,
    Error,
}

/// 観測事実を保持し、runtime完成や安全性保証へ昇格しないreceipt。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoldReceipt {
    pub schema: String,
    pub receipt_id: String,
    pub world_ref: WorldRef,
    pub requested_locator: FoldLocator,
    pub object_hash: Option<HashReference>,
    pub route_table_revision: String,
    pub cache_result: CacheResult,
    pub final_action: ReceiptAction,
    pub observed_at: String,
    pub clock_source: String,
    pub clock_calibration: String,
    pub source_refs: Vec<String>,
    pub unknowns: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WorldId, WorldlineId};

    #[test]
    fn receiptはunknownを落とさずjson化できる() {
        let receipt = FoldReceipt {
            schema: "fold-receipt/0".to_owned(),
            receipt_id: "local-test-1".to_owned(),
            world_ref: WorldRef::new(
                WorldId::parse("fold-nic-forge").expect("valid world"),
                WorldlineId::parse("stage0").expect("valid worldline"),
            ),
            requested_locator: FoldLocator::new("https", "https://example.test")
                .expect("valid locator"),
            object_hash: None,
            route_table_revision: "unbound".to_owned(),
            cache_result: CacheResult::Miss,
            final_action: ReceiptAction::Error,
            observed_at: "2026-08-27T00:00:00+09:00".to_owned(),
            clock_source: "fixture".to_owned(),
            clock_calibration: "unverified".to_owned(),
            source_refs: Vec::new(),
            unknowns: vec!["runtime-not-implemented".to_owned()],
        };
        let json = serde_json::to_string(&receipt).expect("serializable receipt");
        assert!(json.contains("runtime-not-implemented"));
    }
}
