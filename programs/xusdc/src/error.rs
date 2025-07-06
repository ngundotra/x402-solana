use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("Insufficient funds")]
    InsufficientFunds,
    #[msg("Nonce does not exist")]
    NonceDoesNotExist,
    #[msg("Nonce is not writable")]
    NonceIsNotWritable,
    #[msg("Nonce is not expired")]
    NonceIsNotExpired,
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
}
