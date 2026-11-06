use std::fmt::Write as _;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use serde::Serialize;
use serde_json::Value;

use crate::diagnose::{diagnose, Diagnosis, TxMeta};
use crate::sources::{Client, DecodedError, RawTransaction, Source};

#[derive(Parser)]
#[command(
    name = "tx-doctor",
    version,
    about = "Explain why a Solana transaction failed"
)]
pub struct Cli {
    /// Print JSON instead of text.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Diagnose one transaction by signature.
    Tx { signature: String },
    /// Diagnose recent failed transactions that touched an address.
    Address {
        address: String,
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Diagnose a saved getTransaction result or fixture file, offline.
    File { path: PathBuf },
}

#[derive(Serialize)]
pub struct Report {
    pub signature: Option<String>,
    pub source: String,
    pub diagnosis: Diagnosis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub helius_parsed_events: Option<CrossCheck>,
}

#[derive(Serialize)]
pub struct CrossCheck {
    pub agrees: Option<bool>,
    pub decoded_error: Option<DecodedError>,
    pub error: Option<String>,
}

pub fn run(cli: Cli) -> Result<String, String> {
    match cli.command {
        Command::File { path } => {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
            let value: Value = serde_json::from_str(&text)
                .map_err(|e| format!("{} is not JSON: {e}", path.display()))?;
            let report = report_from_file(&value)?;
            Ok(render(&[report], cli.json))
        }
        Command::Tx { signature } => {
            let client = Client::from_env();
            let tx = client
                .get_transaction(&signature)
                .map_err(|e| e.to_string())?
                .ok_or_else(|| format!("{} has no transaction {signature}", client.source()))?;
            let mut report = report_from_tx(&tx, Some(signature.clone()), source_label(&client))?;
            report.helius_parsed_events = cross_check(&client, &signature, &report.diagnosis);
            Ok(render(&[report], cli.json))
        }
        Command::Address { address, limit } => {
            let client = Client::from_env();
            let scan = client
                .failed_transactions(&address, limit)
                .map_err(|e| e.to_string())?;
            let reports = scan
                .txs
                .iter()
                .map(|f| report_from_tx(&f.tx, Some(f.signature.clone()), source_label(&client)))
                .collect::<Result<Vec<_>, _>>()?;
            if cli.json {
                return Ok(render(&reports, true));
            }
            let mut out = format!(
                "{} failed transactions for {address} via {} {} (scanned {}, skipped {})\n",
                reports.len(),
                source_label(&client),
                scan.method,
                scan.scanned,
                scan.skipped
            );
            if let Some(reason) = scan.fallback_reason {
                let _ = writeln!(
                    out,
                    "getTransactionsForAddress failed ({reason}); used the signature scan instead"
                );
            }
            out.push('\n');
            out.push_str(&render(&reports, false));
            Ok(out)
        }
    }
}

fn source_label(client: &Client) -> String {
    match client.source() {
        Source::Helius => "helius".to_string(),
        Source::PublicRpc => "public-rpc (no HELIUS_API_KEY set)".to_string(),
    }
}

/// Accepts a recorded fixture (`{ "response": <tx>, "source": .. }`) or a bare getTransaction result.
pub fn report_from_file(value: &Value) -> Result<Report, String> {
    let tx_value = value.get("response").unwrap_or(value);
    let tx: RawTransaction = serde_json::from_value(tx_value.clone())
        .map_err(|e| format!("not a getTransaction result: {e}"))?;
    let recorded = value
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or("unknown source");
    let signature = value
        .get("signature")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| tx.transaction.signatures.first().cloned());
    report_from_tx(&tx, signature, format!("file (recorded from {recorded})"))
}

fn report_from_tx(
    tx: &RawTransaction,
    signature: Option<String>,
    source: String,
) -> Result<Report, String> {
    let meta: &TxMeta = tx.meta.as_ref().ok_or("the transaction has no meta")?;
    if meta.err.is_null() {
        return Err("the transaction succeeded, so there is nothing to diagnose".to_string());
    }
    Ok(Report {
        signature,
        source,
        diagnosis: diagnose(meta),
        helius_parsed_events: None,
    })
}

fn cross_check(client: &Client, signature: &str, d: &Diagnosis) -> Option<CrossCheck> {
    match client.parsed_events_errors(&[signature.to_string()]) {
        Ok(None) => None,
        Ok(Some(map)) => {
            let decoded = map.get(signature).cloned().flatten();
            let agrees = decoded
                .as_ref()
                .and_then(|e| e.code)
                .map(|c| Some(c) == d.error_code);
            Some(CrossCheck {
                agrees,
                decoded_error: decoded,
                error: None,
            })
        }
        Err(e) => Some(CrossCheck {
            agrees: None,
            decoded_error: None,
            error: Some(e.to_string()),
        }),
    }
}

pub fn render(reports: &[Report], json: bool) -> String {
    if json {
        let v = if reports.len() == 1 {
            serde_json::to_value(&reports[0])
        } else {
            serde_json::to_value(reports)
        };
        return serde_json::to_string_pretty(&v.unwrap_or_default()).unwrap_or_default() + "\n";
    }
    reports
        .iter()
        .map(render_text)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_text(r: &Report) -> String {
    let d = &r.diagnosis;
    let mut out = String::new();
    let mut line = |k: &str, v: String| {
        let _ = writeln!(out, "{k:<12} {v}");
    };
    if let Some(sig) = &r.signature {
        line("signature", sig.clone());
    }
    line("cause", d.cause.as_str().to_string());
    line("title", d.title.to_string());
    line("proof", d.evidence.clone());
    line("fix", d.fix.to_string());
    if let Some(id) = &d.program_id {
        line(
            "program",
            match d.program_name {
                Some(name) => format!("{id} ({name})"),
                None => id.clone(),
            },
        );
    }
    if let Some(i) = d.instruction_index {
        line("instruction", format!("{i} (Explorer shows #{})", i + 1));
    }
    if d.error_code.is_some() || d.error_name.is_some() {
        let code = d.error_code.map(|c| c.to_string()).unwrap_or_default();
        let name = d.error_name.clone().unwrap_or_default();
        line("error", format!("{code} {name}").trim().to_string());
    }
    line("source", r.source.clone());
    if let Some(c) = &r.helius_parsed_events {
        let text = match (&c.error, &c.decoded_error, c.agrees) {
            (Some(e), _, _) => format!("unavailable ({e})"),
            (None, None, _) => "no decoded error returned".to_string(),
            (None, Some(e), agrees) => format!(
                "code {} {} ({})",
                e.code.map(|c| c.to_string()).unwrap_or_else(|| "?".into()),
                e.name.clone().unwrap_or_default(),
                match agrees {
                    Some(true) => "agrees",
                    Some(false) => "differs",
                    None => "no code to compare",
                }
            ),
        };
        line("cross-check", format!("helius parsed events: {text}"));
    }
    out
}
