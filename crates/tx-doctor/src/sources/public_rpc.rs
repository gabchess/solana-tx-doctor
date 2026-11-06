//! The address scan that works on any Solana RPC: recent signatures, keep the
//! failed ones, then fetch each one.

use serde::Deserialize;
use serde_json::{json, Value};

use super::{Client, FailedScan, FailedTx, SourceError};

pub const SIGNATURE_SCAN_LIMIT: usize = 100;

impl Client {
    pub(super) fn scan_signatures(
        &self,
        address: &str,
        limit: usize,
        fallback_reason: Option<String>,
    ) -> Result<FailedScan, SourceError> {
        #[derive(Deserialize)]
        struct SignatureInfo {
            signature: String,
            #[serde(default)]
            err: Value,
        }
        let sigs: Vec<SignatureInfo> = self.rpc(
            "getSignaturesForAddress",
            json!([address, { "limit": SIGNATURE_SCAN_LIMIT }]),
        )?;
        let failed: Vec<&SignatureInfo> = sigs
            .iter()
            .filter(|s| !s.err.is_null())
            .take(limit)
            .collect();
        let mut txs = Vec::new();
        let mut skipped = 0;
        for (i, info) in failed.iter().enumerate() {
            if i > 0 {
                std::thread::sleep(self.scan_pace);
            }
            // One transaction that stays rate limited should not sink the whole scan; count it instead.
            match self.get_transaction(&info.signature) {
                Ok(Some(tx)) if tx.meta.as_ref().is_some_and(|m| !m.err.is_null()) => {
                    txs.push(FailedTx {
                        signature: info.signature.clone(),
                        tx,
                    })
                }
                _ => skipped += 1,
            }
        }
        Ok(FailedScan {
            source: self.source,
            method: "getSignaturesForAddress",
            scanned: sigs.len(),
            skipped,
            txs,
            fallback_reason,
        })
    }
}
