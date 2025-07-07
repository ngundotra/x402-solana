use anchor_lang::prelude::*;
use brine_ed25519::sig_verify;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TransferChecked};

use crate::state::{NonceAccount, NONCE_SEED, RENT_POOL_SEED, TRANSFER_AUTHORITY_SEED};

#[derive(Accounts)]
#[instruction(payload: SettlePayload)]
pub struct SettlePayment<'info> {
    pub facilitator: Signer<'info>,

    pub token_program: Program<'info, Token2022>,

    #[account(
        mint::token_program = token_program.key()
    )]
    pub xusdc_mint: InterfaceAccount<'info, Mint>,

    /// CHECK: This the authority/owner is the `from` pubkey in the payload
    #[account(mut, token::mint=xusdc_mint, token::token_program=token_program.key())]
    pub from_user_xusdc_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: this is the authority/owner of the `to` pubkey in the payload
    #[account(mut, token::mint=xusdc_mint, token::token_program=token_program.key())]
    pub to_user_xusdc_ata: InterfaceAccount<'info, TokenAccount>,

    /// CHECK: PDA used as transfer authority
    #[account(seeds = [TRANSFER_AUTHORITY_SEED], bump)]
    pub transfer_authority: AccountInfo<'info>,

    /// CHECK: PDA used as nonce account
    #[account(
        mut,
        seeds = [NONCE_SEED, payload.payment_auth.nonce.as_ref()],
        bump
    )]
    pub nonce_account: AccountInfo<'info>,

    /// CHECK: Global rent pool that funds nonce account creation
    #[account(mut, seeds = [RENT_POOL_SEED], bump)]
    pub rent_pool: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PaymentAuthorization {
    pub from: Pubkey,
    pub to: Pubkey,
    pub amount: u64,
    pub nonce: [u8; 32],
    pub valid_until: i64,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct SettlePayload {
    pub payment_auth: PaymentAuthorization,
    pub signature: [u8; 64],
    pub signer_pubkey: [u8; 32],
}

pub fn settle_payment(ctx: Context<SettlePayment>, payload: SettlePayload) -> Result<()> {
    let payment_auth = &payload.payment_auth;

    require!(
        ctx.accounts.nonce_account.data_is_empty(),
        ErrorCode::NonceAlreadyUsed
    );

    // payer = rent_pool,
    // Verify the payment authorization matches the provided accounts
    require!(
        payment_auth.from == ctx.accounts.from_user_xusdc_ata.owner,
        ErrorCode::InvalidPaymentAuthorization
    );
    require!(
        payment_auth.to == ctx.accounts.to_user_xusdc_ata.owner,
        ErrorCode::InvalidPaymentAuthorization
    );

    // Check payment hasn't expired
    let current_timestamp = Clock::get()?.unix_timestamp;
    require!(
        current_timestamp <= payment_auth.valid_until,
        ErrorCode::PaymentExpired
    );

    // Serialize the payment authorization for signature verification
    let message = payment_auth.try_to_vec()?;

    msg!("Verifying signature");

    // Verify the ed25519 signature
    sig_verify(&payload.signer_pubkey, &payload.signature, &message)
        .map_err(|_| error!(ErrorCode::InvalidSignature))?;
    msg!("Verified signature");

    // Verify the signer is the from account
    let signer_pubkey = Pubkey::from(payload.signer_pubkey);
    require!(
        signer_pubkey == payment_auth.from,
        ErrorCode::UnauthorizedSigner
    );

    // Transfer xUSDC using permanent delegate authority
    let transfer_authority_bump = ctx.bumps.transfer_authority;
    let transfer_authority_seeds = [TRANSFER_AUTHORITY_SEED, &[transfer_authority_bump]];
    let signer_seeds = &[&transfer_authority_seeds[..]];

    transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            TransferChecked {
                from: ctx.accounts.from_user_xusdc_ata.to_account_info(),
                to: ctx.accounts.to_user_xusdc_ata.to_account_info(),
                mint: ctx.accounts.xusdc_mint.to_account_info(),
                authority: ctx.accounts.transfer_authority.to_account_info(),
            },
            signer_seeds,
        ),
        payment_auth.amount,
        ctx.accounts.xusdc_mint.decimals,
    )?;

    // The nonce account was created and funded by the global rent pool
    let rent_bump = ctx.bumps.rent_pool;
    let nonce_bump = ctx.bumps.nonce_account;
    let space = 8 + std::mem::size_of::<NonceAccount>();
    anchor_lang::system_program::create_account(
        CpiContext::new_with_signer(
            ctx.accounts.system_program.to_account_info(),
            anchor_lang::system_program::CreateAccount {
                from: ctx.accounts.rent_pool.to_account_info(),
                to: ctx.accounts.nonce_account.to_account_info(),
            },
            &[
                &[RENT_POOL_SEED, &[rent_bump]],
                &[NONCE_SEED, &payload.payment_auth.nonce, &[nonce_bump]],
            ],
        ),
        Rent::get()?.minimum_balance(space),
        space as u64,
        &crate::ID,
    )?;

    // Update nonce account with expiry time
    let mut nonce_data = ctx.accounts.nonce_account.try_borrow_mut_data()?;
    let mut nonce_account = NonceAccount::DISCRIMINATOR.to_vec();
    nonce_account.extend_from_slice(
        &NonceAccount {
            expires_at: payment_auth.valid_until,
        }
        .try_to_vec()?,
    );

    nonce_data[0..nonce_account.len()].copy_from_slice(&nonce_account);

    Ok(())
}

#[error_code]
pub enum ErrorCode {
    #[msg("Invalid payment authorization")]
    InvalidPaymentAuthorization,
    #[msg("Payment has expired")]
    PaymentExpired,
    #[msg("Invalid signature")]
    InvalidSignature,
    #[msg("Invalid public key")]
    InvalidPublicKey,
    #[msg("Unauthorized signer")]
    UnauthorizedSigner,
    #[msg("Nonce already used")]
    NonceAlreadyUsed,
    #[msg("Arithmetic overflow")]
    Overflow,
}
