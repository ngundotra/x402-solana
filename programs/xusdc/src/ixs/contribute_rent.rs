use anchor_lang::prelude::*;
use anchor_lang::system_program::{transfer, Transfer};

use crate::state::ContributorRentInfo;
use crate::state::{RENT_CONTRIBUTOR_SEED, RENT_POOL_SEED};

#[derive(Accounts)]

pub struct ContributeRent<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        init,
        seeds = [RENT_CONTRIBUTOR_SEED, &user.key().to_bytes()],
        bump,
        payer = user,
        space = 8 + std::mem::size_of::<ContributorRentInfo>(),
    )]
    pub user_rent_info: Account<'info, ContributorRentInfo>,

    /// CHECK: This is the global rent pool
    #[account(mut, seeds = [RENT_POOL_SEED], bump)]
    pub global_rent_pool: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
}

pub fn handler(ctx: Context<ContributeRent>, amount: u64) -> Result<()> {
    let global_rent_pool = &mut ctx.accounts.global_rent_pool;
    let user_rent_info = &mut ctx.accounts.user_rent_info;

    user_rent_info.amount += amount;

    // System transfer lamports to global rent pool
    transfer(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            Transfer {
                from: ctx.accounts.user.to_account_info(),
                to: global_rent_pool.to_account_info(),
            },
        ),
        amount,
    )?;

    Ok(())
}
