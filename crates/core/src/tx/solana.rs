//! Solana legacy system-transfer transaction builder.
//!
//! Solana wallets derive ed25519 keys from a 32-byte seed with SLIP-0010 (or,
//! in passphrase mode, directly from the chain hash). The wire format is
//! compact; only the system program transfer instruction is needed.

use crate::error::{Error, Result};
use ed25519_dalek::{Signer, SigningKey};
use sha2::Digest;

const SYSTEM_PROGRAM_ID: [u8; 32] = [0u8; 32];
const TRANSFER_INSTRUCTION: u32 = 2;

/// Solana short_vec length encoding.
pub fn shortvec(mut value: usize) -> Vec<u8> {
    let mut out = Vec::new();
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            break;
        } else {
            byte |= 0x80;
            out.push(byte);
        }
    }
    out
}

pub fn parse_blockhash(blockhash: &str) -> Result<[u8; 32]> {
    let bytes = bs58::decode(blockhash.trim())
        .into_vec()
        .map_err(|_| Error::Network("invalid recent blockhash".into()))?;
    if bytes.len() != 32 {
        return Err(Error::Network("recent blockhash must be 32 bytes".into()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub struct BuiltTransfer {
    pub transaction_base64: String,
    pub message_base64: String,
    pub signature_base64: String,
}

pub fn build_transfer(
    from_seed: &[u8; 32],
    to: &[u8; 32],
    lamports: u64,
    recent_blockhash: &str,
) -> Result<BuiltTransfer> {
    if lamports == 0 {
        return Err(Error::InvalidAmount(
            "amount must be greater than zero".into(),
        ));
    }
    let signing = SigningKey::from_bytes(from_seed);
    let from_pubkey = signing.verifying_key().to_bytes();
    let blockhash = parse_blockhash(recent_blockhash)?;

    let mut message = Vec::with_capacity(128);
    message.extend_from_slice(&[1, 0, 1]);
    message.extend_from_slice(&shortvec(3));
    message.extend_from_slice(&from_pubkey);
    message.extend_from_slice(to);
    message.extend_from_slice(&SYSTEM_PROGRAM_ID);
    message.extend_from_slice(&blockhash);
    message.extend_from_slice(&shortvec(1));
    // Instruction: transfer
    message.push(2); // program id index (system program)
    message.extend_from_slice(&shortvec(2));
    message.push(0); // from
    message.push(1); // to
    let mut data = Vec::with_capacity(12);
    data.extend_from_slice(&TRANSFER_INSTRUCTION.to_le_bytes());
    data.extend_from_slice(&lamports.to_le_bytes());
    message.extend_from_slice(&shortvec(data.len()));
    message.extend_from_slice(&data);

    let signature = signing.sign(&message);
    let signature_bytes = signature.to_bytes();

    let mut transaction = Vec::with_capacity(1 + 64 + message.len());
    transaction.extend_from_slice(&shortvec(1));
    transaction.extend_from_slice(&signature_bytes);
    transaction.extend_from_slice(&message);

    use base64::Engine;
    let engine = base64::engine::general_purpose::STANDARD;
    Ok(BuiltTransfer {
        transaction_base64: engine.encode(&transaction),
        message_base64: engine.encode(&message),
        signature_base64: engine.encode(signature_bytes),
    })
}

/// Transaction hash used by explorers is the base58 of the first signature.
pub fn signature_base58(from_seed: &[u8; 32], message: &[u8]) -> Result<String> {
    let signing = SigningKey::from_bytes(from_seed);
    Ok(bs58::encode(signing.sign(message).to_bytes()).into_string())
}

#[allow(dead_code)]
fn sha256_hash(data: &[u8]) -> [u8; 32] {
    let digest = sha2::Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signature, Verifier};

    #[test]
    fn builds_and_verifies_transfer() {
        let seed = [7u8; 32];
        let to = [9u8; 32];
        let blockhash = bs58::encode([3u8; 32]).into_string();
        let built = build_transfer(&seed, &to, 1_000_000, &blockhash).unwrap();
        use base64::Engine;
        let engine = base64::engine::general_purpose::STANDARD;
        let transaction = engine.decode(&built.transaction_base64).unwrap();
        let message = engine.decode(&built.message_base64).unwrap();
        // Shortvec signature count, 64-byte signature, then the message.
        assert_eq!(transaction[0], 1);
        let signature = Signature::from_slice(&transaction[1..65]).unwrap();
        let signing = SigningKey::from_bytes(&seed);
        signing
            .verifying_key()
            .verify(&message, &signature)
            .expect("signature must verify");
        // Header: one signer, one readonly unsigned account (system program).
        assert_eq!(&message[..3], &[1, 0, 1]);
    }

    /// The same transfer compiled and signed with `solders` 0.27 (the official
    /// Solana SDK bindings); see `tools/verify_tx.py`.
    #[test]
    fn transfer_matches_solders() {
        let blockhash = bs58::encode([3u8; 32]).into_string();
        let built = build_transfer(&[7u8; 32], &[9u8; 32], 1_000_000, &blockhash).unwrap();
        assert_eq!(
            built.message_base64,
            "AQABA+pKbGPinFIKvvVQexMuxfmVR3auvr57kkIe6mkURtIsCQkJCQkJCQkJCQkJCQkJCQkJ\
             CQkJCQkJCQkJCQkJCQkAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAMDAwMDAwMD\
             AwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAQICAAEMAgAAAEBCDwAAAAAA"
        );
        assert_eq!(
            built.signature_base64,
            "hbPGJKIOHtF1q+V5Yc4zwZtoi+LnzJ8JLpxQclPTEHfPk+nfdWNs2dRlfPX4lUDHO1IPusLU\
             X4Qlgdtz4vUTAA=="
        );
        assert_eq!(
            built.transaction_base64,
            "AYWzxiSiDh7RdavleWHOM8GbaIvi58yfCS6cUHJT0xB3z5Pp33VjbNnUZXz1+JVAxztSD7rC\
             1F+EJYHbc+L1EwABAAED6kpsY+KcUgq+9VB7Ey7F+ZVHdq6+vnuSQh7qaRRG0iwJCQkJCQkJ\
             CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\
             AAAAAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMDAwMBAgIAAQwCAAAAQEIPAAAAAAA="
        );
    }
}
