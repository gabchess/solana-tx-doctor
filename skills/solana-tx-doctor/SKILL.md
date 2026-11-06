---
name: solana-tx-doctor
description: Find why a Solana transaction failed and how to fix it. Use when someone shares a failed transaction signature, an Explorer link, a "custom program error: 0x..." message, an AnchorError log, or asks why their swap, transfer or program call failed on Solana.
---

# Solana tx doctor

Turn one failed transaction into one root cause, the log line that proves it, and a fix.

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

## References

- [references/error-codes.md](references/error-codes.md): builtin error names and known per-program codes.
- [references/anchor-ranges.md](references/anchor-ranges.md): Anchor framework error ranges.
- [references/helius-endpoints.md](references/helius-endpoints.md): the Helius calls and their request shapes.
