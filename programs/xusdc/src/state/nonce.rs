use anchor_lang::prelude::*;

#[account]
pub struct Nonce {
    pub expires_at: i64,
}
