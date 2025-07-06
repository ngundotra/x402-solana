use anchor_lang::prelude::*;

pub const RENT_CONTRIBUTOR_SEED: &[u8] = b"rent-contributor";

#[account]
pub struct ContributorRentInfo {
    pub amount: u64,
    pub user: Pubkey,
    pub nonces_funded: u64,
}
