# solana-tx-doctor

`tx-doctor` names why a Solana transaction failed. It reads the transaction's `meta.err` and logs through Helius RPC, then prints the cause, the log line that proves it and a fix. Explorer shows a raw `custom program error: 0x1`, and that code means different things in different programs. `tx-doctor` looks it up by the failing program id. Plain Rust rules, no model call.

| Part | Path | Built with |
| --- | --- | --- |
| CLI and rules | [`crates/tx-doctor/`](crates/tx-doctor/) | Rust 2021, clap, reqwest |
| Program that fails on purpose | [`programs/failure-lab/`](programs/failure-lab/src/lib.rs) | Anchor 1.0.2 |
| Data source | [`crates/tx-doctor/src/sources/`](crates/tx-doctor/src/sources/) | Helius RPC, `getTransactionsForAddress`, Parsed Events |
| Agent skill | [`skills/solana-tx-doctor/`](skills/solana-tx-doctor/SKILL.md) | Claude skill, same procedure as the rules |

Proof. `cargo test --workspace` runs 28 Rust tests. 27 passed and 1 is ignored, the live Helius test that needs a key. All 13 recorded mainnet failures and all 3 failure-lab runs decode to their recorded cause.

## Quickstart

Get a free Helius API key at https://dashboard.helius.dev. The key is optional. Without it, `tx` and `address` use the public mainnet RPC, and `file` never needs the network.

```bash
cp .env.example .env               # then set HELIUS_API_KEY=<your key> in .env
cargo build -p tx-doctor           # builds ./target/debug/tx-doctor
./target/debug/tx-doctor file crates/tx-doctor/tests/fixtures/tx/insufficient_token_balance-2CHCzcmK.json  # offline, no key
./target/debug/tx-doctor tx <SIGNATURE>              # one transaction, Helius if the key is set
./target/debug/tx-doctor address <ADDRESS> --limit 10  # recent failed transactions for an address
./target/debug/tx-doctor --json tx <SIGNATURE>       # same diagnosis as JSON
```

Expected output of the `file` command, from a run on 2026-11-06:

```
signature    2CHCzcmKn5i8jDmUjbsFrZrjijyJF1jH6oPvkt4BXsXDjYoWWPhmvycdpGYGPBLXrTQqbTf65bBQpmCm8PQ9fqGN
cause        insufficient_token_balance
title        Not enough tokens in the source account
proof        Program TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA failed: custom program error: 0x1
fix          The token account had fewer tokens than the transfer or swap amount. Re-read the balance right before signing, and leave room for fees and rounding.
program      TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA (SPL Token)
instruction  8 (Explorer shows #9)
error        1 InsufficientFunds
source       file (recorded from public-rpc)
```

`tx-doctor tx` on the same signature with no key prints the same lines, except `source  public-rpc (no HELIUS_API_KEY set)` (live run, 2026-11-06).

No local Rust? Every command above also runs inside the Anchor image that CI uses.

```bash
docker run --rm -v "$PWD":/work -w /work solanafoundation/anchor:v1.0.2 bash -c 'cargo build -p tx-doctor && ./target/debug/tx-doctor file crates/tx-doctor/tests/fixtures/tx/insufficient_token_balance-2CHCzcmK.json'  # same output as above
```

## Setup

| Variable | File | Required | Read by |
| --- | --- | --- | --- |
| `HELIUS_API_KEY` | [`.env.example`](.env.example), copied to `.env` | No | `Client::from_env` in `crates/tx-doctor/src/sources/mod.rs`, after `main.rs` loads `.env` with dotenvy. The process environment works too |

With the key, requests go to `https://mainnet.helius-rpc.com`. With no key, or an empty one, they go to `https://api.mainnet-beta.solana.com`. The `source` line of every result says which one answered. Errors name the source and never print the key or a URL that contains it. Mock tests in `sources/` check that.

The Helius path was not run live, because no key was available when this was built. Its request shapes are covered by unit tests against an httpmock server, and one live test waits for a key.

```bash
HELIUS_API_KEY=<your key> cargo test -p tx-doctor -- --ignored live_  # not run yet
```

## How it works

