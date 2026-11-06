# Design note

## Goal

Show three Solana skills in one small repo. A Rust backend that calls Helius APIs, an Anchor program, and an agent skill. The backend answers one question: why did this transaction fail, and what should the developer change.

## Components

| Part | Path | Job |
| --- | --- | --- |
| Rules | `crates/tx-doctor/src/diagnose/` | Turn `meta.err` and `meta.logMessages` into one cause, a proof line and a fix |
| Sources | `crates/tx-doctor/src/sources/` | Fetch transactions from Helius, or the public RPC without a key |
| CLI | `crates/tx-doctor/src/cli/` | `tx`, `address`, `file`, with `--json` |
| Failure lab | `programs/failure-lab/` | Anchor program with one instruction per failure family |
| Lab recorder | `lab/record.test.mjs` | Run the lab in LiteSVM and save each failure as a getTransaction-shaped file |
| Skill | `skills/solana-tx-doctor/` | The same procedure, written for an agent |

The rules port `src/lib/diagnose.ts` from the grimoire-helius project rule for rule. They keep its cause list, its rule order and its three guards against false positives:

1. The first `Program <ID> failed:` line is the origin. Callers repeat the code later, so later lines are not the cause.
2. Only lines inside the failing program's own frame count, from its last `invoke [` line to its `failed` line.
3. Custom codes are looked up per program id. Anchor framework ranges apply only when the frame has an `AnchorError` line.

## Data flow

```
signature or address
  -> sources::Client (Helius with HELIUS_API_KEY, else public RPC)
  -> RawTransaction { meta: TxMeta { err, logMessages, fee, computeUnitsConsumed } }
  -> diagnose::diagnose(&TxMeta) -> Diagnosis
  -> cli render (text or JSON), plus the Parsed Events cross-check when a key is set
```

`file` skips the network and reads a saved result. The 13 mainnet fixtures and the 3 lab recordings use the same shape.

## Tests

| File | Count | What it proves |
| --- | --- | --- |
| `src/diagnose/rules.rs` | 11 | The false-positive guards and the builtin and Anchor mappings, ported from grimoire's `diagnose.test.ts` |
| `src/diagnose/frame.rs` | 2 | Frame scoping |
| `src/sources/*.rs` | 9 + 1 ignored | Request shapes, the fallback, 429 retry, and that errors never contain the key or URL, against an httpmock server |
| `tests/fixtures.rs` | 2 | All 13 recorded mainnet failures decode to their filed cause, and the proof is a real log line |
| `tests/lab.rs` | 3 | Real failure-lab logs decode to `AmountOverLimit` 6000, `ConstraintHasOne` 2001 and `AccountNotInitialized` 3012 |
| `lab/record.test.mjs` | 4 | The lab program succeeds on `init_vault` and `withdraw` within limit, and fails with the expected codes |

## Limits

- The Helius path was not run live. No key was available. The mock tests check request shapes taken from grimoire's Helius client.
- Whether the free Helius plan serves `getTransactionsForAddress` is not documented clearly. The CLI falls back to the signature scan on any error from that call.
- The known-program table is small. Unknown programs fall back to `program_custom_error` with the code and the Anchor name when one is logged.
- The lab runs in npm `litesvm`, not the Rust crate, because no Rust `litesvm` release resolved on rustc 1.95.0 (see README).
- `serve` (an HTTP route) was cut to keep the MVP small.
