// Runs target/deploy/failure_lab.so in LiteSVM, fails each instruction on purpose,
// and records the logs and error as getTransaction-shaped JSON for the Rust tests
// in crates/tx-doctor/tests/lab.rs.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { FailedTransactionMetadata, LiteSVM } from "litesvm";
import {
  AccountRole,
  address,
  appendTransactionMessageInstruction,
  createTransactionMessage,
  generateKeyPairSigner,
  getAddressEncoder,
  getProgramDerivedAddress,
  getSignatureFromTransaction,
  lamports,
  pipe,
  setTransactionMessageFeePayerSigner,
  signTransactionMessageWithSigners,
} from "@solana/kit";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..");
const PROGRAM_SO = join(ROOT, "target/deploy/failure_lab.so");
const OUT_DIR = join(ROOT, "crates/tx-doctor/tests/fixtures/lab");
const PROGRAM_ID = address("3RUGuRGUWq9guvxDz9suK6FxHHbuCm3YjtkT5zsSGtSY");
const SYSTEM_PROGRAM = address("11111111111111111111111111111111");

function ixData(name, arg) {
  const disc = createHash("sha256").update(`global:${name}`).digest().subarray(0, 8);
  if (arg === undefined) return new Uint8Array(disc);
  const n = Buffer.alloc(8);
  n.writeBigUInt64LE(BigInt(arg));
  return new Uint8Array(Buffer.concat([disc, n]));
}

function errJson(err) {
  if (typeof err?.err === "function" && typeof err.index === "number") {
    const inner = err.err();
    if (typeof inner?.code === "number") return { InstructionError: [err.index, { Custom: inner.code }] };
    return { InstructionError: [err.index, String(inner)] };
  }
  return String(err);
}

async function send(svm, payer, instruction) {
  svm.expireBlockhash();
  const tx = await pipe(
    createTransactionMessage({ version: 0 }),
    (m) => setTransactionMessageFeePayerSigner(payer, m),
    (m) => svm.setTransactionMessageLifetimeUsingLatestBlockhash(m),
    (m) => appendTransactionMessageInstruction(instruction, m),
    (m) => signTransactionMessageWithSigners(m),
  );
  return { signature: getSignatureFromTransaction(tx), result: svm.sendTransaction(tx) };
}

async function lab() {
  const svm = new LiteSVM();
  svm.addProgramFromFile(PROGRAM_ID, PROGRAM_SO);
  const authority = await generateKeyPairSigner();
  svm.airdrop(authority.address, lamports(1_000_000_000n));
  const [vault] = await getProgramDerivedAddress({
    programAddress: PROGRAM_ID,
    seeds: ["vault", getAddressEncoder().encode(authority.address)],
  });
  const init = await send(svm, authority, {
    programAddress: PROGRAM_ID,
    accounts: [
      { address: vault, role: AccountRole.WRITABLE },
      { address: authority.address, role: AccountRole.WRITABLE_SIGNER, signer: authority },
      { address: SYSTEM_PROGRAM, role: AccountRole.READONLY },
    ],
    data: ixData("init_vault", 100),
  });
  assert.ok(!(init.result instanceof FailedTransactionMetadata), `init_vault failed: ${init.result}`);
  const useVault = (name, arg, signer) => ({
    programAddress: PROGRAM_ID,
    accounts: [
      { address: vault, role: AccountRole.WRITABLE },
      { address: signer.address, role: AccountRole.READONLY_SIGNER, signer },
    ],
    data: ixData(name, arg),
  });
  return { svm, authority, useVault };
}

function record(name, expected, { signature, result }) {
  assert.ok(result instanceof FailedTransactionMetadata, `${name} was expected to fail`);
  const meta = result.meta();
  const err = errJson(result.err());
  assert.deepEqual(err, { InstructionError: [0, { Custom: expected.code }] }, `${name} error`);
  mkdirSync(OUT_DIR, { recursive: true });
  const fixture = {
    signature,
    source: "litesvm",
    expected_cause: expected.cause,
    expected_error_code: expected.code,
    expected_error_name: expected.name,
    response: {
      slot: 0,
      blockTime: null,
      meta: {
        err,
        fee: 5000,
        computeUnitsConsumed: Number(meta.computeUnitsConsumed()),
        logMessages: meta.logs(),
      },
      transaction: { signatures: [signature], message: {} },
    },
  };
  writeFileSync(join(OUT_DIR, `${name}.json`), JSON.stringify(fixture, null, 2) + "\n");
}

test("a withdraw within the limit succeeds", async () => {
  const { svm, authority, useVault } = await lab();
  const { result } = await send(svm, authority, useVault("withdraw", 100, authority));
  assert.ok(!(result instanceof FailedTransactionMetadata), String(result));
});

test("require! with a custom error fails with 6000", async () => {
  const { svm, authority, useVault } = await lab();
  record(
    "withdraw_over_limit",
    { cause: "program_custom_error", code: 6000, name: "AmountOverLimit" },
    await send(svm, authority, useVault("withdraw", 101, authority)),
  );
});

test("a has_one violation fails with 2001", async () => {
  const { svm, useVault } = await lab();
  const stranger = await generateKeyPairSigner();
  svm.airdrop(stranger.address, lamports(1_000_000_000n));
  record(
    "set_limit_wrong_authority",
    { cause: "anchor_constraint_violation", code: 2001, name: "ConstraintHasOne" },
    await send(svm, stranger, useVault("set_limit", 5, stranger)),
  );
});

test("an account that was never created fails with 3012", async () => {
  const { svm, authority } = await lab();
  const missing = (await generateKeyPairSigner()).address;
  record(
    "read_ledger_not_initialized",
    { cause: "account_not_initialized", code: 3012, name: "AccountNotInitialized" },
    await send(svm, authority, {
      programAddress: PROGRAM_ID,
      accounts: [{ address: missing, role: AccountRole.READONLY }],
      data: ixData("read_ledger"),
    }),
  );
});
