use anchor_lang::prelude::*;

pub mod error;
pub mod ixs;
pub mod state;

use ixs::*;

declare_id!("AZzGDkysPRAZ9cfyRo1w4rHMS51NDDNT9XqHsC1WziLM");

#[program]
pub mod xusdc {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }

    pub fn contribute_rent(ctx: Context<ContributeRent>, amount: u64) -> Result<()> {
        contribute_rent::handler(ctx, amount)
    }

    pub fn withdraw_rent(ctx: Context<WithdrawRent>, amount: u64) -> Result<()> {
        withdraw_rent::handler(ctx, amount)
    }

    pub fn garbage_collect(ctx: Context<GarbageCollect>) -> Result<()> {
        garbage_collect::handler(ctx)
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}
