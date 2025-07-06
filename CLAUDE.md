# xUSDC Program Specification

## Overview
xUSDC is a Token-2022 mint on Solana that enables gasless micropayments for the x402 protocol. Users deposit USDC once to receive xUSDC, then can make payments by signing off-chain messages.

## Token Configuration

### Token-2022 Extensions Required
1. **Permanent Delegate**: Program PDA has unlimited transfer authority
2. **Metadata**: Protocol information and branding
3. **Transfer Hook** (Optional): For additional validation logic

### Authorities
- **Mint Authority**: Program PDA (for minting xUSDC on deposits)
- **Freeze Authority**: Disabled (set to None)
- **Permanent Delegate**: Program PDA

## Program Instructions

### Core Instructions

**`initialize()`**
- One-time setup, creates xUSDC mint with program as permanent delegate
- Sets program PDA as mint authority and permanent delegate
- Creates USDC vault PDA

**`deposit(amount)`**
- User deposits USDC, receives 1:1 xUSDC
- Transfers USDC to vault PDA
- Mints equivalent xUSDC to user

**`withdraw(amount)`**
- User burns xUSDC, receives 1:1 USDC back
- Burns xUSDC from user account
- Transfers USDC from vault to user

**`settle_payment(signature, payment_params)`**
- Verifies off-chain payment authorization
- Validates ed25519 signature
- Transfers xUSDC using permanent delegate authority
- Records nonce to prevent replay

**`emergency_pause()`**
- Admin function to halt all operations
- Only callable by program upgrade authority

### Data Structures

**Payment Authorization:**
```rust
struct PaymentAuthorization {
    from: Pubkey,
    to: Pubkey,
    amount: u64,
    nonce: [u8; 32],
    valid_until: i64,
}
```

**Program State:**
```rust
struct ProgramState {
    is_initialized: bool,
    is_paused: bool,
    xusdc_mint: Pubkey,
    usdc_mint: Pubkey,
    vault: Pubkey,
    admin: Pubkey,
}
```

### Account PDAs
- Program state: `[b"state"]`
- USDC vault: `[b"vault", usdc_mint]`
- Nonce tracker: `[b"nonce", nonce_bytes]`
- Program authority: `[b"authority"]`
- Rent pool: `[b"rent_pool"]`
- Rent contributor: `[b"rent_contributor", contributor_pubkey]`

## Nonce Management - Rolling Window

### How Rolling Window Works
Instead of keeping nonces forever, we only enforce uniqueness within a time window (e.g., 24 hours):

```
Time -->
[------ 24 hour window ------]
         ↑
    Current time
    
Nonces before window: Can be garbage collected
Nonces in window: Must be unique
Nonces after window: Invalid (future dated)
```

### Implementation

**Nonce PDA Structure:**
```rust
struct NonceAccount {
    expires_at: i64,  // When this nonce can be garbage collected
    // No other data needed - just existence check
}
```

**Modified `settle_payment` logic:**
```rust
// 1. Check payment isn't expired
if current_time > payment.valid_until {
    return Err("Payment expired");
}

// 2. Enforce maximum validity period
let max_allowed_expiry = current_time + MAX_VALIDITY_SECONDS; // e.g., 24 hours
if payment.valid_until > max_allowed_expiry {
    return Err("Payment validity exceeds maximum allowed period");
}

// 3. Check/create nonce PDA
let nonce_pda = [b"nonce", &payment.nonce];
if account_exists(nonce_pda) {
    return Err("Nonce already used");
}

// 4. Create minimal nonce account with payment's expiry time
create_nonce_account(nonce_pda, payment.valid_until);
```

## Rent Pool System

### New Instructions

**`contribute_rent(amount)`**
```rust
// Contributor deposits SOL to cover nonce creation costs
// Tracks contribution for later withdrawal
struct RentContributor {
    contributor: Pubkey,
    amount_contributed: u64,
    nonces_funded: u64,
}
```

