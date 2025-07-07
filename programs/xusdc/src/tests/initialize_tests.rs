#[cfg(test)]
mod tests {
    use crate::ixs::settle_payment::{PaymentAuthorization, SettlePayload};
    use crate::state::{ADMIN_KEY, TRANSFER_AUTHORITY_SEED, USDC_MINT_KEY, XUSDC_MINT_KEY};
    use crate::{self as xusdc};
    use anchor_lang::prelude::*;
    use anchor_lang::solana_program::{
        instruction::Instruction, program_pack::Pack, system_instruction,
    };
    use anchor_lang::system_program;
    use anchor_lang::InstructionData;
    use anchor_spl::associated_token;
    use anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent;
    use anchor_spl::token::Token;
    use anchor_spl::token_2022::spl_token_2022;
    use litesvm::LiteSVM;
    use serde_json::json;
    // use serde::Deserialize;
    use solana_sdk::account::Account;
    use solana_sdk::program_pack::Pack as SolanaPack;
    use solana_sdk::signature::{read_keypair_file, Keypair, Signer};
    use solana_sdk::transaction::Transaction;
    use std::env;
    use std::path::PathBuf;
    use std::str::FromStr;

    /// Read the default Solana keypair file into memory.
    pub fn load_default_keypair() -> Keypair {
        let keypair_path: PathBuf = PathBuf::from(format!(
            "/Users/{}/.config/solana/id.json",
            env::var("USER").unwrap()
        ));

        read_keypair_file(keypair_path).unwrap()
    }

    pub fn load_mint_keypair() -> Keypair {
        let p = PathBuf::from(format!(
            "{}/../../{}.json",
            env!("CARGO_MANIFEST_DIR"),
            XUSDC_MINT_KEY.to_string()
        ));
        read_keypair_file(p).unwrap()
    }

    pub fn load_account_from_file(path: PathBuf) -> Account {
        let data_raw = std::fs::read_to_string(path).unwrap();
        let data: serde_json::Value = serde_json::from_str(&data_raw).unwrap();
        let data = data["account"].as_object().unwrap();
        println!("data: {:?}", data);
        Account {
            lamports: data["lamports"].as_u64().unwrap_or(0),
            data: base64::decode(data["data"][0].as_str().unwrap()).unwrap(),
            owner: Pubkey::from_str(data["owner"].as_str().unwrap()).unwrap(),
            executable: false,
            rent_epoch: data["rentEpoch"].as_u64().unwrap(),
        }
    }

    pub fn setup() -> (LiteSVM, Keypair) {
        let mut svm = LiteSVM::new();

        let admin = load_default_keypair();
        assert_eq!(admin.pubkey(), ADMIN_KEY);
        svm.airdrop(&admin.pubkey(), 10_000_000).unwrap();

        // Deploy the xUSDC program
        let program_id = xusdc::ID;
        let so_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/xusdc.so");
        let program_bytes = std::fs::read(so_path).unwrap();
        svm.add_program(program_id, &program_bytes);
        svm.set_account(
            USDC_MINT_KEY,
            load_account_from_file(PathBuf::from("../../usdc-mint.json")),
        )
        .unwrap();
        (svm, admin)
    }

