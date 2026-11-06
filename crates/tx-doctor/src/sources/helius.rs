//! Calls that only Helius serves: getTransactionsForAddress and Parsed Events.

use std::collections::HashMap;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Client, FailedTx, RawTransaction, Source, SourceError};

/// Helius Parsed Events `decodedError`, used as an outside check on our own decode.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DecodedError {
    pub instruction_index: Option<u64>,
    pub program_id: Option<String>,
    pub program_name: Option<String>,
    pub code: Option<u64>,
    pub name: Option<String>,
    pub msg: Option<String>,
}

#[derive(Deserialize)]
struct ParsedEventsItem {
    signature: String,
    parsed: Option<Parsed>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Parsed {
    decoded_error: Option<DecodedError>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ParsedEventsBody {
    List(Vec<ParsedEventsItem>),
    Wrapped { data: Vec<ParsedEventsItem> },
}

impl Client {
    pub(super) fn transactions_for_address(
        &self,
        address: &str,
        limit: usize,
    ) -> Result<Vec<FailedTx>, SourceError> {
        #[derive(Deserialize)]
        struct Page {
            data: Vec<RawTransaction>,
        }
        let page: Page = self.rpc(
            "getTransactionsForAddress",
            json!([address, {
                "transactionDetails": "full",
                "sortOrder": "desc",
                "limit": limit,
                "encoding": "json",
                "maxSupportedTransactionVersion": 1,
                "filters": { "status": "failed" }
            }]),
        )?;
        Ok(page
            .data
            .into_iter()
            .filter(|tx| tx.meta.as_ref().is_some_and(|m| !m.err.is_null()))
            .filter_map(|tx| {
                let signature = tx.transaction.signatures.first()?.clone();
                Some(FailedTx { signature, tx })
            })
            .collect())
    }

    /// `Ok(None)` without a key: the public RPC has no equivalent.
    pub fn parsed_events_errors(
        &self,
        signatures: &[String],
    ) -> Result<Option<HashMap<String, Option<DecodedError>>>, SourceError> {
        let Some(key) = &self.api_key else {
            return Ok(None);
        };
        if signatures.is_empty() {
            return Ok(None);
        }
        let url = Url::parse_with_params(
            &format!("{}/v1/parsed-events/transactions", self.helius_host),
            [("api-key", key)],
        )
        .map_err(|_| self.error("helius parsed events URL is invalid".to_string()))?;
        let body = json!({ "transactions": signatures, "includeRawTransaction": false });
        let items = match self.post::<ParsedEventsBody>(Source::Helius, &url, &body)? {
            ParsedEventsBody::List(items) | ParsedEventsBody::Wrapped { data: items } => items,
        };
        Ok(Some(
            items
                .into_iter()
                .map(|i| (i.signature, i.parsed.and_then(|p| p.decoded_error)))
                .collect(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{client, tx_json, KEY};
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn address_scan_asks_helius_for_failed_transactions_only() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST).path("/").json_body_partial(
                r#"{"method":"getTransactionsForAddress","params":["Addr1",{"transactionDetails":"full","filters":{"status":"failed"}}]}"#,
            );
            then.status(200).json_body(json!({ "jsonrpc": "2.0", "id": 1, "result": { "data": [
                tx_json("bad", json!({ "InstructionError": [0, { "Custom": 1 }] })),
                tx_json("ok", json!(null)),
            ] } }));
        });
        let scan = client(Some(KEY), &server.base_url())
            .failed_transactions("Addr1", 5)
            .unwrap();
        m.assert();
        assert_eq!(scan.method, "getTransactionsForAddress");
        assert_eq!(scan.txs.len(), 1);
        assert_eq!(scan.txs[0].signature, "bad");
        assert!(scan.fallback_reason.is_none());
    }

    #[test]
    fn address_scan_falls_back_to_signatures_when_helius_refuses_the_call() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST)
                .json_body_partial(r#"{"method":"getTransactionsForAddress"}"#);
            then.status(200)
                .json_body(json!({ "jsonrpc": "2.0", "id": 1,
                "error": { "code": -32601, "message": "method not available on this plan" } }));
        });
        server.mock(|when, then| {
            when.method(POST)
                .json_body_partial(r#"{"method":"getSignaturesForAddress"}"#);
            then.status(200)
                .json_body(json!({ "jsonrpc": "2.0", "id": 1, "result": [
                { "signature": "s1", "err": { "InstructionError": [0, { "Custom": 1 }] } },
                { "signature": "s2", "err": null }
            ] }));
        });
        let tx = server.mock(|when, then| {
            when.method(POST)
                .json_body_partial(r#"{"method":"getTransaction","params":["s1"]}"#);
            then.status(200)
                .json_body(json!({ "jsonrpc": "2.0", "id": 1,
                "result": tx_json("s1", json!({ "InstructionError": [0, { "Custom": 1 }] })) }));
        });
        let scan = client(Some(KEY), &server.base_url())
            .failed_transactions("Addr1", 5)
            .unwrap();
        tx.assert_hits(1);
        assert_eq!(scan.method, "getSignaturesForAddress");
        assert_eq!(scan.scanned, 2);
        assert_eq!(scan.txs.len(), 1);
        assert!(scan.fallback_reason.unwrap().contains("-32601"));
    }

    #[test]
    fn parsed_events_returns_decoded_errors_by_signature() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST)
                .path("/v1/parsed-events/transactions")
                .query_param("api-key", KEY)
                .json_body(json!({ "transactions": ["s1"], "includeRawTransaction": false }));
            then.status(200).json_body(json!([{ "signature": "s1", "parserStatus": "ok",
                "parsed": { "decodedError": { "instructionIndex": 0, "code": 6000, "name": "AmountOverLimit" } } }]));
        });
        let map = client(Some(KEY), &server.base_url())
            .parsed_events_errors(&["s1".to_string()])
            .unwrap()
            .unwrap();
        m.assert();
        let e = map["s1"].as_ref().unwrap();
        assert_eq!(e.code, Some(6000));
        assert_eq!(e.name.as_deref(), Some("AmountOverLimit"));
    }

    #[test]
    fn parsed_events_is_skipped_without_a_key() {
        let c = client(None, "http://127.0.0.1:9");
        assert!(c
            .parsed_events_errors(&["s1".to_string()])
            .unwrap()
            .is_none());
    }

    /// Needs HELIUS_API_KEY and the network: `cargo test -- --ignored live_`.
    #[test]
    #[ignore]
    fn live_helius_decodes_a_recorded_fixture_the_same_way() {
        dotenvy::dotenv().ok();
        let c = crate::sources::Client::from_env();
        assert!(c.has_helius_key(), "set HELIUS_API_KEY to run this test");
        let sig = "2B1FXCLwo7U5HeHSxDKZMxKFZcTskUoYn9nmzfk3t9dFsR9u5W26irqf4cghaazQ5qE2ZyoYHJByEyUsjRUdDMYn";
        let tx = c.get_transaction(sig).unwrap().expect("transaction exists");
        let d = crate::diagnose::diagnose(tx.meta.as_ref().unwrap());
        assert_eq!(d.cause, crate::diagnose::Cause::ProgramCustomError);
    }
}
