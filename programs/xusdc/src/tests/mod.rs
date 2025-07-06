#[cfg(test)]
mod initialize_tests;

#[cfg(test)]
mod settle_payment_tests {
    use crate::ixs::settle_payment::{PaymentAuthorization, SettlePayload};
    use anchor_lang::prelude::*;
    use anchor_lang::solana_program::pubkey::Pubkey;

    #[test]
    fn test_payment_authorization_serialization() {
        // Create test pubkeys
        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        
        // Create payment authorization
        let payment_auth = PaymentAuthorization {
            from,
            to,
            amount: 100_000_000,
            nonce: [1u8; 32],
            valid_until: 1234567890,
        };
        
        // Test serialization
        let serialized = payment_auth.try_to_vec().unwrap();
        
        // Expected size: 32 (from) + 32 (to) + 8 (amount) + 32 (nonce) + 8 (valid_until) = 112 bytes
        assert_eq!(serialized.len(), 112, "Serialized payment auth should be 112 bytes");
        
        // Test deserialization
        let deserialized = PaymentAuthorization::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.from, payment_auth.from);
        assert_eq!(deserialized.to, payment_auth.to);
        assert_eq!(deserialized.amount, payment_auth.amount);
        assert_eq!(deserialized.nonce, payment_auth.nonce);
        assert_eq!(deserialized.valid_until, payment_auth.valid_until);
    }

    #[test]
    fn test_settle_payload_structure() {
        let from = Pubkey::new_unique();
        let to = Pubkey::new_unique();
        
        let payment_auth = PaymentAuthorization {
            from,
            to,
            amount: 50_000_000,
            nonce: [2u8; 32],
            valid_until: 9999999999,
        };
        
        let settle_payload = SettlePayload {
            payment_auth: payment_auth.clone(),
            signature: [0u8; 64],
            signer_pubkey: from.to_bytes(),
        };
        
        // Test that payload can be serialized
        let serialized = settle_payload.try_to_vec().unwrap();
        assert!(serialized.len() > 0, "Settle payload should serialize successfully");
        
        // Test deserialization
        let deserialized = SettlePayload::try_from_slice(&serialized).unwrap();
        assert_eq!(deserialized.payment_auth.from, payment_auth.from);
        assert_eq!(deserialized.payment_auth.to, payment_auth.to);
        assert_eq!(deserialized.payment_auth.amount, payment_auth.amount);
        assert_eq!(deserialized.signature, settle_payload.signature);
        assert_eq!(deserialized.signer_pubkey, settle_payload.signer_pubkey);
    }

    #[test]
    fn test_typed_payload_enforces_all_fields() {
        let alice = Pubkey::new_unique();
        let bob = Pubkey::new_unique();
        
        // Create two payment authorizations that differ only in amount
        let payment_auth1 = PaymentAuthorization {
            from: alice,
            to: bob,
            amount: 100,
            nonce: [3u8; 32],
            valid_until: 1234567890,
        };
        
        let payment_auth2 = PaymentAuthorization {
            from: alice,
            to: bob,
            amount: 200, // Different amount
            nonce: [3u8; 32],
            valid_until: 1234567890,
        };
        
        // Serialize both
        let serialized1 = payment_auth1.try_to_vec().unwrap();
        let serialized2 = payment_auth2.try_to_vec().unwrap();
        
        // They should be different
        assert_ne!(serialized1, serialized2, "Different amounts should produce different serializations");
        
        // Test changing each field produces different serialization
        let payment_auth_diff_to = PaymentAuthorization {
            from: alice,
            to: alice, // Different recipient
            amount: 100,
            nonce: [3u8; 32],
            valid_until: 1234567890,
        };
        
        let serialized_diff_to = payment_auth_diff_to.try_to_vec().unwrap();
        assert_ne!(serialized1, serialized_diff_to, "Different recipients should produce different serializations");
        
        // Different nonce
        let payment_auth_diff_nonce = PaymentAuthorization {
            from: alice,
            to: bob,
            amount: 100,
            nonce: [4u8; 32], // Different nonce
            valid_until: 1234567890,
        };
        
        let serialized_diff_nonce = payment_auth_diff_nonce.try_to_vec().unwrap();
        assert_ne!(serialized1, serialized_diff_nonce, "Different nonces should produce different serializations");
        
        // Different expiry
        let payment_auth_diff_expiry = PaymentAuthorization {
            from: alice,
            to: bob,
            amount: 100,
            nonce: [3u8; 32],
            valid_until: 9999999999, // Different expiry
        };
        
        let serialized_diff_expiry = payment_auth_diff_expiry.try_to_vec().unwrap();
        assert_ne!(serialized1, serialized_diff_expiry, "Different expiry times should produce different serializations");
    }

    #[test]
    fn test_payment_auth_message_format() {
        let alice = Pubkey::new_unique();
        let bob = Pubkey::new_unique();
        
        let payment_auth = PaymentAuthorization {
            from: alice,
            to: bob,
            amount: 1000000,
            nonce: [5u8; 32],
            valid_until: 1234567890,
        };
        
        // This is how the message should be constructed for signing
        let message = payment_auth.try_to_vec().unwrap();
        
        // Verify the message contains all fields in order
        assert_eq!(&message[0..32], alice.as_ref(), "First 32 bytes should be from pubkey");
        assert_eq!(&message[32..64], bob.as_ref(), "Next 32 bytes should be to pubkey");
        
        // Amount as little-endian u64
        let amount_bytes = 1000000u64.to_le_bytes();
        assert_eq!(&message[64..72], &amount_bytes, "Next 8 bytes should be amount");
        
        // Nonce
        assert_eq!(&message[72..104], &[5u8; 32], "Next 32 bytes should be nonce");
        
        // Valid until as little-endian i64
        let valid_until_bytes = 1234567890i64.to_le_bytes();
        assert_eq!(&message[104..112], &valid_until_bytes, "Last 8 bytes should be valid_until");
    }
}