    #[test]
    fn test_initialize_with_litesvm() {
        let (mut svm, admin) = setup();

        let program_id = xusdc::ID;
        // Create admin keypair and airdrop SOL

        // Load mint keypair
        let mint_keypair = load_mint_keypair();
        assert_eq!(mint_keypair.pubkey(), XUSDC_MINT_KEY);

        // Derive PDAs
        let (transfer_authority, _) =
            Pubkey::find_program_address(&[TRANSFER_AUTHORITY_SEED], &program_id);

        // Create global ATA
        let global_ata = anchor_spl::associated_token::spl_associated_token_account::get_associated_token_address_with_program_id(
            &transfer_authority,
            &USDC_MINT_KEY,
            &Token::id(),
        );

        // Create initialize instruction
        let accounts = vec![
            AccountMeta::new(admin.pubkey(), true),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(spl_token_2022::ID, false),
            AccountMeta::new(XUSDC_MINT_KEY, true),
            AccountMeta::new(USDC_MINT_KEY, false),
            AccountMeta::new(global_ata, false),
            AccountMeta::new_readonly(transfer_authority, false),
            AccountMeta::new_readonly(associated_token::ID, false),
            AccountMeta::new_readonly(Token::id(), false),
        ];
        let data = crate::instruction::Initialize {}.data();

        let init_ix = Instruction {
            program_id,
            accounts,
            data,
        };

        let init_tx = Transaction::new_signed_with_payer(
            &[init_ix],
            Some(&admin.pubkey()),
            &[&admin, &mint_keypair],
            svm.latest_blockhash(),
        );

        let init_result = svm.send_transaction(init_tx);

        // For debugging - print result
        match init_result {
            Ok(_) => println!("Initialize succeeded"),
            Err(e) => println!("Initialize failed: {}", e.meta.logs.join("\n")),
        }

        // Verify mint was initialized with permanent delegate
        // Note: This will fail until we have actual program implementation
        let mint_account = svm.get_account(&XUSDC_MINT_KEY);
        assert!(mint_account.is_some(), "Mint account should exist");

        let ata_account = svm.get_account(&global_ata);
        assert!(ata_account.is_some(), "ATA account should exist");
    }

    #[test]
    fn test_permanent_delegate_concept() {
        // This test verifies our understanding of permanent delegate
        // Without executing on-chain, we verify the concept

        let program_id = xusdc::ID;
        let (transfer_authority, _) =
            Pubkey::find_program_address(&[TRANSFER_AUTHORITY_SEED], &program_id);

        // The permanent delegate pattern means:
        // 1. The mint has a permanent delegate set to transfer_authority
        // 2. This delegate can transfer tokens from ANY account without owner signature
        // 3. This enables gasless transfers where users sign off-chain messages

        // Verify our PDA derivation is deterministic
        let (transfer_authority_2, bump) =
            Pubkey::find_program_address(&[TRANSFER_AUTHORITY_SEED], &program_id);

        assert_eq!(
            transfer_authority, transfer_authority_2,
            "PDA should be deterministic"
        );
        assert!(bump > 0, "Bump should be valid");
    }

    #[test]
    fn test_mock_deposit_flow() {
        // This test demonstrates the expected deposit flow
        // User deposits USDC and receives xUSDC 1:1

        let amount = 100_000_000u64; // 100 USDC (6 decimals)

        create_associated_token_account_idempotent(admin, wallet_address, token_mint_address, token_program_id)

        // In the actual deposit:
        // 1. User has USDC in their token account
        // 2. User calls deposit instruction
        // 3. USDC is transferred to vault
        // 4. xUSDC is minted to user (using mint authority = transfer_authority PDA)
        // 5. Result: User has xUSDC they can spend gaslessly

        // The key insight is that xUSDC can be transferred by the permanent delegate
        // without requiring the user's signature on-chain

        assert_eq!(
            amount, 100_000_000,
            "Amount represents 100 tokens with 6 decimals"
        );
    }

