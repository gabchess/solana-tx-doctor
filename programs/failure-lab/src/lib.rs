use anchor_lang::prelude::*;

declare_id!("3RUGuRGUWq9guvxDz9suK6FxHHbuCm3YjtkT5zsSGtSY");

/// Each instruction except `init_vault` fails on purpose, so the doctor can be
/// tested against real program logs: one custom error, one account constraint
/// and one missing account.
#[program]
pub mod failure_lab {
    use super::*;

    pub fn init_vault(ctx: Context<InitVault>, limit: u64) -> Result<()> {
        let vault = &mut ctx.accounts.vault;
        vault.authority = ctx.accounts.authority.key();
        vault.limit = limit;
        Ok(())
    }

    /// Fails with LabError::AmountOverLimit (6000) when `amount` is above the limit.
    pub fn withdraw(ctx: Context<UseVault>, amount: u64) -> Result<()> {
        require!(
            amount <= ctx.accounts.vault.limit,
            LabError::AmountOverLimit
        );
        Ok(())
    }

    /// Fails with ConstraintHasOne (2001) when the signer is not the vault authority.
    pub fn set_limit(ctx: Context<UseVault>, limit: u64) -> Result<()> {
        ctx.accounts.vault.limit = limit;
        Ok(())
    }

    /// Fails with AccountNotInitialized (3012) when the ledger account was never created.
    pub fn read_ledger(ctx: Context<ReadLedger>) -> Result<()> {
        msg!("ledger entries: {}", ctx.accounts.ledger.entries);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitVault<'info> {
    #[account(
        init,
        payer = authority,
        space = 8 + Vault::INIT_SPACE,
        seeds = [b"vault", authority.key().as_ref()],
        bump
    )]
    pub vault: Account<'info, Vault>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct UseVault<'info> {
    #[account(mut, has_one = authority)]
    pub vault: Account<'info, Vault>,
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct ReadLedger<'info> {
    pub ledger: Account<'info, Ledger>,
}

#[account]
#[derive(InitSpace)]
pub struct Vault {
    pub authority: Pubkey,
    pub limit: u64,
}

#[account]
#[derive(InitSpace)]
pub struct Ledger {
    pub entries: u64,
}

#[error_code]
pub enum LabError {
    #[msg("Amount is over the vault limit")]
    AmountOverLimit,
}
