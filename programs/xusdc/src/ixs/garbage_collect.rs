use anchor_lang::{prelude::*};

use crate::error::ErrorCode;
use crate::state::{NonceAccount, RENT_POOL_SEED};

#[derive(Accounts)]
pub struct GarbageCollect<'info> {
    #[account(
        mut, 
        close = global_rent_pool
    )]
    pub nonce_account: Account<'info, NonceAccount>,
    /// CHECK: This is the global rent pool PDA
    #[account(mut, seeds = [RENT_POOL_SEED], bump)]
    pub global_rent_pool: AccountInfo<'info>,
}

pub fn handler<'info>(ctx: Context<'_, '_, '_, 'info, GarbageCollect<'info>>) -> Result<()> {
    require!(
        ctx.accounts.nonce_account.expires_at <= Clock::get()?.unix_timestamp,
        ErrorCode::NonceIsNotExpired
    );
    Ok(())
}