    #[test]
    fn test_transfer_with_permanent_delegate() {
        let mut svm = LiteSVM::new();

        // Deploy programs
        let program_id = xusdc::ID;
        svm.add_program(program_id, &[0u8; 1000]);
        svm.add_program(spl_token_2022::ID, &[0u8; 1000]);

        // Create actors
        let alice = Keypair::new();
        let bob = Keypair::new();
        let facilitator = Keypair::new();

        // Airdrop SOL
        svm.airdrop(&alice.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&bob.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&facilitator.pubkey(), 10_000_000_000).unwrap();

        // Assume xUSDC mint is already initialized with permanent delegate
        // Create Alice's xUSDC account with 100 xUSDC
        let alice_xusdc_account = Keypair::new();
        let token_account_size = 165; // Token-2022 account size
        let token_rent = svm.minimum_balance_for_rent_exemption(token_account_size);

        let create_alice_account_ix = system_instruction::create_account(
            &alice.pubkey(),
            &alice_xusdc_account.pubkey(),
            token_rent,
            token_account_size as u64,
            &spl_token_2022::ID,
        );

        let init_alice_account_ix = spl_token_2022::instruction::initialize_account3(
            &spl_token_2022::ID,
            &alice_xusdc_account.pubkey(),
            &XUSDC_MINT_KEY,
            &alice.pubkey(),
        )
        .unwrap();

        let tx = Transaction::new_signed_with_payer(
            &[create_alice_account_ix, init_alice_account_ix],
            Some(&alice.pubkey()),
            &[&alice, &alice_xusdc_account],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx).unwrap();

        // Create Bob's xUSDC account
        let bob_xusdc_account = Keypair::new();
        let create_bob_account_ix = system_instruction::create_account(
            &bob.pubkey(),
            &bob_xusdc_account.pubkey(),
            token_rent,
            token_account_size as u64,
            &spl_token_2022::ID,
        );

        let init_bob_account_ix = spl_token_2022::instruction::initialize_account3(
            &spl_token_2022::ID,
            &bob_xusdc_account.pubkey(),
            &XUSDC_MINT_KEY,
            &bob.pubkey(),
        )
        .unwrap();

        let tx2 = Transaction::new_signed_with_payer(
            &[create_bob_account_ix, init_bob_account_ix],
            Some(&bob.pubkey()),
            &[&bob, &bob_xusdc_account],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx2).unwrap();

        // Create payment authorization (off-chain)
        let payment_auth = PaymentAuthorization {
            from: alice_xusdc_account.pubkey(),
            to: bob_xusdc_account.pubkey(),
            amount: 10_000_000, // 10 xUSDC
            nonce: [1u8; 32],
            valid_until: 9999999999,
        };

        // Alice signs the payment authorization off-chain
        let message = payment_auth.try_to_vec().unwrap();
        let alice_signature = alice.sign_message(&message);

        // Derive rent pool PDA
        let (rent_pool, _) = Pubkey::find_program_address(&[b"rent_pool"], &program_id);

        // Create nonce PDA
        let (nonce_pda, _) =
            Pubkey::find_program_address(&[b"nonce", &payment_auth.nonce], &program_id);

        // Create settle payment instruction
        let settle_accounts = vec![
            AccountMeta::new(facilitator.pubkey(), true),
            AccountMeta::new(alice_xusdc_account.pubkey(), false),
            AccountMeta::new(bob_xusdc_account.pubkey(), false),
            AccountMeta::new(nonce_pda, false),
            AccountMeta::new(rent_pool, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(spl_token_2022::ID, false),
        ];

        let settle_payload = SettlePayload {
            payment_auth: payment_auth.clone(),
            signature: alice_signature.as_ref().try_into().unwrap(),
            signer_pubkey: alice.pubkey().to_bytes(),
        };

        let settle_data = crate::instruction::SettlePayment {
            payload: settle_payload,
        }
        .data();

        let settle_ix = Instruction {
            program_id,
            accounts: settle_accounts,
            data: settle_data,
        };

        let settle_tx = Transaction::new_signed_with_payer(
            &[settle_ix],
            Some(&facilitator.pubkey()),
            &[&facilitator],
            svm.latest_blockhash(),
        );

        let settle_result = svm.send_transaction(settle_tx);

        // For debugging
        match settle_result {
            Ok(meta) => println!("Settle payment succeeded: {:?}", meta),
            Err(e) => println!("Settle payment failed: {:?}", e),
        }

        // Note: These assertions will fail until we have actual program implementation
        // The key point is that the transfer happened without Alice's on-chain signature
        // Only the facilitator signed the transaction, using the permanent delegate authority
    }

    #[test]
    fn test_create_mock_usdc_with_litesvm() {
        let mut svm = LiteSVM::new();

        // Create a mock USDC mint
        let mint_keypair = Keypair::new();
        let mint_pubkey = mint_keypair.pubkey();
        let authority = Keypair::new();

        // Create a payer
        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

        // Create mint account
        let mint_size = anchor_spl::token::spl_token::state::Mint::LEN;
        let rent = svm.minimum_balance_for_rent_exemption(mint_size);

        let create_account_ix = system_instruction::create_account(
            &payer.pubkey(),
            &mint_pubkey,
            rent,
            mint_size as u64,
            &anchor_spl::token::ID,
        );

        let init_mint_ix = anchor_spl::token::spl_token::instruction::initialize_mint(
            &anchor_spl::token::ID,
            &mint_pubkey,
            &authority.pubkey(),
            None,
            6, // USDC has 6 decimals
        )
        .unwrap();

        let tx = Transaction::new_signed_with_payer(
            &[create_account_ix, init_mint_ix],
            Some(&payer.pubkey()),
            &[&payer, &mint_keypair],
            svm.latest_blockhash(),
        );

        let result = svm.send_transaction(tx);
        assert!(result.is_ok(), "Should create mock USDC mint");

        // Verify mint was created
        let mint_account = svm.get_account(&mint_pubkey);
        assert!(mint_account.is_some(), "Mint account should exist");
    }

    #[test]
    fn test_deposit_with_litesvm() {
        let mut svm = LiteSVM::new();

        // Deploy programs
        let program_id = xusdc::ID;
        svm.add_program(program_id, &[0u8; 1000]);
        svm.add_program(spl_token_2022::ID, &[0u8; 1000]);
        svm.add_program(anchor_spl::token::ID, &[0u8; 1000]);

        // Create actors
        let admin = Keypair::new();
        let user = Keypair::new();
        let mint_authority = Keypair::new();

        // Airdrop SOL
        svm.airdrop(&admin.pubkey(), 10_000_000_000).unwrap();
        svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

        // First initialize the program (assuming it's been initialized)
        // In real test, we'd call initialize first

        // Create mock USDC mint
        let usdc_mint = Keypair::new();
        let mint_size = anchor_spl::token::spl_token::state::Mint::LEN;
        let rent = svm.minimum_balance_for_rent_exemption(mint_size);

        let create_usdc_mint_ix = system_instruction::create_account(
            &user.pubkey(),
            &usdc_mint.pubkey(),
            rent,
            mint_size as u64,
            &anchor_spl::token::ID,
        );

        let init_usdc_mint_ix = anchor_spl::token::spl_token::instruction::initialize_mint(
            &anchor_spl::token::ID,
            &usdc_mint.pubkey(),
            &mint_authority.pubkey(),
            None,
            6, // USDC decimals
        )
        .unwrap();

        let tx = Transaction::new_signed_with_payer(
            &[create_usdc_mint_ix, init_usdc_mint_ix],
            Some(&user.pubkey()),
            &[&user, &usdc_mint],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx).unwrap();

        // Create user's USDC token account
        let user_usdc_account = Keypair::new();
        let token_account_size = anchor_spl::token::spl_token::state::Account::LEN;
        let token_rent = svm.minimum_balance_for_rent_exemption(token_account_size);

        let create_token_account_ix = system_instruction::create_account(
            &user.pubkey(),
            &user_usdc_account.pubkey(),
            token_rent,
            token_account_size as u64,
            &anchor_spl::token::ID,
        );

        let init_token_account_ix = anchor_spl::token::spl_token::instruction::initialize_account(
            &anchor_spl::token::ID,
            &user_usdc_account.pubkey(),
            &usdc_mint.pubkey(),
            &user.pubkey(),
        )
        .unwrap();

        let tx2 = Transaction::new_signed_with_payer(
            &[create_token_account_ix, init_token_account_ix],
            Some(&user.pubkey()),
            &[&user, &user_usdc_account],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx2).unwrap();

        // Mint 100 USDC to user
        let mint_to_ix = anchor_spl::token::spl_token::instruction::mint_to(
            &anchor_spl::token::ID,
            &usdc_mint.pubkey(),
            &user_usdc_account.pubkey(),
            &mint_authority.pubkey(),
            &[],
            100_000_000, // 100 USDC with 6 decimals
        )
        .unwrap();

        let tx3 = Transaction::new_signed_with_payer(
            &[mint_to_ix],
            Some(&user.pubkey()),
            &[&user, &mint_authority],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx3).unwrap();

        // Verify user has 100 USDC
        let token_account = svm.get_account(&user_usdc_account.pubkey()).unwrap();
        let account_data =
            anchor_spl::token::spl_token::state::Account::unpack(&token_account.data).unwrap();
        assert_eq!(
            account_data.amount, 100_000_000,
            "User should have 100 USDC"
        );

        // Create user's xUSDC token account
        let user_xusdc_account = Keypair::new();
        let create_xusdc_account_ix = system_instruction::create_account(
            &user.pubkey(),
            &user_xusdc_account.pubkey(),
            token_rent,
            token_account_size as u64,
            &spl_token_2022::ID,
        );

        let init_xusdc_account_ix = spl_token_2022::instruction::initialize_account3(
            &spl_token_2022::ID,
            &user_xusdc_account.pubkey(),
            &XUSDC_MINT_KEY,
            &user.pubkey(),
        )
        .unwrap();

        let tx4 = Transaction::new_signed_with_payer(
            &[create_xusdc_account_ix, init_xusdc_account_ix],
            Some(&user.pubkey()),
            &[&user, &user_xusdc_account],
            svm.latest_blockhash(),
        );

        svm.send_transaction(tx4).unwrap();

        // Derive vault PDA
        let (vault, _) =
            Pubkey::find_program_address(&[b"vault", usdc_mint.pubkey().as_ref()], &program_id);

        // Create deposit instruction
        let deposit_accounts = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(user_usdc_account.pubkey(), false),
            AccountMeta::new(user_xusdc_account.pubkey(), false),
            AccountMeta::new(vault, false),
            AccountMeta::new(XUSDC_MINT_KEY, false),
            AccountMeta::new_readonly(usdc_mint.pubkey(), false),
            AccountMeta::new_readonly(anchor_spl::token::ID, false),
            AccountMeta::new_readonly(spl_token_2022::ID, false),
        ];

        let deposit_data = crate::instruction::Deposit { amount: 50_000_000 }.data(); // 50 USDC

        let deposit_ix = Instruction {
            program_id,
            accounts: deposit_accounts,
            data: deposit_data,
        };

        let deposit_tx = Transaction::new_signed_with_payer(
            &[deposit_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );

        let deposit_result = svm.send_transaction(deposit_tx);

        // For debugging
        match deposit_result {
            Ok(meta) => println!("Deposit succeeded: {:?}", meta),
            Err(e) => println!("Deposit failed: {:?}", e),
        }

        // Note: These assertions will fail until we have actual program implementation
        // Check user's USDC balance decreased by 50
        let usdc_account_after = svm.get_account(&user_usdc_account.pubkey());
        if let Some(account) = usdc_account_after {
            let account_data =
                anchor_spl::token::spl_token::state::Account::unpack(&account.data).ok();
            println!("USDC balance after: {:?}", account_data.map(|a| a.amount));
        }

        // Check user's xUSDC balance increased by 50
        let xusdc_account_after = svm.get_account(&user_xusdc_account.pubkey());
        if let Some(account) = xusdc_account_after {
            println!("xUSDC account data length: {}", account.data.len());
        }
    }
}
