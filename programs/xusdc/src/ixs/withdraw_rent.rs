use anchor_lang::prelude::*;
use anchor_lang::system_program::{Transfer, transfer};

use crate::error::ErrorCode;
use crate::state::ContributorRentInfo;
use crate::state::{RENT_CONTRIBUTOR_SEED, RENT_POOL_SEED};

#[derive(Accounts)]
pub struct WithdrawRent<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut, 
        has_one = user, 
        seeds = [RENT_CONTRIBUTOR_SEED, &user.key().to_bytes()],
        bump
    )]
    pub user_rent_info: Account<'info, ContributorRentInfo>,

    /// CHECK: This is the global rent pool PDA
    #[account(
        mut, 
        seeds = [RENT_POOL_SEED], 
        bump
    )]
    pub global_rent_pool: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<WithdrawRent>, amt: u64) -> Result<()> {
    let user_rent_info = &mut ctx.accounts.user_rent_info;
    let global_rent_pool = &mut ctx.accounts.global_rent_pool;

    let global_rent_pool_bump = ctx.bumps.global_rent_pool;

    if amt > user_rent_info.amount {
        return Err(ErrorCode::InsufficientFunds.into());
    }

    user_rent_info.amount -= amt;

    // System transfer lamports from global rent pool to user
    transfer(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: global_rent_pool.to_account_info(),
                to: ctx.accounts.user.to_account_info(),
            },
            &[&[RENT_POOL_SEED, &[global_rent_pool_bump]]],
        ),
        amt,
    )?;

    Ok(())
}
