// SPDX-License-Identifier: AGPL-3.0-or-later

//! Fold NIC runtime間で共有する、I/Oを行わないprotocol基本型。

#![forbid(unsafe_code)]

mod error;
mod locator;
mod manifest;
mod receipt;
mod world;

pub use error::{FoldError, FoldErrorCode};
pub use locator::FoldLocator;
pub use manifest::{FoldObjectManifest, HashReference};
pub use receipt::{CacheResult, FoldReceipt, ReceiptAction};
pub use world::{WorldId, WorldRef, WorldlineId};
