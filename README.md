# xUSDC - Gasless Micropayments on Solana

xUSDC is a Token-2022 program that enables gasless micropayments for the x402 protocol on Solana. It implements the EIP-3009 standard (Transfer With Authorization) adapted for Solana, allowing users to deposit USDC once to receive xUSDC tokens, then make payments by signing off-chain messages without paying transaction fees.

## Key Features

- **EIP-3009 Compatible**: Implements the Transfer With Authorization standard on Solana
- **1:1 USDC Backing**: Every xUSDC is backed by real USDC in the program vault
- **Gasless Payments**: Make payments by signing messages off-chain - no SOL needed
- **Permanent Delegate**: Program has transfer authority for seamless payment processing
- **Replay Protection**: Rolling window nonce system prevents double-spending
- **Rent Recycling**: Automated garbage collection keeps storage costs sustainable

## How It Works

1. **Deposit**: Users deposit USDC and receive xUSDC tokens 1:1
2. **Pay**: Sign payment authorizations off-chain (no gas fees)
3. **Settle**: Facilitator service submits transactions using permanent delegate
4. **Withdraw**: Burn xUSDC to get USDC back anytime

## Program Instructions

### Core Operations
- `initialize()` - One-time setup of the xUSDC mint and program state
- `deposit(amount)` - Convert USDC to xUSDC
- `withdraw(amount)` - Convert xUSDC back to USDC
- `settle_payment(payload: SettlePayload)` - Process off-chain payment authorization (EIP-3009 compatible)
- `emergency_pause()` - Admin function to halt operations

### Rent Management
- `contribute_rent(amount)` - Add SOL to cover nonce storage costs
- `withdraw_rent(amount)` - Reclaim unused rent contributions
- `garbage_collect(nonces)` - Clean up expired nonces for rewards

## Architecture

```
┌─────────────┐     ┌──────────────┐     ┌─────────────┐
│    User     │────▶│  Facilitator │────▶│   xUSDC     │
│   Wallet    │     │   Service    │     │   Program   │
└─────────────┘     └──────────────┘     └─────────────┘
       │                                         │
       │                                         ▼
       │                                  ┌─────────────┐
       └─────────── Deposits/Withdrawals─▶│ USDC Vault  │
                                          └─────────────┘
```

## Payment Flow (EIP-3009 Compatible)

1. User signs payment authorization off-chain:
   ```rust
   PaymentAuthorization {
     from: user_wallet,      // Signer's pubkey
     to: recipient_wallet,   // Recipient's pubkey
     amount: 1000000,        // 1 USDC in base units
     nonce: [u8; 32],        // Random 32 bytes
     valid_until: i64,       // Unix timestamp
   }
   ```

2. Recipient sends authorization with Ed25519 signature to facilitator

3. Facilitator submits `settle_payment` with:
   ```rust
   SettlePayload {
     payment_auth: PaymentAuthorization,
     signature: [u8; 64],      // Ed25519 signature
     signer_pubkey: [u8; 32],  // Must match 'from' field
   }
   ```

4. Program verifies signature and transfers xUSDC using permanent delegate authority

## Security Features

- **Ed25519 Signature Verification**: All payments require valid signatures
- **Nonce Uniqueness**: Each payment can only be processed once
- **Time-Bounded Validity**: Payments expire after 24 hours maximum
- **Emergency Pause**: Admin can halt all operations if needed

## Development

### Prerequisites
- Rust 1.75+
- Solana CLI 1.18+
- Anchor Framework 0.30+

### Building
```bash
# Build the program
anchor build

# Run tests
anchor test
```

### Testing
The test suite covers:
- Initialization and token setup
- Deposit/withdraw flows
- Payment settlement with signature verification
- Nonce management and garbage collection
- Edge cases and security scenarios

## Deployment

### Mainnet Addresses
- Program: `[TBD]`
- xUSDC Mint: `[TBD]`
- USDC Vault: `[TBD]`

### Devnet Testing
1. Deploy program: `anchor deploy --provider.cluster devnet`
2. Initialize: `anchor run initialize --provider.cluster devnet`
3. Follow test scripts in `tests/` directory

## Integration

### For dApp Developers
```javascript
// Deposit USDC to get xUSDC
await program.methods
  .deposit(new BN(1_000_000))
  .accounts({...})
  .rpc();

// Sign payment off-chain
const payment = {
  from: wallet.publicKey,
  to: recipientPubkey,
  amount: new BN(1_000_000),
  nonce: randomBytes(32),
  validUntil: Date.now() + 3600
};

const signature = await wallet.signMessage(payment);
// Send to facilitator API
```

### For Facilitator Services
See `facilitator/README.md` for API documentation and integration guide.

## License

Apache 2.0 - See LICENSE file for details