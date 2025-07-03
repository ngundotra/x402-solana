use anchor_lang::prelude::*;

use crate::error::ErrorCode;
use crate::state::RENT_POOL_SEED;

#[derive(Accounts)]
pub struct GarbageCollect<'info> {
    /// CHECK: This is the global rent pool PDA
    #[account(mut, seeds = [RENT_POOL_SEED], bump)]
    pub global_rent_pool: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct Nonce {
    pub expires_at: i64,
}

pub fn handler(ctx: Context<GarbageCollect>) -> Result<()> {
    for nonce in ctx.remaining_accounts {
        let nonce_account = nonce;
        if nonce_account.data_is_empty() {
            return Err(ErrorCode::NonceDoesNotExist.into());
        }
        if !nonce_account.is_writable {
            return Err(ErrorCode::NonceIsNotWritable.into());
        }

        // Try to deserialize as Nonce account
        let nonce_data = Nonce::try_deserialize(&mut &nonce_account.data.borrow()[..])?;

        // Check if nonce has expired
        if nonce_data.expires_at > Clock::get()?.unix_timestamp {
            return Err(ErrorCode::NonceIsNotExpired.into());
        }

        // Close the nonce account and transfer lamports to global rent pool
        let nonce_lamports = nonce_account.lamports();
        **nonce_account.lamports.borrow_mut() = 0;
        **ctx.accounts.global_rent_pool.lamports.borrow_mut() += nonce_lamports;
    }
    Ok(())
}
