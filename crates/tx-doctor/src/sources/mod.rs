//! Read-only Solana data. Helius when HELIUS_API_KEY is set, the public mainnet RPC
//! otherwise. Errors name the source, never the URL, so the key cannot leak into output.

mod helius;
mod public_rpc;

pub use helius::DecodedError;

use std::fmt;
use std::time::Duration;

use reqwest::blocking::Client as Http;
use reqwest::{StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::diagnose::TxMeta;

pub const HELIUS_HOST: &str = "https://mainnet.helius-rpc.com";
pub const PUBLIC_RPC: &str = "https://api.mainnet-beta.solana.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    Helius,
    PublicRpc,
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Source::Helius => "helius",
            Source::PublicRpc => "public-rpc",
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RawTransaction {
    pub slot: u64,
    #[serde(default)]
    pub block_time: Option<i64>,
    pub meta: Option<TxMeta>,
    pub transaction: TxBody,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TxBody {
    pub signatures: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FailedTx {
    pub signature: String,
    pub tx: RawTransaction,
}

/// Failed transactions for one address, and how they were found.
#[derive(Debug)]
pub struct FailedScan {
    pub source: Source,
    pub method: &'static str,
    pub txs: Vec<FailedTx>,
    pub scanned: usize,
    pub skipped: usize,
    /// Set when the Helius call failed and the signature scan ran instead.
    pub fallback_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceError(pub String);

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SourceError {}

fn tx_config() -> Value {
    json!({ "encoding": "json", "maxSupportedTransactionVersion": 1, "commitment": "finalized" })
}

pub struct Client {
    http: Http,
    source: Source,
    api_key: Option<String>,
    helius_host: String,
    rpc_url: Url,
    retry_delays: Vec<Duration>,
    scan_pace: Duration,
}

impl Client {
    /// Reads HELIUS_API_KEY from the environment (load .env first).
    pub fn from_env() -> Client {
        let key = std::env::var("HELIUS_API_KEY").ok();
        Client::new(key.as_deref(), HELIUS_HOST, PUBLIC_RPC)
    }

    pub fn new(api_key: Option<&str>, helius_host: &str, public_rpc: &str) -> Client {
        let api_key = api_key
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string);
        let helius_host = helius_host.trim_end_matches('/').to_string();
        let (source, rpc_url) = match &api_key {
            Some(key) => (
                Source::Helius,
                Url::parse_with_params(&format!("{helius_host}/"), [("api-key", key)]),
            ),
            None => (Source::PublicRpc, Url::parse(public_rpc)),
        };
        Client {
            http: Http::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("TLS backend available"),
            source,
            api_key,
            helius_host,
            rpc_url: rpc_url.expect("valid RPC host"),
            retry_delays: vec![Duration::from_millis(700)],
            scan_pace: Duration::from_millis(300),
        }
    }

    pub fn source(&self) -> Source {
        self.source
    }

    pub fn has_helius_key(&self) -> bool {
        self.api_key.is_some()
    }

    fn error(&self, message: String) -> SourceError {
        match &self.api_key {
            Some(key) => SourceError(message.replace(key.as_str(), "***")),
            None => SourceError(message),
        }
    }

    fn post<T: DeserializeOwned>(
        &self,
        source: Source,
        url: &Url,
        body: &Value,
    ) -> Result<T, SourceError> {
        let mut delays = self.retry_delays.iter();
        loop {
            let res =
                self.http.post(url.clone()).json(body).send().map_err(|e| {
                    self.error(format!("{source} request failed: {}", e.without_url()))
                })?;
            // The public RPC answers 429 when a client sends many calls in a row; back off and retry.
            if res.status() == StatusCode::TOO_MANY_REQUESTS {
                if let Some(delay) = delays.next() {
                    std::thread::sleep(*delay);
                    continue;
                }
            }
            if !res.status().is_success() {
                return Err(self.error(format!(
                    "{source} request failed with HTTP {}",
                    res.status().as_u16()
                )));
            }
            return res.json::<T>().map_err(|e| {
                self.error(format!(
                    "{source} sent a response we could not read: {}",
                    e.without_url()
                ))
            });
        }
    }

    /// `Ok(None)` when the call succeeded with a null result.
    fn rpc_optional<T: DeserializeOwned>(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Option<T>, SourceError> {
        #[derive(Deserialize)]
        struct Envelope<T> {
            result: Option<T>,
            error: Option<RpcError>,
        }
        #[derive(Deserialize)]
        struct RpcError {
            code: i64,
            message: String,
        }
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let env: Envelope<T> = self.post(self.source, &self.rpc_url, &body)?;
        if let Some(e) = env.error {
            return Err(self.error(format!(
                "{} {method} error {}: {}",
                self.source, e.code, e.message
            )));
        }
        Ok(env.result)
    }

    fn rpc<T: DeserializeOwned>(&self, method: &str, params: Value) -> Result<T, SourceError> {
        self.rpc_optional(method, params)?
            .ok_or_else(|| self.error(format!("{} {method} returned no result", self.source)))
    }

    /// `Ok(None)` when the node has no such transaction.
    pub fn get_transaction(&self, signature: &str) -> Result<Option<RawTransaction>, SourceError> {
        self.rpc_optional("getTransaction", json!([signature, tx_config()]))
    }

    /// Failed transactions that touched an address, newest first.
    /// Helius: one getTransactionsForAddress call with the status filter, and the
    /// signature scan if that call errors. Public RPC: the signature scan.
    pub fn failed_transactions(
        &self,
        address: &str,
        limit: usize,
    ) -> Result<FailedScan, SourceError> {
        if self.source == Source::Helius {
            match self.transactions_for_address(address, limit) {
                Ok(txs) => {
                    return Ok(FailedScan {
                        source: self.source,
                        method: "getTransactionsForAddress",
                        scanned: txs.len(),
                        skipped: 0,
                        txs,
                        fallback_reason: None,
                    })
                }
                Err(e) => return self.scan_signatures(address, limit, Some(e.0)),
            }
        }
        self.scan_signatures(address, limit, None)
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::Client;
    use std::time::Duration;

    pub const KEY: &str = "test-key-0000";

    pub fn client(key: Option<&str>, base: &str) -> Client {
        let mut c = Client::new(key, base, &format!("{base}/public"));
        c.retry_delays = vec![Duration::ZERO];
        c.scan_pace = Duration::ZERO;
        c
    }

    pub fn tx_json(signature: &str, err: serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "slot": 1,
            "blockTime": null,
            "meta": { "err": err, "fee": 5000, "logMessages": [] },
            "transaction": { "signatures": [signature], "message": {} }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{client, tx_json, KEY};
    use super::Source;
    use httpmock::prelude::*;
    use serde_json::json;

    #[test]
    fn with_a_key_get_transaction_goes_to_helius_with_the_grimoire_config() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST).path("/").query_param("api-key", KEY).json_body_partial(
                r#"{"method":"getTransaction","params":["sig1",{"encoding":"json","maxSupportedTransactionVersion":1,"commitment":"finalized"}]}"#,
            );
            then.status(200)
                .json_body(json!({ "jsonrpc": "2.0", "id": 1, "result": tx_json("sig1", json!(null)) }));
        });
        let c = client(Some(KEY), &server.base_url());
        assert_eq!(c.source(), Source::Helius);
        let tx = c.get_transaction("sig1").unwrap().unwrap();
        assert_eq!(tx.transaction.signatures[0], "sig1");
        m.assert();
    }

    #[test]
    fn without_a_key_get_transaction_goes_to_the_public_rpc() {
        let server = MockServer::start();
        let m = server.mock(|when, then| {
            when.method(POST).path("/public");
            then.status(200)
                .json_body(json!({ "jsonrpc": "2.0", "id": 1, "result": null }));
        });
        let c = client(None, &server.base_url());
        assert_eq!(c.source(), Source::PublicRpc);
        assert!(c.get_transaction("missing").unwrap().is_none());
        m.assert();
    }

    #[test]
    fn errors_name_the_source_and_never_the_key_or_url() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(401).body(format!("bad key {KEY}"));
        });
        let c = client(Some(KEY), &server.base_url());
        let err = c.get_transaction("sig1").unwrap_err().to_string();
        assert_eq!(err, "helius request failed with HTTP 401");
        assert!(!err.contains(KEY));
        assert!(!err.contains(&server.base_url()));
    }

    #[test]
    fn rpc_error_messages_have_the_key_redacted() {
        let server = MockServer::start();
        server.mock(|when, then| {
            when.method(POST);
            then.status(200).json_body(json!({
                "jsonrpc": "2.0", "id": 1,
                "error": { "code": -32600, "message": format!("key {KEY} not allowed") }
            }));
        });
        let err = client(Some(KEY), &server.base_url())
            .get_transaction("sig1")
            .unwrap_err()
            .to_string();
        assert!(!err.contains(KEY), "{err}");
        assert!(err.contains("helius getTransaction error -32600"), "{err}");
    }

    #[test]
    fn a_429_is_retried_once() {
        let server = MockServer::start();
        let always_429 = server.mock(|when, then| {
            when.method(POST);
            then.status(429);
        });
        let err = client(None, &server.base_url())
            .get_transaction("s")
            .unwrap_err();
        assert_eq!(err.0, "public-rpc request failed with HTTP 429");
        always_429.assert_hits(2);
    }
}
