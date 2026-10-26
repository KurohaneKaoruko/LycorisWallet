//! Message signing and verification for the tools view.
//!
//! Three standards, one goal: prove ownership of an address to a third party
//! without moving funds.
//!
//! - EVM — EIP-191 `personal_sign` (`\x19Ethereum Signed Message:` prefix),
//!   recovered with ecrecover so any web3 wallet can verify it.
//! - Bitcoin — BIP-137 style: double-SHA-256 of the same magic prefix over a
//!   *compressed* key, signed with low-S, recoverable so the verifier can
//!   reconstruct the pubkey.
//! - Solana — plain ed25519 over the raw message bytes (what most Solana
//!   sign-message flows expect).
//!
//! Signing never touches the vault: callers hand over key material they have
//! already unlocked, and the result is immediately serialisable public data.

use crate::address::{compressed_pubkey, keccak256};
use crate::error::{Error, Result};
use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
use k256::SecretKey;
use serde::{Deserialize, Serialize};

/* ------------------------------------------------------------------- EVM (EIP-191) */

fn eip191_payload(message: &[u8]) -> Vec<u8> {
    let prefix = format!("\x19Ethereum Signed Message:\n{}", message.len());
    let mut payload = Vec::with_capacity(prefix.len() + message.len());
    payload.extend_from_slice(prefix.as_bytes());
    payload.extend_from_slice(message);
    payload
}

/// `personal_sign` style signature, 65 bytes `r ‖ s ‖ v` (v = recovery + 27).
pub fn evm_sign_message(key: &SecretKey, message: &str) -> Result<MessageSignature> {
    let signing = SigningKey::from_slice(&key.to_bytes())
        .map_err(|e| Error::Crypto(format!("invalid signing key: {e}")))?;
    let digest = keccak256(&eip191_payload(message.as_bytes()));
    let (signature, recovery) = signing
        .sign_prehash_recoverable(&digest)
        .map_err(|e| Error::Crypto(format!("ECDSA signing failed: {e}")))?;
    let bytes = signature.to_bytes();
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(&bytes);
    out[64] = recovery.to_byte() + 27;
    Ok(MessageSignature {
        scheme: "eip-191 personal_sign".into(),
        signature_hex: hex::encode(out),
        pubkey: None,
        address: Some(crate::address::evm_address(
            &crate::address::uncompressed_pubkey(key),
        )?),
        recovery: Some(recovery.to_byte()),
    })
}

/// ecrecover the address a `personal_sign` signature came from.
pub fn evm_verify_message(message: &str, signature_hex: &str) -> Result<String> {
    let raw = hex::decode(signature_hex.trim().trim_start_matches("0x"))
        .map_err(|_| Error::Crypto("signature must be hex".into()))?;
    if raw.len() != 65 {
        return Err(Error::Crypto(
            "EVM signature must be 65 bytes (r‖s‖v)".into(),
        ));
    }
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&raw[..64]);
    let v = raw[64];
    let rec_byte = v
        .checked_sub(27)
        .filter(|v| *v < 4)
        .ok_or_else(|| Error::Crypto("v must be 27..=30 (personal_sign convention)".into()))?;
    let recovery_id = RecoveryId::new(rec_byte % 4 == 1, false);
    let signature = Signature::from_bytes(&sig_bytes.into())
        .map_err(|e| Error::Crypto(format!("invalid signature: {e}")))?;
    let digest = keccak256(&eip191_payload(message.as_bytes()));
    let recovered = VerifyingKey::recover_from_prehash(
        &digest,
        &signature
            .try_into()
            .map_err(|e| Error::Crypto(format!("ecdsa signature conversion failed: {e}")))?,
        recovery_id,
    )
    .map_err(|e| Error::Crypto(format!("ecrecover failed: {e}")))?;
    let pubkey = recovered.to_encoded_point(false);
    let address = crate::address::evm_address(pubkey.as_bytes())?;
    Ok(address)
}

