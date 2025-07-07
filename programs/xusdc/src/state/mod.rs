use anchor_lang::prelude::*;

mod contributor;
mod nonce;

pub use contributor::*;
pub use nonce::*;

// Alias for clarity
pub type NonceAccount = Nonce;

pub const RENT_POOL_SEED: &[u8] = b"global_rent_pool";

pub const ADMIN_KEY: Pubkey = pubkey!("CyJj5ejJAUveDXnLduJbkvwjxcmWJNqCuB9DR7AExrHn");
pub const XUSDC_MINT_KEY: Pubkey = pubkey!("xUSD1YCoHxQGvNRhaSGnACc8Rj7gTEB3LmCUxSPLSzM");
#[cfg(feature = "devnet")]
pub const USDC_MINT_KEY: Pubkey = pubkey!("4zMMC9srt5Ri5X14GAgXhaHii3GnPAEERYPJgZJDncDU");
#[cfg(not(feature = "devnet"))]
pub const USDC_MINT_KEY: Pubkey = pubkey!("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
pub const TRANSFER_AUTHORITY_SEED: &[u8] = b"transfer-authority";
pub const NONCE_SEED: &[u8] = b"nonce";
