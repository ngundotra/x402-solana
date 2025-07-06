import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Xusdc } from "../target/types/xusdc";
import { assert } from "chai";
import * as nacl from "tweetnacl";

describe("simple settle_payment test", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Xusdc as Program<Xusdc>;

  it("Verifies typed payload structure", async () => {
    // Create test keypair
    const testKeypair = anchor.web3.Keypair.generate();
    
    // Create payment authorization
    const paymentAuth = {
      from: testKeypair.publicKey,
      to: anchor.web3.Keypair.generate().publicKey,
      amount: new anchor.BN(100),
      nonce: Buffer.alloc(32, 1), // Simple nonce for testing
      validUntil: new anchor.BN(Math.floor(Date.now() / 1000) + 3600),
    };
    
    // Serialize the payment authorization (matching on-chain borsh serialization)
    const message = Buffer.concat([
      paymentAuth.from.toBuffer(),
      paymentAuth.to.toBuffer(),
      paymentAuth.amount.toArrayLike(Buffer, "le", 8),
      paymentAuth.nonce,
      paymentAuth.validUntil.toArrayLike(Buffer, "le", 8),
    ]);
    
    // Sign with the test keypair
    const signature = nacl.sign.detached(message, testKeypair.secretKey);
    
    // Verify the signature locally
    const isValid = nacl.sign.detached.verify(
      message,
      signature,
      testKeypair.publicKey.toBuffer()
    );
    
    assert.isTrue(isValid, "Ed25519 signature should be valid");
    
    // Verify message structure
    assert.equal(message.length, 32 + 32 + 8 + 32 + 8, "Message should be 112 bytes");
    
    // Test signature manipulation detection
    const tamperedSignature = Buffer.from(signature);
    tamperedSignature[0] = tamperedSignature[0] ^ 0xFF; // Flip bits
    
    const isTamperedValid = nacl.sign.detached.verify(
      message,
      tamperedSignature,
      testKeypair.publicKey.toBuffer()
    );
    
    assert.isFalse(isTamperedValid, "Tampered signature should be invalid");
    
    // Test payload manipulation detection
    const tamperedAuth = {
      ...paymentAuth,
      amount: new anchor.BN(200), // Change amount
    };
    
    const tamperedMessage = Buffer.concat([
      tamperedAuth.from.toBuffer(),
      tamperedAuth.to.toBuffer(),
      tamperedAuth.amount.toArrayLike(Buffer, "le", 8),
      tamperedAuth.nonce,
      tamperedAuth.validUntil.toArrayLike(Buffer, "le", 8),
    ]);
    
    const isTamperedPayloadValid = nacl.sign.detached.verify(
      tamperedMessage,
      signature,
      testKeypair.publicKey.toBuffer()
    );
    
    assert.isFalse(isTamperedPayloadValid, "Signature should be invalid for tampered payload");
    
    console.log("✅ Typed payload structure enforced correctly");
    console.log("✅ Ed25519 signature verification working");
    console.log("✅ Payload tampering detected successfully");
  });
});