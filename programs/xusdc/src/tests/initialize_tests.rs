#[cfg(test)]
mod tests {
    use crate::ixs::settle_payment::{PaymentAuthorization, SettlePayload};
    use crate::state::{
        NonceAccount, ADMIN_KEY, NONCE_SEED, RENT_CONTRIBUTOR_SEED, RENT_POOL_SEED,
        TRANSFER_AUTHORITY_SEED, USDC_MINT_KEY, XUSDC_MINT_KEY,
    };
    use crate::{self as xusdc};
    use anchor_lang::prelude::*;
    use anchor_lang::solana_program::{instruction::Instruction, program_pack::Pack};
    use anchor_lang::system_program;
    use anchor_lang::InstructionData;
    use anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account_idempotent;
    use anchor_spl::associated_token::{self, get_associated_token_address_with_program_id};
    use anchor_spl::token::Token;
    use anchor_spl::token_2022::spl_token_2022;
    use litesvm::LiteSVM;
    use litesvm_token::get_spl_account;
    use litesvm_token::spl_token::extension::permanent_delegate::get_permanent_delegate;
    use litesvm_token::spl_token::instruction::mint_to;
    use litesvm_token::spl_token::{extension::StateWithExtensions, state::Mint};
    use solana_sdk::account::Account;
    use solana_sdk::instruction::InstructionError;
    use solana_sdk::program_option::COption;
    use solana_sdk::signature::{read_keypair_file, Keypair, Signer};
    use solana_sdk::transaction::{Transaction, TransactionError};
    use std::env;
    use std::path::PathBuf;
    use std::str::FromStr;

    const TEN_USDC: u64 = 10_000_000u64; // 100 USDC (6 decimals)

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

