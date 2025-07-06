import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Xusdc } from "../target/types/xusdc";
import { assert } from "chai";
import {
  Keypair,
  SystemProgram,
} from "@solana/web3.js";
import * as utils from "./utils";

describe("xusdc", () => {
  // Configure the client to use the local cluster.
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace.Xusdc as Program<Xusdc>;
  
  // Test context that will be shared across tests
  let ctx: any;
  
  // Test users
  let alice: Keypair;
  let bob: Keypair;
  let charlie: Keypair;
  
  before(async () => {
    // Create test users
    alice = await utils.createTestUser(provider);
    bob = await utils.createTestUser(provider);
    charlie = await utils.createTestUser(provider);
    
    // Derive PDAs
    const [programAuthority] = utils.getProgramAuthority(program.programId);
    const [rentPool] = utils.getRentPool(program.programId);
    
    // Create test context
    ctx = {
      programAuthority,
      rentPool,
      admin: provider.wallet,
    };
  });
  
  describe("Initialize", () => {
    it("should initialize the program", async () => {
      const tx = await program.methods
        .initialize()
        .accounts({
          authority: provider.wallet.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: "TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb",
          xUsdcMint: Keypair.generate().publicKey,
          xUsdcGlobalAta: Keypair.generate().publicKey,
          transferAuthority: ctx.programAuthority,
        })
        .rpc();
        
      console.log("Initialize transaction:", tx);
      
      // Verify rent pool was created
      const rentPoolAccount = await provider.connection.getAccountInfo(ctx.rentPool);
      assert.isNotNull(rentPoolAccount, "Rent pool should be created");
    });
  });

  describe("Rent Pool", () => {
    const contributionAmount = new anchor.BN(10 * anchor.web3.LAMPORTS_PER_SOL);
    
    it("should contribute rent to pool", async () => {
      const [rentContributor] = utils.getRentContributor(program.programId, alice.publicKey);
      
      const rentPoolBalanceBefore = await provider.connection.getBalance(ctx.rentPool);
      
      const tx = await program.methods
        .contributeRent(contributionAmount)
        .accounts({
          user: alice.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([alice])
        .rpc();
        
      console.log("Contribute rent transaction:", tx);
      
      const rentPoolBalanceAfter = await provider.connection.getBalance(ctx.rentPool);
      
      // Verify rent pool balance increased
      assert.equal(
        new anchor.BN(rentPoolBalanceAfter - rentPoolBalanceBefore).toString(),
        contributionAmount.toString(),
        "Rent pool balance not increased correctly"
      );
      
      // Verify contributor account
      const contributorAccount = await program.account.contributorRentInfo.fetch(rentContributor);
      assert.equal(
        contributorAccount.user.toString(),
        alice.publicKey.toString(),
        "Contributor not set correctly"
      );
      assert.equal(
        contributorAccount.amount.toString(),
        contributionAmount.toString(),
        "Contribution amount not recorded correctly"
      );
    });

    it("should allow multiple contributions from same contributor", async () => {
      const [rentContributor] = utils.getRentContributor(program.programId, alice.publicKey);
      const additionalContribution = new anchor.BN(5 * anchor.web3.LAMPORTS_PER_SOL);
      
      // Get initial state
      const contributorAccountBefore = await program.account.contributorRentInfo.fetch(rentContributor);
      const initialContribution = contributorAccountBefore.amount;
      
      // Make additional contribution
      await program.methods
        .contributeRent(additionalContribution)
        .accounts({
          user: alice.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([alice])
        .rpc();
        
      // Verify cumulative contribution
      const contributorAccountAfter = await program.account.contributorRentInfo.fetch(rentContributor);
      assert.equal(
        contributorAccountAfter.amount.toString(),
        initialContribution.add(additionalContribution).toString(),
        "Contribution amount should be cumulative"
      );
    });

    it("should allow contributions from multiple contributors", async () => {
      const [bobRentContributor] = utils.getRentContributor(program.programId, bob.publicKey);
      const bobContribution = new anchor.BN(3 * anchor.web3.LAMPORTS_PER_SOL);
      
      await program.methods
        .contributeRent(bobContribution)
        .accounts({
          user: bob.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([bob])
        .rpc();
        
      // Verify Bob's contributor account
      const bobContributorAccount = await program.account.contributorRentInfo.fetch(bobRentContributor);
      assert.equal(
        bobContributorAccount.user.toString(),
        bob.publicKey.toString(),
        "Bob's contributor not set correctly"
      );
      assert.equal(
        bobContributorAccount.amount.toString(),
        bobContribution.toString(),
        "Bob's contribution amount not recorded correctly"
      );
    });

    it("should fail to contribute zero amount", async () => {
      const [rentContributor] = utils.getRentContributor(program.programId, charlie.publicKey);
      
      try {
        await program.methods
          .contributeRent(new anchor.BN(0))
          .accounts({
            user: charlie.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([charlie])
          .rpc();
        assert.fail("Should have failed with zero contribution");
      } catch (error) {
        assert.include(error.toString(), "InvalidAmount");
      }
    });
    
    it("should withdraw unused rent contribution", async () => {
      const [rentContributor] = utils.getRentContributor(program.programId, alice.publicKey);
      const withdrawAmount = new anchor.BN(5 * anchor.web3.LAMPORTS_PER_SOL);
      
      const aliceBalanceBefore = await provider.connection.getBalance(alice.publicKey);
      const rentPoolBalanceBefore = await provider.connection.getBalance(ctx.rentPool);
      const contributorAccountBefore = await program.account.contributorRentInfo.fetch(rentContributor);
      
      const tx = await program.methods
        .withdrawRent(withdrawAmount)
        .accounts({
          user: alice.publicKey,
          systemProgram: SystemProgram.programId,
        })
        .signers([alice])
        .rpc();
        
      console.log("Withdraw rent transaction:", tx);
      
      const aliceBalanceAfter = await provider.connection.getBalance(alice.publicKey);
      const rentPoolBalanceAfter = await provider.connection.getBalance(ctx.rentPool);
      const contributorAccountAfter = await program.account.contributorRentInfo.fetch(rentContributor);
      
      // Verify balances changed correctly (accounting for transaction fees)
      assert.isTrue(
        new anchor.BN(aliceBalanceAfter).gt(new anchor.BN(aliceBalanceBefore)),
        "Alice balance should increase"
      );
      assert.equal(
        new anchor.BN(rentPoolBalanceBefore - rentPoolBalanceAfter).toString(),
        withdrawAmount.toString(),
        "Rent pool balance not decreased correctly"
      );
      
      // Verify contributor account updated
      assert.equal(
        contributorAccountAfter.amount.toString(),
        contributorAccountBefore.amount.sub(withdrawAmount).toString(),
        "Contributor account not updated correctly"
      );
    });

    it("should fail to withdraw more than available", async () => {
      const [rentContributor] = utils.getRentContributor(program.programId, alice.publicKey);
      const contributorAccount = await program.account.contributorRentInfo.fetch(rentContributor);
      
      // Try to withdraw more than available
      const excessiveAmount = contributorAccount.amount.add(new anchor.BN(1));
      
      try {
        await program.methods
          .withdrawRent(excessiveAmount)
          .accounts({
            user: alice.publicKey,
            userRentInfo: rentContributor,
            globalRentPool: ctx.rentPool,
            systemProgram: SystemProgram.programId,
          })
          .signers([alice])
          .rpc();
        assert.fail("Should have failed with excessive withdrawal");
      } catch (error) {
        assert.include(error.toString(), "InsufficientFunds");
      }
    });

    it("should fail to withdraw with wrong contributor", async () => {
      const [aliceRentContributor] = utils.getRentContributor(program.programId, alice.publicKey);
      
      try {
        await program.methods
          .withdrawRent(new anchor.BN(1 * anchor.web3.LAMPORTS_PER_SOL))
          .accounts({
            user: bob.publicKey,
            systemProgram: SystemProgram.programId,
          })
          .signers([bob])
          .rpc();
        assert.fail("Should have failed with wrong contributor");
      } catch (error) {
        assert.include(error.toString(), "ConstraintSeeds");
      }
    });

    it("should fail to withdraw zero amount", async () => {
      const [rentContributor] = utils.getRentContributor(program.programId, alice.publicKey);
      
      try {
        await program.methods
          .withdrawRent(new anchor.BN(0))
          .accounts({
            user: alice.publicKey,
            userRentInfo: rentContributor,
            globalRentPool: ctx.rentPool,
            systemProgram: SystemProgram.programId,
          })
          .signers([alice])
          .rpc();
        assert.fail("Should have failed with zero withdrawal");
      } catch (error) {
        assert.include(error.toString(), "InvalidAmount");
      }
    });

    it("should withdraw entire remaining balance", async () => {
      const [rentContributor] = utils.getRentContributor(program.programId, alice.publicKey);
      const contributorAccount = await program.account.contributorRentInfo.fetch(rentContributor);
      
      if (contributorAccount.amount.gt(new anchor.BN(0))) {
        await program.methods
          .withdrawRent(contributorAccount.amount)
          .accounts({
            user: alice.publicKey,
            userRentInfo: rentContributor,
            globalRentPool: ctx.rentPool,
            systemProgram: SystemProgram.programId,
          })
          .signers([alice])
          .rpc();
          
        // Verify account is updated correctly
        const contributorAccountAfter = await program.account.contributorRentInfo.fetch(rentContributor);
        assert.equal(
          contributorAccountAfter.amount.toString(),
          "0",
          "Should have zero balance remaining"
        );
      }
    });
  });

  describe("Garbage Collection", () => {
    it("should call garbage collect successfully", async () => {
      // The garbage_collect instruction in the current implementation
      // doesn't take any nonce arguments
      const tx = await program.methods
        .garbageCollect()
        .accounts({
          globalRentPool: ctx.rentPool,
          systemProgram: SystemProgram.programId,
        })
        .rpc();
        
      console.log("Garbage collect transaction:", tx);
      
      // Since the current implementation doesn't specify what garbage_collect does,
      // we just verify it can be called successfully
      assert.exists(tx, "Garbage collect transaction should succeed");
    });
  });
});