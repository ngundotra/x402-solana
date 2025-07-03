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
}
