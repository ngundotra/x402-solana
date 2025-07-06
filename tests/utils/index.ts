import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Xusdc } from "../../target/types/xusdc";
import {
  PublicKey,
  Keypair,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  SYSVAR_RENT_PUBKEY,
} from "@solana/web3.js";
import {
  TOKEN_2022_PROGRAM_ID,
  TOKEN_PROGRAM_ID,
  createInitializeMintInstruction,
  createInitializePermanentDelegateInstruction,
  createInitializeMetadataPointerInstruction,
  getMintLen,
  ExtensionType,
  createMint,
  createAccount,
  mintTo,
  getAccount,
  getMint,
  createAssociatedTokenAccountIdempotent,
  getAssociatedTokenAddressSync,
  ASSOCIATED_TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { createInitializeInstruction as createInitializeMetadataInstruction } from "@solana/spl-token-metadata";
import * as nacl from "tweetnacl";
import { assert } from "chai";

export interface TestContext {
  provider: anchor.AnchorProvider;
  program: Program<Xusdc>;
  admin: Keypair;
  usdcMint: PublicKey;
  xUsdcMint: PublicKey;
  programAuthority: PublicKey;
  programState: PublicKey;
  vault: PublicKey;
  rentPool: PublicKey;
}

export interface PaymentAuthorization {
  from: PublicKey;
  to: PublicKey;
  amount: anchor.BN;
  nonce: Buffer;
  validUntil: anchor.BN;
}

// PDA derivations
export function getProgramAuthority(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("authority")],
    programId
  );
}

export function getProgramState(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("state")],
    programId
  );
}

export function getVault(programId: PublicKey, usdcMint: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("vault"), usdcMint.toBuffer()],
    programId
  );
}

export function getNoncePda(programId: PublicKey, nonce: Buffer): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("nonce"), nonce],
    programId
  );
}

export function getRentPool(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("rent_pool")],
    programId
  );
}

export function getRentContributor(
  programId: PublicKey,
  contributor: PublicKey
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("rent_contributor"), contributor.toBuffer()],
    programId
  );
}

// Create mock USDC mint for testing
export async function createMockUsdc(
  provider: anchor.AnchorProvider,
  decimals: number = 6
): Promise<PublicKey> {
  const mint = await createMint(
    provider.connection,
    provider.wallet as anchor.Wallet,
    provider.wallet.publicKey,
    provider.wallet.publicKey,
    decimals,
    undefined,
    undefined,
    TOKEN_PROGRAM_ID
  );
  
  return mint;
}

// Create xUSDC mint with Token-2022 extensions
export async function createXUsdcMint(
  provider: anchor.AnchorProvider,
  programAuthority: PublicKey,
  decimals: number = 6
): Promise<Keypair> {
  const mintKeypair = Keypair.generate();
  
  // Calculate mint size with extensions
  const extensions = [
    ExtensionType.PermanentDelegate,
    ExtensionType.MetadataPointer,
  ];
  const mintLen = getMintLen(extensions);
  
  // Create mint account
  const lamports = await provider.connection.getMinimumBalanceForRentExemption(mintLen);
  
  const transaction = new Transaction().add(
    SystemProgram.createAccount({
      fromPubkey: provider.wallet.publicKey,
      newAccountPubkey: mintKeypair.publicKey,
      space: mintLen,
      lamports,
      programId: TOKEN_2022_PROGRAM_ID,
    }),
    // Initialize permanent delegate extension
    createInitializePermanentDelegateInstruction(
      mintKeypair.publicKey,
      programAuthority,
      TOKEN_2022_PROGRAM_ID
    ),
    // Initialize metadata pointer extension
    createInitializeMetadataPointerInstruction(
      mintKeypair.publicKey,
      programAuthority,
      mintKeypair.publicKey,
      TOKEN_2022_PROGRAM_ID
    ),
    // Initialize mint
    createInitializeMintInstruction(
      mintKeypair.publicKey,
      decimals,
      programAuthority,
      null, // No freeze authority
      TOKEN_2022_PROGRAM_ID
    )
  );
  
  await sendAndConfirmTransaction(
    provider.connection,
    transaction,
    [provider.wallet as anchor.Wallet, mintKeypair],
    { commitment: "confirmed" }
  );
  
  return mintKeypair;
}

// Initialize metadata for xUSDC
export async function initializeXUsdcMetadata(
  provider: anchor.AnchorProvider,
  mint: PublicKey,
  updateAuthority: PublicKey
): Promise<void> {
  const transaction = new Transaction().add(
    createInitializeMetadataInstruction({
      programId: TOKEN_2022_PROGRAM_ID,
      mint,
      metadata: mint,
      name: "xUSDC",
      symbol: "xUSDC",
      uri: "https://x402.io/xusdc",
      mintAuthority: updateAuthority,
      updateAuthority,
    })
  );
  
  await sendAndConfirmTransaction(
    provider.connection,
    transaction,
    [provider.wallet as anchor.Wallet],
    { commitment: "confirmed" }
  );
}

