# solana-tx-doctor

`tx-doctor` is a Rust CLI that reads a failed Solana transaction through Helius RPC and prints the root cause, the log line that proves it, and a fix. The repo also has an Anchor program that fails on purpose (`programs/failure-lab`) and a Claude skill (`skills/solana-tx-doctor`) that teaches an agent the same procedure.

Draft. Commands and output below are real runs. Prose will be rewritten.

## Quickstart

Get a free Helius API key at https://dashboard.helius.dev, then `cp .env.example .env` and set HELIUS_API_KEY.

```
cargo build --release -p tx-doctor
./target/release/tx-doctor tx <SIGNATURE>
./target/release/tx-doctor address <ADDRESS> --limit 10
./target/release/tx-doctor file crates/tx-doctor/tests/fixtures/tx/insufficient_token_balance-2CHCzcmK.json
./target/release/tx-doctor --json tx <SIGNATURE>
```

Without `HELIUS_API_KEY`, `tx` and `address` use the public RPC at `https://api.mainnet-beta.solana.com` and the output says `public-rpc (no HELIUS_API_KEY set)`. Errors name the source and never print the key or the URL.

The Helius path has unit tests against a mock server. It was not run live: no key was available when this was built. The public RPC path was run live (output below).

## CLI output

`tx` on the public RPC, no key set (live run, 2026-11-06):

```
$ tx-doctor tx 2CHCzcmKn5i8jDmUjbsFrZrjijyJF1jH6oPvkt4BXsXDjYoWWPhmvycdpGYGPBLXrTQqbTf65bBQpmCm8PQ9fqGN
signature    2CHCzcmKn5i8jDmUjbsFrZrjijyJF1jH6oPvkt4BXsXDjYoWWPhmvycdpGYGPBLXrTQqbTf65bBQpmCm8PQ9fqGN
cause        insufficient_token_balance
title        Not enough tokens in the source account
proof        Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA failed: custom program error: 0x1
fix          The token account had fewer tokens than the transfer or swap amount. Re-read the balance right before signing, and leave room for fees and rounding.
program      TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA (SPL Token)
instruction  8 (Explorer shows #9)
error        1 InsufficientFunds
source       public-rpc (no HELIUS_API_KEY set)
```

`address` on the public RPC (live run, 2026-11-06, first result shown):

```
$ tx-doctor address JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 --limit 2
2 failed transactions for JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 via public-rpc (no HELIUS_API_KEY set) getSignaturesForAddress (scanned 100, skipped 0)

signature    4V5d9C27kWxfkqrpW8fJstBbsh5SpTJ3eLdqY2TrhhkpdWFJyUNmBqZNzh5KC5KPMAuoq6ZiL4nLa4ZPpX8QUwsU
cause        slippage_exceeded
title        Slippage tolerance exceeded
proof        Program JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 failed: custom program error: 0x1771
fix          The price moved past your limit before the swap landed. Retry with a fresh quote, raise slippage a little, or add a priority fee so the transaction lands sooner.
program      JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4 (Jupiter Aggregator v6)
instruction  5 (Explorer shows #6)
error        6001 SlippageToleranceExceeded
source       public-rpc (no HELIUS_API_KEY set)
```

`file` on a recorded mainnet fixture (offline):

```
$ tx-doctor file crates/tx-doctor/tests/fixtures/tx/slippage_exceeded-22fygFWb.json
signature    22fygFWbn1Ux2L9Xu6f3ZFpbGnVRFmWTvFiaE8kmgV8xeApZQV5xQYa3i3y5GUErYnRZSivhc7idHonBk9hRvh1d
cause        slippage_exceeded
title        Slippage tolerance exceeded
proof        Program log: AnchorError thrown in programs/pump/src/trade_v3.rs:1001. Error Code: BuySlippageBelowMinTokensOut. Error Number: 6042. Error Message: Slippage: Would buy less tokens than expected min_tokens_out.
fix          The price moved past your limit before the swap landed. Retry with a fresh quote, raise slippage a little, or add a priority fee so the transaction lands sooner.
program      6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P (pump.fun)
instruction  13 (Explorer shows #14)
error        6042 BuySlippageBelowMinTokensOut
source       file (recorded from public-rpc)
```

With a key, `tx` adds a `cross-check` line from Helius Parsed Events (`decodedError`) and says whether its code agrees with ours.

## Anchor failure lab