/* --------------------------------------------------------------- Bitcoin (BIP-137) */

fn bip137_payload(message: &str) -> Vec<u8> {
    let mut buf = Vec::with_capacity(message.len() + 25);
    buf.extend_from_slice(b"\x18Bitcoin Signed Message:\n");
    buf.push(message.len() as u8);
    buf.extend_from_slice(message.as_bytes());
    buf
}

/// BIP-137 compatible message signature over the compressed key.
pub fn btc_sign_message(key: &SecretKey, message: &str) -> Result<MessageSignature> {
    let signing = SigningKey::from_slice(&key.to_bytes())
        .map_err(|e| Error::Crypto(format!("invalid signing key: {e}")))?;
    let digest = crate::address::double_sha256(&bip137_payload(message));
    let (signature, recovery) = signing
        .sign_prehash_recoverable(&digest)
        .map_err(|e| Error::Crypto(format!("ECDSA signing failed: {e}")))?;
    let bytes = signature.to_bytes();
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(&bytes);
    // BIP-137: 27/28 = compressed key, +4 for the p2wpkh/p2sh variants.
    out[64] = recovery.to_byte() + 27 + 4;
    Ok(MessageSignature {
        scheme: "bip-137 (compressed)".into(),
        signature_hex: hex::encode(out),
        pubkey: Some(hex::encode(compressed_pubkey(key))),
        address: Some(crate::address::p2wpkh_address(
            &compressed_pubkey(key),
            "bc",
        )?),
        recovery: Some(recovery.to_byte()),
    })
}

/// Recover the Bitcoin address a BIP-137 signature proves.
pub fn btc_verify_message(message: &str, signature_b64: &str) -> Result<String> {
    use base64::Engine;
    let raw = base64::engine::general_purpose::STANDARD
        .decode(signature_b64.trim())
        .map_err(|_| Error::Crypto("signature must be base64".into()))?;
    if raw.len() != 65 {
        return Err(Error::Crypto("BIP-137 signature must be 65 bytes".into()));
    }
    let header = raw[64];
    if !(27..=42).contains(&header) {
        return Err(Error::Crypto(
            "invalid BIP-137 header byte (must be 27..=42)".into(),
        ));
    }
    let compressed = header >= 31;
    let rec_byte = (header - 27) % 4;
    let recovery_id = RecoveryId::new(rec_byte == 1, false);
    let mut sig_bytes = [0u8; 64];
    sig_bytes.copy_from_slice(&raw[..64]);
    let signature = Signature::from_bytes(&sig_bytes.into())
        .map_err(|e| Error::Crypto(format!("invalid signature: {e}")))?;
    let digest = crate::address::double_sha256(&bip137_payload(message));
    let recovered = VerifyingKey::recover_from_prehash(
        &digest,
        &signature
            .try_into()
            .map_err(|e| Error::Crypto(format!("ecdsa signature conversion failed: {e}")))?,
        recovery_id,
    )
    .map_err(|e| Error::Crypto(format!("recovery failed: {e}")))?;
    let point = recovered.to_encoded_point(compressed);
    // The app only ever produces P2WPKH addresses; BIP-137 headers 31..=34
    // cover the same compressed key.
    crate::address::p2wpkh_address(point.as_bytes(), "bc")
}

/* ------------------------------------------------------------------- Solana (ed25519) */

pub fn solana_sign_message(seed: &[u8; 32], message: &str) -> Result<MessageSignature> {
    use ed25519_dalek::{Signer, SigningKey, Verifier};
    let signing = SigningKey::from_bytes(seed);
    let signature = signing.sign(message.as_bytes());
    let verifying = signing.verifying_key();
    verifying
        .verify(message.as_bytes(), &signature)
        .map_err(|e| Error::Crypto(format!("ed25519 self-check failed: {e}")))?;
    Ok(MessageSignature {
        scheme: "ed25519 (solana sign_message)".into(),
        signature_hex: hex::encode(signature.to_bytes()),
        pubkey: Some(bs58::encode(verifying.to_bytes()).into_string()),
        address: Some(bs58::encode(verifying.to_bytes()).into_string()),
        recovery: None,
    })
}