// Mint tokens to a user
export async function mintTokensTo(
  provider: anchor.AnchorProvider,
  mint: PublicKey,
  destination: PublicKey,
  amount: number,
  decimals: number = 6,
  programId: PublicKey = TOKEN_PROGRAM_ID
): Promise<void> {
  await mintTo(
    provider.connection,
    provider.wallet as anchor.Wallet,
    mint,
    destination,
    provider.wallet.publicKey,
    amount * Math.pow(10, decimals),
    [],
    { commitment: "confirmed" },
    programId
  );
}

// Create or get associated token account
export async function createTokenAccount(
  provider: anchor.AnchorProvider,
  mint: PublicKey,
  owner: PublicKey,
  programId: PublicKey = TOKEN_PROGRAM_ID
): Promise<PublicKey> {
  return await createAssociatedTokenAccountIdempotent(
    provider.connection,
    provider.wallet as anchor.Wallet,
    mint,
    owner,
    { commitment: "confirmed" },
    programId
  );
}

// Get token balance
export async function getTokenBalance(
  provider: anchor.AnchorProvider,
  tokenAccount: PublicKey,
  programId: PublicKey = TOKEN_PROGRAM_ID
): Promise<anchor.BN> {
  try {
    const account = await getAccount(
      provider.connection,
      tokenAccount,
      "confirmed",
      programId
    );
    return new anchor.BN(account.amount.toString());
  } catch (e) {
    return new anchor.BN(0);
  }
}

// Sign payment authorization
export function signPaymentAuthorization(
  payment: PaymentAuthorization,
  signerKeypair: Keypair
): Buffer {
  // Serialize payment data for signing
  const message = Buffer.concat([
    payment.from.toBuffer(),
    payment.to.toBuffer(),
    payment.amount.toArrayLike(Buffer, "le", 8),
    payment.nonce,
    payment.validUntil.toArrayLike(Buffer, "le", 8),
  ]);
  
  const signature = nacl.sign.detached(message, signerKeypair.secretKey);
  return Buffer.from(signature);
}

// Create random nonce
export function createNonce(): Buffer {
  return Buffer.from(nacl.randomBytes(32));
}

// Helper to airdrop SOL for testing
export async function airdropSol(
  provider: anchor.AnchorProvider,
  to: PublicKey,
  amount: number = 10
): Promise<void> {
  const signature = await provider.connection.requestAirdrop(
    to,
    amount * anchor.web3.LAMPORTS_PER_SOL
  );
  
  const latestBlockhash = await provider.connection.getLatestBlockhash();
  await provider.connection.confirmTransaction({
    signature,
    ...latestBlockhash,
  });
}

// Helper to advance time for testing nonce expiry
export async function advanceTime(
  provider: anchor.AnchorProvider,
  seconds: number
): Promise<void> {
  // This is a placeholder - in actual tests you might use clock syscall
  // or bankrun for time manipulation
  await new Promise(resolve => setTimeout(resolve, seconds * 1000));
}

// Helper to create test users
export async function createTestUser(
  provider: anchor.AnchorProvider,
  lamports: number = 10
): Promise<Keypair> {
  const user = Keypair.generate();
  await airdropSol(provider, user.publicKey, lamports);
  return user;
}

// Verify program state
export async function verifyProgramState(
  program: Program<Xusdc>,
  state: PublicKey,
  expected: {
    isInitialized?: boolean;
    isPaused?: boolean;
    xUsdcMint?: PublicKey;
    usdcMint?: PublicKey;
    vault?: PublicKey;
    admin?: PublicKey;
  }
): Promise<void> {
  const accountData = await program.account.programState.fetch(state);
  
  if (expected.isInitialized !== undefined) {
    assert.equal(accountData.isInitialized, expected.isInitialized, "Initialization state mismatch");
  }
  if (expected.isPaused !== undefined) {
    assert.equal(accountData.isPaused, expected.isPaused, "Pause state mismatch");
  }
  if (expected.xUsdcMint) {
    assert.equal(accountData.xUsdcMint.toString(), expected.xUsdcMint.toString(), "xUSDC mint mismatch");
  }
  if (expected.usdcMint) {
    assert.equal(accountData.usdcMint.toString(), expected.usdcMint.toString(), "USDC mint mismatch");
  }
  if (expected.vault) {
    assert.equal(accountData.vault.toString(), expected.vault.toString(), "Vault mismatch");
  }
  if (expected.admin) {
    assert.equal(accountData.admin.toString(), expected.admin.toString(), "Admin mismatch");
  }
}

// Constants
export const NONCE_ACCOUNT_SIZE = 8;
export const NONCE_RENT_COST = new anchor.BN(890880);
export const MAX_VALIDITY_SECONDS = new anchor.BN(86400); // 24 hours
export const GC_REWARD_RATE = 10; // 10%