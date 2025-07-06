use anchor_lang::{prelude::*, system_program};
use anchor_spl::{
    associated_token::{self, create_idempotent, AssociatedToken, Create},
    token_2022::{
        self,
        spl_token_2022::{
            extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
            instruction::AuthorityType,
            state::Mint as MintState,
        },
        Token2022,
    },
    token_2022_extensions,
};
// use litesvm_token::CreateAssociatedTokenAccountIdempotent;
// use litesvm_token::spl_token::extension::permanent_delegate::PermanentDelegate;
use std::mem::size_of;

use crate::state::{ADMIN_KEY, TRANSFER_AUTHORITY_SEED, XUSDC_MINT_KEY};

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut, address = ADMIN_KEY)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token2022>,
    /// CHECK: This is the xUSDC mint that we will initialize with token extensions
    #[account(mut,address = XUSDC_MINT_KEY)]
    pub xusdc_mint: Signer<'info>,
    /// CHECK: This is the xUSDC global ATA that we will initialize with token extensions
    pub xusdc_global_ata: UncheckedAccount<'info>,
    /// CHECK: This is the transfer authority
    #[account(seeds = [TRANSFER_AUTHORITY_SEED], bump)]
    pub transfer_authority: UncheckedAccount<'info>,
    pub associated_token_program: Program<'info, AssociatedToken>,
}

pub fn handler(ctx: Context<Initialize>) -> Result<()> {
    // let space: u64 = 0x52 + 0x24;
    let space = 0xca;
    // let space =
    // size_of<token_2022_extensions::<Token2022>>(&[ExtensionType::PermanentDelegate]) as u64;
    // token_2022_extensions::size_of::<Token2022>([PermanentDelegate::EXTENSION_SEED]) as u64;
    // let space = StateWithExtensions::<MintState>::size_of();
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

    msg!(
        "Creating global ATA: {:?}, {:?}, {:?}, {:?}, {:?}, {:?}, {:?}",
        ctx.accounts.xusdc_global_ata.key(),
        ctx.accounts.transfer_authority.key(),
        ctx.accounts.xusdc_mint.key(),
        ctx.accounts.authority.key(),
        ctx.accounts.associated_token_program.key(),
        ctx.accounts.token_program.key(),
        ctx.accounts.system_program.key(),
    );

    let expected_ata = anchor_spl::associated_token::get_associated_token_address_with_program_id(
        &ctx.accounts.transfer_authority.key(),
        &ctx.accounts.xusdc_mint.key(),
        &Token2022::id(),
    );
    require_eq!(ctx.accounts.xusdc_global_ata.key(), expected_ata);

    associated_token::create_idempotent(CpiContext::new(
        ctx.accounts.associated_token_program.to_account_info(),
        Create {
            payer: ctx.accounts.authority.to_account_info(),
            associated_token: ctx.accounts.xusdc_global_ata.to_account_info(),
            authority: ctx.accounts.transfer_authority.to_account_info(),
            mint: ctx.accounts.xusdc_mint.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
            token_program: ctx.accounts.token_program.to_account_info(),
        },
    ))?;

    Ok(())
}