        let usdc_mint_account = svm.get_account(&USDC_MINT_KEY);
        let usdc_mint_info = Mint::unpack(&usdc_mint_account.clone().unwrap().data).unwrap();
        let mut owned_usdc_mint = usdc_mint_info.clone();
        owned_usdc_mint.mint_authority = COption::Some(admin.pubkey());
        let mut data = [0u8; Mint::LEN];
        Mint::pack(owned_usdc_mint, &mut data).unwrap();
        svm.set_account(
            USDC_MINT_KEY,
            Account {
                lamports: usdc_mint_account.unwrap().lamports,
                data: data.to_vec(),
                owner: anchor_spl::token::ID,
                executable: false,
                rent_epoch: 0,
            },
        )
        .unwrap();
        (svm, admin)
    }

    #[test]
    fn test_initialize_with_litesvm() {
        let (mut svm, admin) = setup();
        initialize(&mut svm, &admin);
    }

    fn initialize(svm: &mut LiteSVM, admin: &Keypair) {
        let program_id = xusdc::ID;
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
            Ok(_) => {}
            Err(e) => println!("Initialize failed: {}", e.meta.logs.join("\n")),
        }

        // Verify mint was initialized with permanent delegate
        // Note: This will fail until we have actual program implementation
        let mint_account = svm.get_account(&XUSDC_MINT_KEY);
        assert!(mint_account.is_some(), "Mint account should exist");

        let ata_account = svm.get_account(&global_ata);
        assert!(ata_account.is_some(), "ATA account should exist");

        let mint_data = svm.get_account(&XUSDC_MINT_KEY).unwrap().data;
        let mint_ext = StateWithExtensions::<Mint>::unpack(&mint_data).unwrap();
        assert_eq!(mint_ext.base.mint_authority.unwrap(), transfer_authority);
        let permanent_delegate = get_permanent_delegate(&mint_ext).unwrap();
        assert_eq!(permanent_delegate, transfer_authority);
    }

    #[test]
    fn test_mock_deposit_flow() {
        // This test demonstrates the expected deposit flow
        // User deposits USDC and receives xUSDC 1:1
        let (mut svm, admin) = setup();
        let _ = deposit_and_initialize(&mut svm, &admin, TEN_USDC);
    }

    fn deposit_and_initialize(svm: &mut LiteSVM, admin: &Keypair, amount: u64) -> Keypair {
        initialize(svm, &admin);

        let user = Keypair::new();

        let user_usdc_ata = get_associated_token_address_with_program_id(
            &user.pubkey(),
            &USDC_MINT_KEY,
            &Token::id(),
        );

        let create_user_usdc_ata_ix = create_associated_token_account_idempotent(
            &admin.pubkey(),
            &user.pubkey(),
            &USDC_MINT_KEY,
            &Token::id(),
        );

        let mint_ix = mint_to(
            &Token::id(),
            &USDC_MINT_KEY,
            &user_usdc_ata,
            &admin.pubkey(),
            &[&admin.pubkey()],
            amount,
        )
        .unwrap();

        // let data = get_spl_account::<Mint>(&svm, &USDC_MINT_KEY);
        // println!("data: {:?}", data);

        let tx = Transaction::new_signed_with_payer(
            &[create_user_usdc_ata_ix, mint_ix],
            Some(&admin.pubkey()),
            &[&admin],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();

        let user_usdc_ata_account = svm.get_account(&user_usdc_ata);
        assert!(
            user_usdc_ata_account.is_some(),
            "User USDC ATA should exist"
        );

        let user_xusdc_ata = get_associated_token_address_with_program_id(
            &user.pubkey(),
            &XUSDC_MINT_KEY,
            &spl_token_2022::ID,
        );

        let (transfer_authority, _) =
            Pubkey::find_program_address(&[TRANSFER_AUTHORITY_SEED], &xusdc::ID);

        let global_usdc_ata = get_associated_token_address_with_program_id(
            &transfer_authority,
            &USDC_MINT_KEY,
            &Token::id(),
        );

        let accounts = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new_readonly(Token::id(), false),
            AccountMeta::new_readonly(spl_token_2022::ID, false),
            AccountMeta::new(XUSDC_MINT_KEY, false),
            AccountMeta::new_readonly(USDC_MINT_KEY, false),
            AccountMeta::new(user_usdc_ata, false),
            AccountMeta::new(user_xusdc_ata, false),
            AccountMeta::new(global_usdc_ata, false),
            AccountMeta::new_readonly(transfer_authority, false),
        ];
        let data = crate::instruction::Deposit { amount }.data();

        let init_ix = Instruction {
            program_id: xusdc::id(),
            accounts,
            data,
        };

        let create_user_xusdc_ata_ix = create_associated_token_account_idempotent(
            &admin.pubkey(),
            &user.pubkey(),
            &XUSDC_MINT_KEY,
            &spl_token_2022::ID,
        );
        let tx = Transaction::new_signed_with_payer(
            &[create_user_xusdc_ata_ix, init_ix],
            Some(&admin.pubkey()),
            &[&admin, &user],
            svm.latest_blockhash(),
        );
        if let Err(e) = svm.send_transaction(tx) {
            println!("Error: {}", e.meta.logs.join("\n"));
        }

        let user_xusdc_ata_account =
            get_spl_account::<litesvm_token::spl_token::state::Account>(&svm, &user_xusdc_ata)
                .unwrap();
        assert_eq!(user_xusdc_ata_account.amount, amount);

        let user_usdc_ata_account =
            get_spl_account::<litesvm_token::spl_token::state::Account>(&svm, &user_usdc_ata)
                .unwrap();
        assert_eq!(user_usdc_ata_account.amount, 0);

        // The key insight is that xUSDC can be transferred by the permanent delegate
        // without requiring the user's signature on-chain
        user
    }

    // In the actual deposit:
    // 1. User has USDC in their token account
    // 2. User calls deposit instruction
    // 3. USDC is transferred to vault
    // 4. xUSDC is minted to user (using mint authority = transfer_authority PDA)
    // 5. Result: User has xUSDC they can spend gaslessly
    #[test]
    fn test_transfer_with_permanent_delegate() {
        let (mut svm, admin) = setup();
        let user = deposit_and_initialize(&mut svm, &admin, TEN_USDC);
        transfer_with_permanent_delegate(&mut svm, &user, 10_000);
    }

    fn transfer_with_permanent_delegate(
        svm: &mut LiteSVM,
        alice: &Keypair,
        expiry_delta: i64,
    ) -> PaymentAuthorization {
        let clock = svm.get_sysvar::<Clock>();
        let expires_at = clock.unix_timestamp + expiry_delta;

        let program_id = xusdc::ID;
        let (transfer_authority, _) =
            Pubkey::find_program_address(&[TRANSFER_AUTHORITY_SEED], &program_id);

        let alice_xusdc_ata = get_associated_token_address_with_program_id(
            &alice.pubkey(),
            &XUSDC_MINT_KEY,
            &spl_token_2022::ID,
        );

        let bob = Keypair::new();

        svm.airdrop(&bob.pubkey(), 10_000_000_000).unwrap();

        let create_bob_xusdc_ata_ix = create_associated_token_account_idempotent(
            &bob.pubkey(),
            &bob.pubkey(),
            &XUSDC_MINT_KEY,
            &spl_token_2022::ID,
        );

        let bob_xusdc_ata = get_associated_token_address_with_program_id(
            &bob.pubkey(),
            &XUSDC_MINT_KEY,
            &spl_token_2022::ID,
        );

        let (user_rent_info, _) = Pubkey::find_program_address(
            &[RENT_CONTRIBUTOR_SEED, &bob.pubkey().to_bytes()],
            &program_id,
        );
        let (global_rent_pool, _) = Pubkey::find_program_address(&[RENT_POOL_SEED], &program_id);
        let contribute_rent_data = crate::instruction::ContributeRent { amount: 10_000_000 }.data();
        let contribute_rent_ix = Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(bob.pubkey(), true),
                AccountMeta::new(user_rent_info, false),
                AccountMeta::new(global_rent_pool, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: contribute_rent_data,
        };

        let payment_auth = PaymentAuthorization {
            from: alice.pubkey(),
            to: bob.pubkey(),
            amount: TEN_USDC,
            nonce: [1u8; 32],
            valid_until: expires_at,
        };
        let (nonce_pda, _) =
            Pubkey::find_program_address(&[NONCE_SEED, &payment_auth.nonce], &program_id);

        let settle_payment_accounts = [
            AccountMeta::new(bob.pubkey(), true),
            AccountMeta::new_readonly(spl_token_2022::ID, false),
            AccountMeta::new_readonly(XUSDC_MINT_KEY, false),
            AccountMeta::new(alice_xusdc_ata, false),
            AccountMeta::new(bob_xusdc_ata, false),
            AccountMeta::new_readonly(transfer_authority, false),
            AccountMeta::new(nonce_pda, false),
            AccountMeta::new(global_rent_pool, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ];
        let data = crate::instruction::SettlePayment {
            payload: SettlePayload {
                payment_auth: payment_auth.clone(),
                signature: alice
                    .sign_message(&payment_auth.try_to_vec().unwrap())
                    .as_ref()
                    .try_into()
                    .unwrap(),
                signer_pubkey: alice.pubkey().to_bytes(),
            },
        }
        .data();
        let settle_payment_ix = Instruction {
            program_id,
            accounts: settle_payment_accounts.to_vec(),
            data,
        };

        let tx = Transaction::new_signed_with_payer(
            &[
                create_bob_xusdc_ata_ix,
                contribute_rent_ix,
                settle_payment_ix,
            ],
            Some(&bob.pubkey()),
            &[&bob],
            svm.latest_blockhash(),
        );
        if let Err(e) = svm.send_transaction(tx) {
            println!("Error: {}", e.meta.logs.join("\n"));
        }

        let alice_xusdc_ata_account =
            get_spl_account::<litesvm_token::spl_token::state::Account>(&svm, &alice_xusdc_ata)
                .unwrap();
        assert_eq!(alice_xusdc_ata_account.amount, 0);

        let bob_xusdc_ata_account =
            get_spl_account::<litesvm_token::spl_token::state::Account>(&svm, &bob_xusdc_ata)
                .unwrap();
        assert_eq!(bob_xusdc_ata_account.amount, TEN_USDC);

        let nonce_account = svm.get_account(&nonce_pda);
        assert!(nonce_account.is_some(), "Nonce account should exist");
        let nonce_account_data = nonce_account.unwrap().data;
        let nonce_account_ext =
            NonceAccount::try_deserialize(&mut nonce_account_data.as_slice()).unwrap();
        assert_eq!(nonce_account_ext.expires_at, expires_at);

        payment_auth
    }

    #[test]
    fn test_withdraw_flow() {
        let (mut svm, admin) = setup();
        let user = deposit_and_initialize(&mut svm, &admin, TEN_USDC);
        withdraw(&mut svm, &user, TEN_USDC);
    }

    fn withdraw(svm: &mut LiteSVM, user: &Keypair, amount: u64) {
        let program_id = xusdc::ID;

        let (transfer_authority, _) =
            Pubkey::find_program_address(&[TRANSFER_AUTHORITY_SEED], &program_id);

        let user_xusdc_ata = get_associated_token_address_with_program_id(
            &user.pubkey(),
            &XUSDC_MINT_KEY,
            &spl_token_2022::ID,
        );
        let user_usdc_ata = get_associated_token_address_with_program_id(
            &user.pubkey(),
            &USDC_MINT_KEY,
            &Token::id(),
        );
        let global_usdc_ata = get_associated_token_address_with_program_id(
            &transfer_authority,
            &USDC_MINT_KEY,
            &Token::id(),
        );

        let accounts = vec![
            AccountMeta::new(user.pubkey(), true),
            AccountMeta::new(user_xusdc_ata, false),
            AccountMeta::new(user_usdc_ata, false),
            AccountMeta::new(global_usdc_ata, false),
            AccountMeta::new_readonly(USDC_MINT_KEY, false),
            AccountMeta::new(XUSDC_MINT_KEY, false),
            AccountMeta::new_readonly(transfer_authority, false),
            AccountMeta::new_readonly(Token::id(), false),
            AccountMeta::new_readonly(spl_token_2022::ID, false),
        ];
        let withdraw_ix = Instruction {
            program_id,
            accounts,
            data: crate::instruction::Withdraw { amount }.data(),
        };
        svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

        let tx = Transaction::new_signed_with_payer(
            &[withdraw_ix],
            Some(&user.pubkey()),
            &[&user],
            svm.latest_blockhash(),
        );
        if let Err(e) = svm.send_transaction(tx) {
            println!("Error: {}", e.meta.logs.join("\n"));
        }

        let user_xusdc_ata_account =
            get_spl_account::<litesvm_token::spl_token::state::Account>(&svm, &user_xusdc_ata)
                .unwrap();
        assert_eq!(user_xusdc_ata_account.amount, 0);

        let user_usdc_ata_account =
            get_spl_account::<litesvm_token::spl_token::state::Account>(&svm, &user_usdc_ata)
                .unwrap();
        assert_eq!(user_usdc_ata_account.amount, amount);
    }

    #[test]
    fn test_garbage_collection() {
        let (mut svm, admin) = setup();
        let user = deposit_and_initialize(&mut svm, &admin, TEN_USDC);

        let expiry_delta = 10_000;
        let payment_auth = transfer_with_permanent_delegate(&mut svm, &user, expiry_delta);

        let (nonce_pda, _) =
            Pubkey::find_program_address(&[NONCE_SEED, &payment_auth.nonce], &xusdc::ID);
        let nonce = svm.get_account(&nonce_pda).unwrap();
        let nonce_lamports = nonce.lamports;
        let nonce = NonceAccount::try_deserialize(&mut nonce.data.as_slice()).unwrap();
        assert_eq!(nonce.expires_at, payment_auth.valid_until);

        let clock = svm.get_sysvar::<Clock>();
        assert!(clock.unix_timestamp < payment_auth.valid_until);

        let (global_rent_pool, _) = Pubkey::find_program_address(&[RENT_POOL_SEED], &xusdc::ID);
        let global_rent_pool_account = svm.get_account(&global_rent_pool).unwrap();
        let global_lamports = global_rent_pool_account.lamports;

        let ix = Instruction {
            program_id: xusdc::ID,
            accounts: vec![
                AccountMeta::new(nonce_pda, false),
                AccountMeta::new(global_rent_pool, false),
            ],
            data: crate::instruction::GarbageCollect {}.data(),
        };

        let tx = Transaction::new_signed_with_payer(
            &[ix.clone()],
            Some(&admin.pubkey()),
            &[&admin],
            svm.latest_blockhash(),
        );
        match svm.send_transaction(tx) {
            Ok(_) => {
                panic!("Garbage collection should fail, since the nonce is not yet expired");
            }
            Err(e) => {
                if let TransactionError::InstructionError(_, e) = e.err {
                    if let InstructionError::Custom(e) = e {
                        assert_eq!(
                            u32::from(e),
                            u32::from(crate::error::ErrorCode::NonceIsNotExpired)
                        );
                    } else {
                        panic!("Expected Custom(NonceIsNotExpired), got {:?}", e);
                    }
                } else {
                    panic!("Expected InstructionError, got {:?}", e.err);
                }
            }
        }

        svm.expire_blockhash();
        let mut clock = svm.get_sysvar::<Clock>();
        clock.unix_timestamp = clock.unix_timestamp + expiry_delta + 1;
        svm.set_sysvar(&clock);

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&admin.pubkey()),
            &[&admin],
            svm.latest_blockhash(),
        );
        match svm.send_transaction(tx) {
            Ok(_) => {}
            Err(e) => {
                panic!("Error: {}", e.meta.logs.join("\n"));
            }
        }

        svm.expire_blockhash();
        let global_rent_pool_account = svm.get_account(&global_rent_pool).unwrap();
        assert_eq!(
            global_rent_pool_account.lamports,
            global_lamports + nonce_lamports
        );

        // NOTE: svm doesn't actually delete the nonce account data when lamports are zeroed
        // but we expect it to be deleted on mainnet/devnet
        let nonce_account = svm.get_account(&nonce_pda).unwrap();
        assert_eq!(nonce_account.lamports, 0);
        assert_eq!(nonce_account.data.len(), 0);
    }
}