**`withdraw_rent(amount)`**
```rust
// Contributors can withdraw unused rent contributions
// amount <= (amount_contributed - nonces_funded * NONCE_RENT_COST)
```

**`garbage_collect(nonces: Vec<Pubkey>)`**
```rust
// Anyone can call to clean up expired nonces
// Receives small reward from rent pool
for nonce_pda in nonces {
    let nonce_account = get_account(nonce_pda);
    
    if current_time > nonce_account.expires_at {
        // Close account, recover rent
        close_account(nonce_pda);
        recovered_rent += account.lamports;
    }
}

// Reward caller with portion of recovered rent
transfer_sol(caller, recovered_rent * GARBAGE_COLLECT_REWARD_RATE);
```

### Updated Program State
```rust
struct ProgramState {
    is_initialized: bool,
    is_paused: bool,
    xusdc_mint: Pubkey,
    usdc_mint: Pubkey,
    vault: Pubkey,
    admin: Pubkey,
    
    // Rent pool tracking
    rent_pool_balance: u64,
    total_nonces_created: u64,
    nonce_rent_cost: u64, // Cost per nonce in lamports
    rolling_window_seconds: i64, // Default: 86400 (24 hours)
    gc_reward_rate: u8, // Percentage of recovered rent for GC caller
}
```

### Constants
```rust
const NONCE_ACCOUNT_SIZE: usize = 8; // Just expiry timestamp
const NONCE_RENT_COST: u64 = 890880; // ~0.0009 SOL
const MAX_VALIDITY_SECONDS: i64 = 86400; // 24 hours max
const GC_REWARD_RATE: u8 = 10; // 10% of recovered rent
```

## Service Architecture

### Off-chain Garbage Collector Service
```python
# Runs periodically (e.g., every hour)
async def garbage_collect():
    # 1. Query all nonce PDAs
    nonces = await get_program_accounts(
        filters=[nonce_account_filter]
    )
    
    # 2. Filter expired nonces
    current_time = time.time()
    expired = [
        n for n in nonces 
        if current_time > n.expires_at
    ]
    
    # 3. Batch garbage collect (max 10 per tx)
    for batch in chunks(expired, 10):
        await program.garbage_collect(batch)
```

### Benefits
1. **Bounded storage**: Nonces older than window get cleaned up
2. **Rent recycling**: Recovered rent funds new nonces
3. **Incentivized cleanup**: GC callers earn rewards
4. **No permanent storage growth**: System is sustainable

## Integration Points

### For x402 Clients
1. Check xUSDC balance before payments
2. Sign payment authorization messages
3. No on-chain interaction needed for payments

### For Resource Servers
1. Provide Solana wallet address for payments
2. Call facilitator API with payment data
3. No direct blockchain interaction required

### For Facilitator Service
1. Verify payment signatures
2. Submit transactions using permanent delegate
3. Track nonces to prevent replay attacks
4. Handle RPC errors and retry logic

## Security Considerations

1. **Permanent Delegate Risk**: Facilitator has full control - must be secured with multisig or HSM
2. **Audit Focus Areas**:
   - Signature verification logic
   - Nonce uniqueness enforcement
   - Integer overflow/underflow in amounts
   - Reentrancy in deposit/withdraw
3. **Emergency Controls**: Include pause functionality for incident response

## Testing Requirements

1. **Unit Tests**:
   - All program instructions
   - Edge cases (zero amounts, expired signatures)
   - Signature validation

2. **Integration Tests**:
   - Full deposit → payment → withdraw flow
   - Concurrent payment processing
   - RPC failure handling

3. **Security Tests**:
   - Replay attack prevention
   - Invalid signature rejection
   - Authority validation

## Deployment Checklist

- [ ] Deploy escrow program
- [ ] Initialize xUSDC mint with permanent delegate
- [ ] Set up facilitator keypair/multisig
- [ ] Deploy facilitator program
- [ ] Create monitoring for vault balance
- [ ] Set up nonce tracking infrastructure
- [ ] Test on devnet with real scenarios
- [ ] Audit programs before mainnet
- [ ] Prepare emergency response procedures