import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Xusdc } from "../target/types/xusdc";
import { assert } from "chai";
import {
  Keypair,
  PublicKey,
  SystemProgram,
} from "@solana/web3.js";
import * as utils from "./utils";
import * as nacl from "tweetnacl";
import { TOKEN_2022_PROGRAM_ID, getAssociatedTokenAddressSync } from "@solana/spl-token";

describe("settle_payment", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Xusdc as Program<Xusdc>;
  
  // Test users
  let alice: Keypair;
  let bob: Keypair;
  let charlie: Keypair;
  
  // Test context
  let usdcMint: PublicKey;
  let xUsdcMint: Keypair;
  let programAuthority: PublicKey;
  let programState: PublicKey;
  let vault: PublicKey;
  let rentPool: PublicKey;
  
  // Facilitator keypair for testing
  let facilitator: Keypair;
  
  before(async () => {
    // Create test users
    alice = await utils.createTestUser(provider);
    bob = await utils.createTestUser(provider);
    charlie = await utils.createTestUser(provider);
    facilitator = await utils.createTestUser(provider);
    
    // Create mock USDC
    usdcMint = await utils.createMockUsdc(provider);
    
    // Derive PDAs
    [programAuthority] = utils.getProgramAuthority(program.programId);
    [programState] = utils.getProgramState(program.programId);
    [vault] = utils.getVault(program.programId, usdcMint);
    [rentPool] = utils.getRentPool(program.programId);
    
    // Create xUSDC mint with permanent delegate
    xUsdcMint = await utils.createXUsdcMint(provider, programAuthority);
    
    // Initialize program
    await program.methods
      .initialize()
      .accounts({
        authority: provider.wallet.publicKey,
        systemProgram: SystemProgram.programId,
        tokenProgram: TOKEN_2022_PROGRAM_ID,
        xUsdcMint: xUsdcMint.publicKey,
        usdcMint: usdcMint,
      })
      .rpc();
      
    // Initialize xUSDC metadata
    await utils.initializeXUsdcMetadata(provider, xUsdcMint.publicKey, programAuthority);
    
    // Create token accounts and fund them
    const aliceUsdcAta = await utils.createTokenAccount(provider, usdcMint, alice.publicKey);
    const bobUsdcAta = await utils.createTokenAccount(provider, usdcMint, bob.publicKey);
    
    await utils.mintTokensTo(provider, usdcMint, aliceUsdcAta, 1000);
    await utils.mintTokensTo(provider, usdcMint, bobUsdcAta, 1000);
    
    // Deposit USDC to get xUSDC
    const aliceXUsdcAta = await utils.createTokenAccount(provider, xUsdcMint.publicKey, alice.publicKey, TOKEN_2022_PROGRAM_ID);
    const bobXUsdcAta = await utils.createTokenAccount(provider, xUsdcMint.publicKey, bob.publicKey, TOKEN_2022_PROGRAM_ID);
    
    await program.methods
      .deposit(new anchor.BN(500 * 1e6))
      .accounts({
        user: alice.publicKey,
        usdcMint: usdcMint,
        xUsdcMint: xUsdcMint.publicKey,
        userUsdcAta: aliceUsdcAta,
        userXUsdcAta: aliceXUsdcAta,
        vault: vault,
        tokenProgram: anchor.utils.token.TOKEN_PROGRAM_ID,
        token2022Program: TOKEN_2022_PROGRAM_ID,
        systemProgram: SystemProgram.programId,
      })
      .signers([alice])
      .rpc();
  });

  describe("Valid payment authorization", () => {
    it("should successfully settle a payment with valid ed25519 signature", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(50 * 1e6); // 50 USDC
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) + 3600); // Valid for 1 hour
      
      // Create payment authorization
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Serialize payment authorization using Anchor's borsh serialization
      const paymentAuthSerialized = Buffer.concat([
        paymentAuth.from.toBuffer(),
        paymentAuth.to.toBuffer(),
        paymentAuth.amount.toArrayLike(Buffer, "le", 8),
        paymentAuth.nonce,
        paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
      ]);
      
      // Sign with Alice's keypair
      const signature = nacl.sign.detached(paymentAuthSerialized, alice.secretKey);
      
      // Get token accounts
      const aliceXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        alice.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      const bobXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        bob.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      // Get initial balances
      const aliceBalanceBefore = await utils.getTokenBalance(provider, aliceXUsdcAta, TOKEN_2022_PROGRAM_ID);
      const bobBalanceBefore = await utils.getTokenBalance(provider, bobXUsdcAta, TOKEN_2022_PROGRAM_ID);
      
      // Create nonce contributor account for rent
      await program.methods
        .contributeRent(new anchor.BN(10 * anchor.web3.LAMPORTS_PER_SOL))
        .accounts({
          user: facilitator.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([facilitator])
        .rpc();
      
      // Settle payment
      await program.methods
        .settlePayment({
          paymentAuth: {
            from: paymentAuth.from,
            to: paymentAuth.to,
            amount: paymentAuth.amount,
            nonce: Array.from(paymentAuth.nonce),
            validUntil: paymentAuth.validUntil,
          },
          signature: Array.from(signature),
          signerPubkey: Array.from(alice.publicKey.toBuffer()),
        })
        .accounts({
          facilitator: facilitator.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          xUsdcMint: xUsdcMint.publicKey,
          fromUserXUsdcAta: aliceXUsdcAta,
          toUserXUsdcAta: bobXUsdcAta,
          transferAuthority: programAuthority,
        })
        .signers([facilitator])
        .rpc();
      
      // Verify balances changed correctly
      const aliceBalanceAfter = await utils.getTokenBalance(provider, aliceXUsdcAta, TOKEN_2022_PROGRAM_ID);
      const bobBalanceAfter = await utils.getTokenBalance(provider, bobXUsdcAta, TOKEN_2022_PROGRAM_ID);
      
      assert.equal(
        aliceBalanceBefore.sub(aliceBalanceAfter).toString(),
        amount.toString(),
        "Alice's balance should decrease by payment amount"
      );
      assert.equal(
        bobBalanceAfter.sub(bobBalanceBefore).toString(),
        amount.toString(),
        "Bob's balance should increase by payment amount"
      );
      
      // Verify nonce was created
      const [noncePda] = utils.getNoncePda(program.programId, nonce);
      const nonceAccount = await provider.connection.getAccountInfo(noncePda);
      assert.isNotNull(nonceAccount, "Nonce account should be created");
    });
  });

  describe("Invalid signature scenarios", () => {
    it("should reject payment with invalid signature", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(10 * 1e6);
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
      
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Create invalid signature (random bytes)
      const invalidSignature = nacl.randomBytes(64);
      
      const aliceXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        alice.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      const bobXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        bob.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      try {
        await program.methods
          .settlePayment({
            paymentAuth: {
              from: paymentAuth.from,
              to: paymentAuth.to,
              amount: paymentAuth.amount,
              nonce: Array.from(paymentAuth.nonce),
              validUntil: paymentAuth.validUntil,
            },
            signature: Array.from(invalidSignature),
            signerPubkey: Array.from(alice.publicKey.toBuffer()),
          })
          .accounts({
            facilitator: facilitator.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            xUsdcMint: xUsdcMint.publicKey,
            fromUserXUsdcAta: aliceXUsdcAta,
            toUserXUsdcAta: bobXUsdcAta,
            transferAuthority: programAuthority,
          })
          .signers([facilitator])
          .rpc();
        assert.fail("Should have failed with invalid signature");
      } catch (error) {
        assert.include(error.toString(), "InvalidSignature");
      }
    });

    it("should reject payment signed by wrong keypair", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(10 * 1e6);
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
      
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Serialize payment authorization
      const paymentAuthSerialized = Buffer.concat([
        paymentAuth.from.toBuffer(),
        paymentAuth.to.toBuffer(),
        paymentAuth.amount.toArrayLike(Buffer, "le", 8),
        paymentAuth.nonce,
        paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
      ]);
      
      // Sign with Bob's keypair instead of Alice's
      const signature = nacl.sign.detached(paymentAuthSerialized, bob.secretKey);
      
      const aliceXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        alice.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      const bobXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        bob.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      try {
        await program.methods
          .settlePayment({
            paymentAuth: {
              from: paymentAuth.from,
              to: paymentAuth.to,
              amount: paymentAuth.amount,
              nonce: Array.from(paymentAuth.nonce),
              validUntil: paymentAuth.validUntil,
            },
            signature: Array.from(signature),
            signerPubkey: Array.from(bob.publicKey.toBuffer()), // Wrong signer
          })
          .accounts({
            facilitator: facilitator.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            xUsdcMint: xUsdcMint.publicKey,
            fromUserXUsdcAta: aliceXUsdcAta,
            toUserXUsdcAta: bobXUsdcAta,
            transferAuthority: programAuthority,
          })
          .signers([facilitator])
          .rpc();
        assert.fail("Should have failed with unauthorized signer");
      } catch (error) {
        assert.include(error.toString(), "UnauthorizedSigner");
      }
    });
  });

  describe("Expired payment validation", () => {
    it("should reject expired payment authorization", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(10 * 1e6);
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) - 3600); // Expired 1 hour ago
      
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Serialize and sign
      const paymentAuthSerialized = Buffer.concat([
        paymentAuth.from.toBuffer(),
        paymentAuth.to.toBuffer(),
        paymentAuth.amount.toArrayLike(Buffer, "le", 8),
        paymentAuth.nonce,
        paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
      ]);
      const signature = nacl.sign.detached(paymentAuthSerialized, alice.secretKey);
      
      const aliceXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        alice.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      const bobXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        bob.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      try {
        await program.methods
          .settlePayment({
            paymentAuth: {
              from: paymentAuth.from,
              to: paymentAuth.to,
              amount: paymentAuth.amount,
              nonce: Array.from(paymentAuth.nonce),
              validUntil: paymentAuth.validUntil,
            },
            signature: Array.from(signature),
            signerPubkey: Array.from(alice.publicKey.toBuffer()),
          })
          .accounts({
            facilitator: facilitator.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            xUsdcMint: xUsdcMint.publicKey,
            fromUserXUsdcAta: aliceXUsdcAta,
            toUserXUsdcAta: bobXUsdcAta,
            transferAuthority: programAuthority,
          })
          .signers([facilitator])
          .rpc();
        assert.fail("Should have failed with expired payment");
      } catch (error) {
        assert.include(error.toString(), "PaymentExpired");
      }
    });
  });

  describe("Account validation", () => {
    it("should reject payment with mismatched from account", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(10 * 1e6);
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
      
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Serialize and sign
      const paymentAuthSerialized = Buffer.concat([
        paymentAuth.from.toBuffer(),
        paymentAuth.to.toBuffer(),
        paymentAuth.amount.toArrayLike(Buffer, "le", 8),
        paymentAuth.nonce,
        paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
      ]);
      const signature = nacl.sign.detached(paymentAuthSerialized, alice.secretKey);
      
      // Use Charlie's account instead of Alice's
      const charlieXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        charlie.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      const bobXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        bob.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      try {
        await program.methods
          .settlePayment({
            paymentAuth: {
              from: paymentAuth.from,
              to: paymentAuth.to,
              amount: paymentAuth.amount,
              nonce: Array.from(paymentAuth.nonce),
              validUntil: paymentAuth.validUntil,
            },
            signature: Array.from(signature),
            signerPubkey: Array.from(alice.publicKey.toBuffer()),
          })
          .accounts({
            facilitator: facilitator.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            xUsdcMint: xUsdcMint.publicKey,
            fromUserXUsdcAta: charlieXUsdcAta, // Wrong account
            toUserXUsdcAta: bobXUsdcAta,
            transferAuthority: programAuthority,
          })
          .signers([facilitator])
          .rpc();
        assert.fail("Should have failed with mismatched from account");
      } catch (error) {
        assert.include(error.toString(), "InvalidPaymentAuthorization");
      }
    });

    it("should reject payment with mismatched to account", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(10 * 1e6);
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
      
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Serialize and sign
      const paymentAuthSerialized = Buffer.concat([
        paymentAuth.from.toBuffer(),
        paymentAuth.to.toBuffer(),
        paymentAuth.amount.toArrayLike(Buffer, "le", 8),
        paymentAuth.nonce,
        paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
      ]);
      const signature = nacl.sign.detached(paymentAuthSerialized, alice.secretKey);
      
      const aliceXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        alice.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      // Use Charlie's account instead of Bob's
      const charlieXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        charlie.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      try {
        await program.methods
          .settlePayment({
            paymentAuth: {
              from: paymentAuth.from,
              to: paymentAuth.to,
              amount: paymentAuth.amount,
              nonce: Array.from(paymentAuth.nonce),
              validUntil: paymentAuth.validUntil,
            },
            signature: Array.from(signature),
            signerPubkey: Array.from(alice.publicKey.toBuffer()),
          })
          .accounts({
            facilitator: facilitator.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            xUsdcMint: xUsdcMint.publicKey,
            fromUserXUsdcAta: aliceXUsdcAta,
            toUserXUsdcAta: charlieXUsdcAta, // Wrong account
            transferAuthority: programAuthority,
          })
          .signers([facilitator])
          .rpc();
        assert.fail("Should have failed with mismatched to account");
      } catch (error) {
        assert.include(error.toString(), "InvalidPaymentAuthorization");
      }
    });
  });

  describe("Nonce replay protection", () => {
    it("should reject payment with already used nonce", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(10 * 1e6);
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
      
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Serialize and sign
      const paymentAuthSerialized = Buffer.concat([
        paymentAuth.from.toBuffer(),
        paymentAuth.to.toBuffer(),
        paymentAuth.amount.toArrayLike(Buffer, "le", 8),
        paymentAuth.nonce,
        paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
      ]);
      const signature = nacl.sign.detached(paymentAuthSerialized, alice.secretKey);
      
      const aliceXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        alice.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      const bobXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        bob.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      // First payment should succeed
      await program.methods
        .settlePayment({
          paymentAuth: {
            from: paymentAuth.from,
            to: paymentAuth.to,
            amount: paymentAuth.amount,
            nonce: Array.from(paymentAuth.nonce),
            validUntil: paymentAuth.validUntil,
          },
          signature: Array.from(signature),
          signerPubkey: Array.from(alice.publicKey.toBuffer()),
        })
        .accounts({
          facilitator: facilitator.publicKey,
          tokenProgram: TOKEN_2022_PROGRAM_ID,
          xUsdcMint: xUsdcMint.publicKey,
          fromUserXUsdcAta: aliceXUsdcAta,
          toUserXUsdcAta: bobXUsdcAta,
          transferAuthority: programAuthority,
        })
        .signers([facilitator])
        .rpc();
      
      // Second payment with same nonce should fail
      try {
        await program.methods
          .settlePayment({
            paymentAuth: {
              from: paymentAuth.from,
              to: paymentAuth.to,
              amount: paymentAuth.amount,
              nonce: Array.from(paymentAuth.nonce),
              validUntil: paymentAuth.validUntil,
            },
            signature: Array.from(signature),
            signerPubkey: Array.from(alice.publicKey.toBuffer()),
          })
          .accounts({
            facilitator: facilitator.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            xUsdcMint: xUsdcMint.publicKey,
            fromUserXUsdcAta: aliceXUsdcAta,
            toUserXUsdcAta: bobXUsdcAta,
            transferAuthority: programAuthority,
          })
          .signers([facilitator])
          .rpc();
        assert.fail("Should have failed with duplicate nonce");
      } catch (error) {
        assert.include(error.toString(), "NonceAlreadyUsed");
      }
    });
  });

  describe("Typed payload enforcement", () => {
    it("should reject payment if any field in typed payload is modified", async () => {
      const nonce = utils.createNonce();
      const amount = new anchor.BN(10 * 1e6);
      const validUntil = new anchor.BN(Math.floor(Date.now() / 1000) + 3600);
      
      const paymentAuth: utils.PaymentAuthorization = {
        from: alice.publicKey,
        to: bob.publicKey,
        amount: amount,
        nonce: nonce,
        validUntil: validUntil,
      };
      
      // Serialize and sign original payload
      const paymentAuthSerialized = Buffer.concat([
        paymentAuth.from.toBuffer(),
        paymentAuth.to.toBuffer(),
        paymentAuth.amount.toArrayLike(Buffer, "le", 8),
        paymentAuth.nonce,
        paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
      ]);
      const signature = nacl.sign.detached(paymentAuthSerialized, alice.secretKey);
      
      const aliceXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        alice.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      const bobXUsdcAta = getAssociatedTokenAddressSync(
        xUsdcMint.publicKey,
        bob.publicKey,
        false,
        TOKEN_2022_PROGRAM_ID
      );
      
      // Try to submit with modified amount
      try {
        await program.methods
          .settlePayment({
            paymentAuth: {
              from: paymentAuth.from,
              to: paymentAuth.to,
              amount: new anchor.BN(20 * 1e6), // Modified amount
              nonce: Array.from(paymentAuth.nonce),
              validUntil: paymentAuth.validUntil,
            },
            signature: Array.from(signature),
            signerPubkey: Array.from(alice.publicKey.toBuffer()),
          })
          .accounts({
            facilitator: facilitator.publicKey,
            tokenProgram: TOKEN_2022_PROGRAM_ID,
            xUsdcMint: xUsdcMint.publicKey,
            fromUserXUsdcAta: aliceXUsdcAta,
            toUserXUsdcAta: bobXUsdcAta,
            transferAuthority: programAuthority,
          })
          .signers([facilitator])
          .rpc();
        assert.fail("Should have failed with modified payload");
      } catch (error) {
        assert.include(error.toString(), "InvalidSignature");
      }
    });
  });
});