`programs/failure-lab` is an Anchor 1.0 program. Program id `3RUGuRGUWq9guvxDz9suK6FxHHbuCm3YjtkT5zsSGtSY`. It was never deployed to any cluster. The keypair in `target/deploy/` is committed so the id stays stable; it controls nothing on mainnet.

| Instruction | Fails with | Code |
| --- | --- | --- |
| `init_vault(limit)` | succeeds, creates the vault PDA | |
| `withdraw(amount)` | `require!` with custom `LabError::AmountOverLimit` when `amount > limit` | 6000 |
| `set_limit(limit)` | `has_one = authority` violation when another signer calls it | 2001 `ConstraintHasOne` |
| `read_ledger()` | `Account<Ledger>` that was never created | 3012 `AccountNotInitialized` |

`lab/record.test.mjs` loads `target/deploy/failure_lab.so` into LiteSVM (npm `litesvm` 1.6.2), sends each failing instruction, and writes the logs and error to `crates/tx-doctor/tests/fixtures/lab/`. `crates/tx-doctor/tests/lab.rs` checks that each one decodes to the named error:

```
$ tx-doctor file crates/tx-doctor/tests/fixtures/lab/set_limit_wrong_authority.json
signature    3pawp5piYfrimeZ9ytanL6pTQ1d6BX4yH4gXaDw6eZxuymzVFR9s1iikD9cJYe57ixZbe8wAGeX9oLD7iL9V2ycS
cause        anchor_constraint_violation
title        An Anchor account constraint failed
proof        Program log: AnchorError caused by account: vault. Error Code: ConstraintHasOne. Error Number: 2001. Error Message: A has one constraint was violated.
fix          An account did not match the program's #[account(...)] rules, such as seeds, owner, has_one or mut. Compare the accounts the client passes with the program's IDL.
program      3RUGuRGUWq9guvxDz9suK6FxHHbuCm3YjtkT5zsSGtSY (failure-lab)
instruction  0 (Explorer shows #1)
error        2001 ConstraintHasOne
source       file (recorded from litesvm)
```

The lab runs in the npm `litesvm` package, not the Rust crate. The Rust crate did not resolve with the toolchain in the Anchor image: `litesvm` 0.8.1 does not compile against agave 3.1.14, and 0.15 and 0.18 need rustc 1.97 or newer while `solanafoundation/anchor:v1.0.2` ships rustc 1.95.0.

### Build the program

No local Rust or Solana toolchain is needed. With Docker:

```
docker run --rm -v "$PWD":/work -w /work solanafoundation/anchor:v1.0.2 anchor build
```

The image has anchor-cli 1.0.2, solana-cli 3.1.10 and cargo 1.95.0. The build writes `target/deploy/failure_lab.so`.

## Claude skill

```
cp -r skills/solana-tx-doctor ~/.claude/skills/
```

`skills/solana-tx-doctor/SKILL.md` holds the decision procedure. `references/` holds the error tables and the Helius request shapes.

## Tests

Inside the Anchor image (or with Rust, Anchor and Node installed):

```
anchor build
npm ci --prefix lab && npm test --prefix lab
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
python3 scripts/check_skill.py skills/solana-tx-doctor/SKILL.md
```

`cargo test` never touches the network. One live test is ignored by default and needs a key:

```
HELIUS_API_KEY=... cargo test -p tx-doctor -- --ignored live_
```

## Which Helius APIs and why

| API | Used by | Why |
| --- | --- | --- |
| `getTransaction` on `mainnet.helius-rpc.com` | `tx`, `address` fallback | Logs and `meta.err` are all the rules need |
| `getTransactionsForAddress` with `filters.status = "failed"` | `address` | Failed transactions for an address in one call, instead of 1 + N calls |
| `getSignaturesForAddress` + `getTransaction` | `address` fallback | Runs if `getTransactionsForAddress` errors; the docs do not say whether the free plan includes it |
| Parsed Events `POST /v1/parsed-events/transactions` | `tx` | Helius `decodedError` as a second opinion on our decode |

## Layout

```
crates/tx-doctor/src/diagnose/   causes, frame scoping, known programs, rules
crates/tx-doctor/src/sources/    Helius and public RPC clients
crates/tx-doctor/src/cli/        clap commands and output
programs/failure-lab/            Anchor program that fails on purpose
lab/                             LiteSVM recorder for the failure lab
skills/solana-tx-doctor/         Claude skill
docs/design.md                   design note
```

MIT license.