pub fn solana_verify_message(address: &str, message: &str, signature_hex: &str) -> Result<bool> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let pubkey_bytes = bs58::decode(address.trim())
        .into_vec()
        .map_err(|_| Error::InvalidAddress("not a base58 ed25519 pubkey".into()))?;
    if pubkey_bytes.len() != 32 {
        return Err(Error::InvalidAddress(
            "ed25519 pubkey must be 32 bytes".into(),
        ));
    }
    let verifying = VerifyingKey::from_bytes(
        pubkey_bytes
            .as_slice()
            .try_into()
            .map_err(|_| Error::InvalidAddress("bad pubkey".into()))?,
    )
    .map_err(|_| Error::InvalidAddress("bad ed25519 pubkey".into()))?;
    let mut sig_bytes = [0u8; 64];
    let raw = hex::decode(signature_hex.trim().trim_start_matches("0x"))
        .map_err(|_| Error::Crypto("signature must be hex".into()))?;
    if raw.len() != 64 {
        return Err(Error::Crypto("ed25519 signature must be 64 bytes".into()));
    }
    sig_bytes.copy_from_slice(&raw);
    let signature = Signature::from_bytes(&sig_bytes);
    Ok(verifying.verify(message.as_bytes(), &signature).is_ok())
}

/* ---------------------------------------------------------------- shared result */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSignature {
    pub scheme: String,
    /// Hex for EVM/Solana; hex for BTC too (UI shows base64 alongside).
    pub signature_hex: String,
    pub pubkey: Option<String>,
    pub address: Option<String>,
    pub recovery: Option<u8>,
}

/// Verification outcome: the recovered signer address plus whether the
/// signature actually checked out (Solana returns `false` instead of erroring
/// when the signature does not match).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VerifyResult {
    pub scheme: String,
    pub signer: String,
    pub verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evm_key() -> SecretKey {
        // "lycoris test key" hashed — deterministic, never funded.
        SecretKey::from_slice(&crate::address::sha256(b"lycoris-tools-test-key")).unwrap()
    }

    #[test]
    fn eip191_round_trip() {
        let key = evm_key();
        let signed = evm_sign_message(&key, "彼岸花").unwrap();
        assert!(signed.address.as_deref().unwrap().starts_with("0x"));
        let recovered = evm_verify_message("彼岸花", &signed.signature_hex).unwrap();
        assert_eq!(recovered, signed.address.unwrap());
    }

    #[test]
    fn bip137_round_trip() {
        use base64::Engine;
        let key = evm_key();
        let signed = btc_sign_message(&key, "hello lycoris").unwrap();
        // MessageSignature carries hex; convert to base64 for the verifier.
        let raw = hex::decode(&signed.signature_hex).unwrap();
        assert_eq!(raw.len(), 65);
        assert!((31..=34).contains(&raw[64]));
        let b64 = base64::engine::general_purpose::STANDARD.encode(raw);
        let recovered = btc_verify_message("hello lycoris", &b64).unwrap();
        assert_eq!(recovered, signed.address.unwrap());
    }

    #[test]
    fn solana_round_trip() {
        let seed = crate::address::sha256(b"lycoris-tools-solana-seed");
        let signed = solana_sign_message(&seed, "sign me").unwrap();
        assert!(solana_verify_message(
            signed.address.as_deref().unwrap(),
            "sign me",
            &signed.signature_hex
        )
        .unwrap());
        assert!(!solana_verify_message(
            signed.address.as_deref().unwrap(),
            "tampered",
            &signed.signature_hex
        )
        .unwrap());
    }

    #[test]
    fn eip191_rejects_garbage() {
        assert!(evm_verify_message("m", "00ff").is_err());
    }
}
