use anchor_lang::prelude::*;
use brine_ed25519::sig_verify;

use anchor_spl::token_2022::Token2022;
use anchor_spl::token_interface::{transfer_checked, Mint, TokenAccount, TransferChecked};

use crate::state::{NonceAccount, TRANSFER_AUTHORITY_SEED};

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

    #[account(
        init,
        payer = rent_pool,
        space = 8 + std::mem::size_of::<NonceAccount>(),
        seeds = [b"nonce", payload.payment_auth.nonce.as_ref()],
        bump
    )]
    pub nonce_account: Account<'info, NonceAccount>,

    /// CHECK: Global rent pool that funds nonce account creation
    #[account(mut, seeds = [b"rent_pool"], bump)]
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

    // Verify the ed25519 signature
    sig_verify(&payload.signer_pubkey, &payload.signature, &message)
        .map_err(|_| error!(ErrorCode::InvalidSignature))?;

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

    // Update nonce account with expiry time
    ctx.accounts.nonce_account.expires_at = payment_auth.valid_until;

    // The nonce account was created and funded by the global rent pool

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
