use anchor_lang::accounts::interface_account::InterfaceAccount;
use anchor_lang::prelude::*;
use anchor_spl::token::{self, Token};
use anchor_spl::token_2022::{self, Token2022};
use anchor_spl::token_interface::{Mint, TokenAccount};

use crate::state::{TRANSFER_AUTHORITY_SEED, USDC_MINT_KEY, XUSDC_MINT_KEY};

#[derive(Accounts)]
pub struct Withdraw<'info> {
    #[account(mut)]
    pub user: Signer<'info>,

    #[account(
        mut,
        token::mint=xusdc_mint,
        token::authority=user,
        token::token_program=token_program.key()
    )]
    pub user_xusdc_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint=usdc_mint,
        token::authority=user,
        token::token_program=tokenkeg.key()
    )]
    pub user_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mut,
        token::mint=usdc_mint,
        token::token_program=tokenkeg.key()
    )]
    pub global_usdc_ata: InterfaceAccount<'info, TokenAccount>,

    #[account(
        mint::token_program = tokenkeg.key(),
        address = USDC_MINT_KEY
    )]
    pub usdc_mint: InterfaceAccount<'info, Mint>,

    #[account(
        mut,
        mint::token_program = token_program.key(),
        address = XUSDC_MINT_KEY
    )]
    pub xusdc_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: PDA used as transfer authority
    #[account(seeds = [TRANSFER_AUTHORITY_SEED], bump)]
    pub transfer_authority: AccountInfo<'info>,

    pub tokenkeg: Program<'info, Token>,
    pub token_program: Program<'info, Token2022>,
}

/// Exchange xUSDC for USDC
pub fn handler(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
    let bump = ctx.bumps.transfer_authority;

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.tokenkeg.to_account_info(),
            token::TransferChecked {
                from: ctx.accounts.global_usdc_ata.to_account_info(),
                to: ctx.accounts.user_usdc_ata.to_account_info(),
                mint: ctx.accounts.usdc_mint.to_account_info(),
                authority: ctx.accounts.transfer_authority.to_account_info(),
            },
            &[&[TRANSFER_AUTHORITY_SEED, &[bump]]],
        ),
        amount,
        ctx.accounts.usdc_mint.decimals,
    )?;

    token_2022::burn_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            token_2022::BurnChecked {
                mint: ctx.accounts.xusdc_mint.to_account_info(),
                authority: ctx.accounts.transfer_authority.to_account_info(),
                from: ctx.accounts.user_xusdc_ata.to_account_info(),
            },
            &[&[TRANSFER_AUTHORITY_SEED, &[bump]]],
        ),
        amount,
        ctx.accounts.xusdc_mint.decimals,
    )?;

    Ok(())
}
