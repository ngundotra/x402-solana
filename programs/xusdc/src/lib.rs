use anchor_lang::prelude::*;

pub mod error;
pub mod ixs;
pub mod state;

#[cfg(test)]
mod tests;

use ixs::*;

declare_id!("AZzGDkysPRAZ9cfyRo1w4rHMS51NDDNT9XqHsC1WziLM");

#[program]
pub mod xusdc {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        initialize::handler(ctx)
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

    pub fn settle_payment(ctx: Context<SettlePayment>, payload: SettlePayload) -> Result<()> {
        settle_payment::settle_payment(ctx, payload)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        deposit::handler(ctx, amount)
    }

    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        withdraw::handler(ctx, amount)
    }
}
