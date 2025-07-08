# xUSDC - Pay for Anything on the Internet with USDC

xUSDC brings the simplicity of EIP-3009 (gasless USDC transfers) to Solana. Just buy xUSDC once, and pay for anything online without worrying about gas fees, blockchain complexity, or transaction signing.

## For Users: It's This Simple

1. **Buy xUSDC** - Swap your USDC for xUSDC (1:1 exchange)
2. **Transfer to a burner wallet** - Keep your browsing private
3. **Pay for anything** - Just click "pay with xUSDC" on any website

That's it. No gas fees. No popups. No blockchain knowledge required.

## Why xUSDC?

- **Zero Gas Fees**: Pay $0.10 for something that costs $0.10
- **Instant Payments**: No waiting for confirmations
- **Works Everywhere**: Any website can accept xUSDC payments
- **Complete Privacy**: Use burner wallets for anonymous browsing
- **EIP-3009 Standard**: The same trusted gasless payment system used on Ethereum

## How It Works (For the Curious)

When you pay with xUSDC, you're just signing a message - like signing a check. The website handles all the blockchain stuff for you. Your xUSDC stays in your wallet until the exact moment of payment.

## Getting Started

### Where to Get xUSDC
- **DEXs**: Swap USDC → xUSDC on Jupiter, Raydium, or Orca
- **Direct Deposit**: Use any xUSDC-enabled app to convert USDC
- **Liquidity Pools**: Facilitators maintain USDC/xUSDC pools for instant swaps

### Using xUSDC
1. Install any Solana wallet (Phantom, Solflare, etc.)
2. Buy/swap for xUSDC
3. Send some to a fresh wallet for private browsing
4. Click "Pay with xUSDC" on any supported website

---

## For Facilitators & Developers

Facilitators handle the infrastructure so users don't have to think about blockchain mechanics. They manage liquidity pools, process payments, and handle all the technical complexity.

### EIP-3009 Implementation

xUSDC implements the Transfer With Authorization standard, enabling gasless transfers through signed messages:

```javascript
// User signs a payment authorization (happens automatically in wallets)
const authorization = {
  from: userWallet,
  to: merchantWallet,
  amount: 1000000, // $1.00 USDC
  nonce: randomBytes(32),
  validUntil: Date.now() + 3600
};
```

### Technical Architecture

```
User Experience:
┌─────────────┐
│    User     │ ← Just signs messages, no gas fees
│   Wallet    │
└─────────────┘

Behind the Scenes (handled by facilitators):
┌──────────────┐     ┌─────────────┐
│  Facilitator │────▶│   xUSDC     │
│   Service    │     │   Program   │
└──────────────┘     └─────────────┘
                            │
                            ▼
                     ┌─────────────┐
                     │ USDC Vault  │
                     └─────────────┘
```

### Core Program Instructions

- `deposit(amount)` - Convert USDC to xUSDC (1:1)
- `withdraw(amount)` - Convert xUSDC back to USDC
- `settle_payment(payload)` - Process EIP-3009 signed authorizations

### Rent & Infrastructure Management

Facilitators handle storage costs through a rent pool system:
- `contribute_rent()` - Add SOL to cover transaction costs
- `garbage_collect()` - Clean up old data for rewards

### Security

- **Signature Verification**: Ed25519 signatures prevent forgery
- **Replay Protection**: Each payment can only be processed once
- **Time Limits**: Payments expire after 24 hours
- **1:1 Backing**: Every xUSDC is backed by real USDC


### Building & Development

```bash
# Build
anchor build

# Test
cargo test --lib tests -- --nocapture
```

## License

Apache 2.0 - See LICENSE file for details
