---
name: solana-tx-doctor
description: Find why a Solana transaction failed and how to fix it. Use when someone shares a failed transaction signature, an Explorer link, a "custom program error: 0x..." message, an AnchorError log, or asks why their swap, transfer or program call failed on Solana.
---

# Solana tx doctor

Turn one failed transaction into one root cause, the log line that proves it, and a fix. The procedure below is the same rule order that `crates/tx-doctor/src/diagnose/rules.rs` runs, so the agent and the CLI give the same answer. It reads only `meta.err` and `meta.logMessages`, and it never needs a model to guess.

## Task routing

| When you are | Read |
| --- | --- |
| Fetching a transaction or an address's failures from Helius | [references/helius-endpoints.md](references/helius-endpoints.md) |
| Mapping a builtin error name or a known program's custom code | [references/error-codes.md](references/error-codes.md) |
| Reading an `AnchorError ... Error Number: N.` line | [references/anchor-ranges.md](references/anchor-ranges.md) |
| Diagnosing with the CLI installed | The CLI section below |

## When to use

- A user pastes a transaction signature or a Solana Explorer link and says it failed.
- A user quotes `custom program error: 0x...`, `AnchorError`, `insufficient lamports` or `exceeded CUs meter`.
- A user asks why recent transactions from a wallet or program keep failing.

Do not use it for transactions that succeeded but did the wrong thing. The logs of a successful transaction hold no error to decode.

## Procedure

1. Fetch the transaction. Run `tx-doctor tx <SIG>` if the CLI is installed. Otherwise call `getTransaction` with `{"encoding":"json","maxSupportedTransactionVersion":1,"commitment":"finalized"}` on `https://mainnet.helius-rpc.com/?api-key=$HELIUS_API_KEY`. See [references/helius-endpoints.md](references/helius-endpoints.md). Never print the key or the URL that holds it.
2. Read `meta.err`. `{"InstructionError":[i, X]}` means instruction `i` failed (zero-based; Explorer shows `i + 1`). `X` is either `{"Custom": n}` or a builtin name such as `"InsufficientFunds"`.
3. Find the origin. In `meta.logMessages`, take the first line that matches `Program <ID> failed: ...`. The innermost program fails first, and its callers repeat the same code, so the first failed line names the program that caused the failure.
4. Scope the frame. Keep only the lines from that program's last `Program <ID> invoke [` line up to its failed line. Ignore every line outside this frame. A slippage or lamports message from an earlier program that succeeded does not explain this failure.
5. Apply the rules in this order, and stop at the first match:
   1. A compute message in the frame, or the builtin `ComputationalBudgetExceeded`, means compute budget exceeded.
   2. `InsufficientFundsForFee`, `InsufficientFundsForRent`, or `insufficient lamports` in the frame means not enough SOL.
   3. A builtin error name maps through the table in [references/error-codes.md](references/error-codes.md).
   4. A custom code that the failing program's own table lists takes that table's cause. Look the code up per program id. The same number means different things in different programs.
   5. A frame line that contains "error" and a slippage word means slippage exceeded.
   6. An `AnchorError ... Error Number: N.` line in the frame maps through [references/anchor-ranges.md](references/anchor-ranges.md). Use the Anchor ranges only when that line exists. A program that is not built with Anchor can return 2006 or 3012 with its own meaning.
   7. Any other custom code is the program's own error. Point the user to that program's IDL or source.
6. Report the cause, the proof line copied exactly from the logs, the fix, the program id, and the instruction index as Explorer shows it.

## CLI

```
tx-doctor tx <SIG>                    # one transaction
tx-doctor address <ADDR> --limit 10   # recent failed transactions for an address
tx-doctor file <tx.json>              # a saved getTransaction result, offline
tx-doctor --json tx <SIG>             # JSON output
```

With `HELIUS_API_KEY` set, `tx` also asks Helius Parsed Events for its `decodedError` and prints whether it agrees.

## Example report

From `tx-doctor file crates/tx-doctor/tests/fixtures/tx/slippage_exceeded-22fygFWb.json`, a recorded pump.fun buy:

```
cause        slippage_exceeded
proof        Program log: AnchorError thrown in programs/pump/src/trade_v3.rs:1001. Error Code: BuySlippageBelowMinTokensOut. Error Number: 6042. Error Message: Slippage: Would buy less tokens than expected min_tokens_out.
fix          The price moved past your limit before the swap landed. Retry with a fresh quote, raise slippage a little, or add a priority fee so the transaction lands sooner.
program      6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P (pump.fun)
instruction  13 (Explorer shows #14)
```

pump.fun has no code table here, so rule 4 does not match. Rule 5 matches first, because the frame line holds "Error" and "Slippage". That makes the cause slippage, not the program's own error 6042.
