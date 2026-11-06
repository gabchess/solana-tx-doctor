# Anchor error ranges

Anchor writes a log line in this form when an instruction fails:

```
Program log: AnchorError caused by account: vault. Error Code: ConstraintHasOne. Error Number: 2001. Error Message: A has one constraint was violated.
Program log: AnchorError thrown in programs/x/src/lib.rs:21. Error Code: AmountOverLimit. Error Number: 6000. Error Message: ...
```

Use these ranges only when the failing program's frame has such a line. The numbers come from `ErrorCode` in the `anchor-lang-error` crate (anchor `lang/error/src/lib.rs`).

| Number | Meaning | Cause |
| --- | --- | --- |
| 100 to 103 | Discriminator missing, fallback not supported, or instruction data that does not deserialize | The program did not accept this instruction |
| 2000 to 2999, except 2500 to 2506 | Account constraint failed (`mut`, `has_one`, `seeds`, `owner` and others). 2001 is `ConstraintHasOne`, 2006 is `ConstraintSeeds` | An Anchor account constraint failed |
| 2500 to 2506 | A `require!` family check in the program's logic failed. 2500 is `RequireViolated`, 2501 `RequireEqViolated`, 2502 `RequireKeysEqViolated`, 2505 `RequireGtViolated`. The AnchorError line names the source file and line of the check | The program returned its own error |
| 3001, 3012 | No discriminator on the account, or the account was never initialized | An account was not initialized |
| 3002, 3007, 3008, 3014 | Wrong discriminator, owned by another program, wrong program id, not the associated token account | Wrong account passed |
| 3005 | Not enough account keys for the instruction | The program did not accept this instruction |
| 3010 | Account not signer | A required signer did not sign |
| 6000 and up | The program's own `#[error_code]` enum | Look up the name in the program's IDL |

A program that does not use Anchor can return any of these numbers with its own meaning. Without the AnchorError line, treat the code as the program's own error.
