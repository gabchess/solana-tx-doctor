# Error codes

## Builtin instruction errors

The runtime returns these as a string in `{"InstructionError":[i, "<Name>"]}`. Names come from `InstructionError` in anza-xyz/solana-sdk.

| Name | Cause |
| --- | --- |
| `InsufficientFunds` | Not enough SOL |
| `UninitializedAccount` | An account was not initialized |
| `InvalidInstructionData` | The program did not accept this instruction |
| `IllegalOwner`, `InvalidAccountOwner`, `IncorrectProgramId`, `InvalidAccountData` | Wrong account passed |
| `MissingRequiredSignature` | A required signer did not sign |
| `ComputationalBudgetExceeded` | Ran out of compute units |

Transaction-level errors `InsufficientFundsForFee` and `InsufficientFundsForRent` also mean not enough SOL.

## Custom codes by program

A custom code means something only inside the program that returned it. Look it up by the program id from the first `failed` line.

| Program | Id | Code | Name | Cause |
| --- | --- | --- | --- | --- |
| SPL Token | `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` | 1 (`0x1`) | `InsufficientFunds` | Not enough tokens |
| Token-2022 | `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` | 1 (`0x1`) | `InsufficientFunds` | Not enough tokens |
| Jupiter Aggregator v6 | `JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4` | 6001 (`0x1771`) | `SlippageToleranceExceeded` | Slippage exceeded |
| failure-lab (this repo) | `3RUGuRGUWq9guvxDz9suK6FxHHbuCm3YjtkT5zsSGtSY` | 6000 (`0x1770`) | `AmountOverLimit` | Program's own error |

Logs print the code in hex (`custom program error: 0x1771`). `meta.err` holds it in decimal (`{"Custom": 6001}`).

For any other program, read its IDL. Anchor programs number their own errors from 6000 in the order of the `#[error_code]` enum.
