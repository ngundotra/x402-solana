use anchor_lang::{prelude::*, system_program};
use anchor_spl::{
    associated_token::{self, AssociatedToken, Create},
    token::Token,
    token_2022::{self, Token2022},
    token_2022_extensions,
    token_interface::Mint,
};

use crate::state::{ADMIN_KEY, TRANSFER_AUTHORITY_SEED, USDC_MINT_KEY, XUSDC_MINT_KEY};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut, address = ADMIN_KEY)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    /// CHECK: This is the xUSDC mint that we will initialize with token extensions
    #[account(mut, address = XUSDC_MINT_KEY)]
    pub xusdc_mint: Signer<'info>,
    #[account(
        mint::token_program = tokenkeg.key(),
        address = USDC_MINT_KEY
    )]
    pub usdc_mint: InterfaceAccount<'info, Mint>,
    /// CHECK: This is the xUSDC global ATA that we will initialize with token extensions
    pub usdc_global_ata: UncheckedAccount<'info>,
    /// CHECK: This is the transfer authority
    #[account(seeds = [TRANSFER_AUTHORITY_SEED], bump)]
    pub transfer_authority: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub tokenkeg: Program<'info, Token>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // Found this out by running the spl-token cli, creating a perma delegate mint
    // and checking the size of the mint account
    let space = 0xca;
    system_program::create_account(
        CpiContext::new(
            ctx.accounts.system_program.to_account_info(),
            system_program::CreateAccount {
                from: ctx.accounts.authority.to_account_info(),
                to: ctx.accounts.xusdc_mint.to_account_info(),
            },
        ),
        Rent::get()?.minimum_balance(space as usize),
        space,
        &Token2022::id(),
    )?;

    // Now we need to initialize the permanent delegate on the mint
    let cpi_context = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token_2022_extensions::PermanentDelegateInitialize {
            mint: ctx.accounts.xusdc_mint.to_account_info(),
            token_program_id: ctx.accounts.token_program.to_account_info(),
        },
    );
    token_2022_extensions::permanent_delegate_initialize(
        cpi_context,
        &ctx.accounts.transfer_authority.key(),
    )?;

    // Need to initialize the token 2022 mi nt
    let cpi_context = CpiContext::new(
        ctx.accounts.token_program.to_account_info(),
        token_2022::InitializeMint2 {
            mint: ctx.accounts.xusdc_mint.to_account_info(),
        },
    );
    token_2022::initialize_mint2(cpi_context, 6, &ctx.accounts.authority.key(), None)?;

    let expected_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &ctx.accounts.transfer_authority.key(),
        &ctx.accounts.usdc_mint.key(),
        &Token::id(),
    );
    require_eq!(ctx.accounts.usdc_global_ata.key(), expected_ata);

    associated_token::create_idempotent(CpiContext::new(
        ctx.accounts.associated_token_program.to_account_info(),
        Create {
            payer: ctx.accounts.authority.to_account_info(),
            associated_token: ctx.accounts.usdc_global_ata.to_account_info(),
            authority: ctx.accounts.transfer_authority.to_account_info(),
            mint: ctx.accounts.usdc_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.tokenkeg.to_account_info(),
        },
    ))?;

    Ok(())
}