1. `sources::Client` fetches the transaction with `getTransaction` from Helius, or from the public RPC without a key (`crates/tx-doctor/src/sources/mod.rs`).
2. `diagnose` takes the first `Program <ID> failed:` line as the origin, because callers repeat the same code after it (`crates/tx-doctor/src/diagnose/rules.rs`).
3. `failing_frame` keeps only that program's own lines, from its last `invoke [` to its `failed` line, so a message from an earlier program that succeeded cannot set the cause (`crates/tx-doctor/src/diagnose/frame.rs`).
4. Custom codes are looked up by program id first, because `0x1` means "not enough tokens" in SPL Token and something else elsewhere. Anchor ranges count only when the frame has an `AnchorError` line (`crates/tx-doctor/src/diagnose/programs.rs`).
5. With a key, `tx` also asks Helius Parsed Events for its `decodedError` and prints a `cross-check` line that says whether Helius agrees (`crates/tx-doctor/src/cli/mod.rs`).

## Helius APIs used

| API | Used by | Why |
| --- | --- | --- |
| `getTransaction` on `mainnet.helius-rpc.com` | `tx`, `address` fallback | `meta.err` and `meta.logMessages` are all the rules read |
| `getTransactionsForAddress` with `filters.status = "failed"` | `address` | Failed transactions for an address in one call, instead of 1 + N calls |
| `getSignaturesForAddress` then `getTransaction` | `address` fallback | Runs when `getTransactionsForAddress` returns an error. The Helius docs do not say whether the free plan includes that method |
| Parsed Events `POST /v1/parsed-events/transactions` | `tx` | Helius `decodedError` as a second opinion on the local decode |

## Anchor program

[`programs/failure-lab/src/lib.rs`](programs/failure-lab/src/lib.rs) is an Anchor 1.0.2 program with program id `3RUGuRGUWq9guvxDz9suK6FxHHbuCm3YjtkT5zsSGtSY`. It was never deployed to any cluster. Each instruction except `init_vault` fails on purpose, so the doctor is tested against real program logs.

| Instruction | How it fails | Code |
| --- | --- | --- |
| `init_vault(limit)` | Succeeds and creates the vault PDA | none |
| `withdraw(amount)` | `require!` returns `LabError::AmountOverLimit` when `amount` is over the limit | 6000 |
| `set_limit(limit)` | `has_one = authority` fails when another signer calls it | 2001 `ConstraintHasOne` |
| `read_ledger()` | `Account<Ledger>` points at an account that was never created | 3012 `AccountNotInitialized` |

`lab/record.test.mjs` loads `target/deploy/failure_lab.so` into LiteSVM (npm `litesvm` 1.6.2), sends each instruction and writes each failure as a `getTransaction`-shaped file to `crates/tx-doctor/tests/fixtures/lab/`. `crates/tx-doctor/tests/lab.rs` checks that each file decodes to its named error. Output for the `set_limit` recording, from a run on 2026-11-06:

```
$ ./target/debug/tx-doctor file crates/tx-doctor/tests/fixtures/lab/set_limit_wrong_authority.json
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

Build the program inside the Anchor image. No local Rust or Solana toolchain is needed.

```bash
docker run --rm -v "$PWD":/work -w /work solanafoundation/anchor:v1.0.2 anchor build  # writes target/deploy/failure_lab.so
```

The lab uses the npm `litesvm` package, not the Rust crate. Rust `litesvm` 0.8.1 does not compile against agave 3.1.14, and 0.15 and 0.18 need rustc 1.97 or newer, while the image ships rustc 1.95.0.

## Tests

These are the CI steps in [`.github/workflows/ci.yml`](.github/workflows/ci.yml). They run inside `solanafoundation/anchor:v1.0.2`.

```bash
anchor build                                            # builds the failure-lab program
npm ci --prefix lab && npm test --prefix lab            # records the 3 lab failures in LiteSVM
cargo fmt --all --check                                 # formatting
cargo clippy --workspace --all-targets -- -D warnings   # lints as errors
cargo test --workspace                                  # all Rust tests, no network
python3 scripts/check_skill.py skills/solana-tx-doctor/SKILL.md  # skill frontmatter and references
```

Results from runs in the image on 2026-11-06. `anchor build` was not run locally for this README. CI runs it.

| Suite | Result | What it proves |
| --- | --- | --- |
| `crates/tx-doctor/src/diagnose/rules.rs` and `frame.rs` | 14 passed | The first-failed-line rule, frame scoping, per-program codes, that a non-Anchor 2006 or 3012 stays the program's own error, and that an Anchor 2500 `require!` failure is not a constraint failure like 2001 |
| `crates/tx-doctor/src/sources/` | 9 passed, 1 ignored | Helius request shapes, the fallback scan, the 429 retry, and that errors never contain the key. The ignored test is the live Helius run |
| `crates/tx-doctor/tests/fixtures.rs` | 2 passed | All 13 recorded mainnet failures decode to the cause in their file name, and the proof line is copied from the logs when the logs have one |
| `crates/tx-doctor/tests/lab.rs` | 3 passed | The failure-lab logs decode to 6000 `AmountOverLimit`, 2001 `ConstraintHasOne` and 3012 `AccountNotInitialized` |
| `lab/record.test.mjs` | 4 of 4 pass | `init_vault` and an in-limit `withdraw` succeed, and the 3 failing instructions return their expected codes |

## Claude skill

```bash
cp -r skills/solana-tx-doctor ~/.claude/skills/  # install for Claude Code
```

[`skills/solana-tx-doctor/SKILL.md`](skills/solana-tx-doctor/SKILL.md) gives an agent the same procedure as the Rust rules. Its `references/` hold the error tables and the Helius request shapes.

## Requirements

| Tool | Version | Needed for |
| --- | --- | --- |
| Docker image `solanafoundation/anchor` | v1.0.2 (anchor-cli 1.0.2, solana-cli 3.1.10, cargo 1.95.0, Node 20.18.0) | Everything below, if you do not install them locally |
| Rust | not pinned (CI uses cargo 1.95.0) | The CLI and tests |
| Anchor | `anchor-lang = "1.0.2"` | Building `programs/failure-lab`. Optional, the built `.so` is committed |
| Node | not pinned (CI uses 20.18.0) | `lab/`, optional |
| Helius API key | free plan | Optional |

## Layout

```
.env.example                      # HELIUS_API_KEY, copy to .env
.github/workflows/ci.yml          # anchor build, lab, fmt, clippy, cargo test, skill check
Anchor.toml                       # failure_lab program id on localnet
Cargo.toml                        # workspace of programs/failure-lab and crates/tx-doctor
Cargo.lock                        # pinned Rust dependencies
crates/tx-doctor/                 # the CLI crate
  Cargo.toml                      # clap, reqwest, serde, httpmock for tests
  src/main.rs                     # loads .env, runs the CLI
  src/lib.rs                      # exposes cli, diagnose and sources to the tests
  src/cli/mod.rs                  # tx, address, file, --json, and the cross-check line
  src/diagnose/mod.rs             # Diagnosis type and entry point
  src/diagnose/rules.rs           # rule order and the false-positive guards
  src/diagnose/frame.rs           # the failing program's own log frame
  src/diagnose/causes.rs          # 11 causes, each with a title and a fix
  src/diagnose/programs.rs        # known program ids and their custom codes
  src/sources/mod.rs              # Client, Helius or public RPC, 429 retry
  src/sources/helius.rs           # getTransactionsForAddress and Parsed Events
  src/sources/public_rpc.rs       # signature scan that works on any RPC
  tests/fixtures.rs               # mainnet fixtures decode to their recorded cause
  tests/lab.rs                    # lab recordings decode to their named error
  tests/fixtures/tx/              # 13 recorded mainnet failures, named <cause>-<signature prefix>.json
  tests/fixtures/lab/             # 3 failure-lab runs recorded in LiteSVM
programs/failure-lab/             # the Anchor program
  Cargo.toml                      # anchor-lang 1.0.2
  src/lib.rs                      # 4 instructions, 3 fail on purpose
lab/                              # LiteSVM recorder for the failure lab
  package.json                    # litesvm 1.6.2, @solana/kit 8.6.0
  package-lock.json               # pinned npm dependencies
  record.test.mjs                 # runs the program and writes the lab fixtures
target/deploy/                    # committed on purpose, see the comments below
  failure_lab.so                  # so npm test runs with Node alone, no anchor build
  failure_lab-keypair.json        # only fixes the program id for local tests. The program is
                                  # not deployed. getAccountInfo returns null on mainnet,
                                  # devnet and testnet as of 2026-11-06. The secret is
                                  # public, so anyone could deploy at this address, and the
                                  # "failure-lab" label means nothing on a live cluster
skills/solana-tx-doctor/          # the Claude skill
  SKILL.md                        # procedure an agent follows
  references/error-codes.md       # builtin errors and per-program codes
  references/anchor-ranges.md     # Anchor error number ranges
  references/helius-endpoints.md  # Helius request shapes
scripts/check_skill.py            # CI check for SKILL.md frontmatter and references
docs/design.md                    # design note, components and limits
LICENSE                           # MIT
```

## What it does not do

It only reads. The crate never signs, simulates or sends a transaction, and it loads no keypair. It explains transactions that failed, not ones that succeeded and did the wrong thing.

## License

MIT. See [LICENSE](LICENSE).
