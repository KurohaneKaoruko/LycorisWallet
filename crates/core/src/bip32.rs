//! Minimal BIP-32 private derivation for secp256k1.
//!
//! Only private parent keys are needed by a wallet that signs locally, so the
//! public derivation and serialisation code from the full BIP are omitted.

use crate::error::{Error, Result};
use hmac::{Hmac, Mac};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::elliptic_curve::PrimeField;
use k256::{FieldBytes, Scalar, SecretKey};
use sha2::Sha512;

type HmacSha512 = Hmac<Sha512>;

#[derive(Clone)]
pub struct ExtendedPriv {
    pub key: SecretKey,
    pub chain_code: [u8; 32],
}

impl std::fmt::Debug for ExtendedPriv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtendedPriv")
            .field("key", &"[redacted]")
            .field("chain_code", &"[redacted]")
            .finish()
    }
}

pub fn master_from_seed(seed: &[u8]) -> Result<ExtendedPriv> {
    if seed.len() < 16 {
        return Err(Error::Crypto("seed is too short".into()));
    }
    let mut mac = HmacSha512::new_from_slice(b"Bitcoin seed")
        .map_err(|e| Error::Crypto(format!("HMAC setup failed: {e}")))?;
    mac.update(seed);
    let out = mac.finalize().into_bytes();
    let key = SecretKey::from_slice(&out[..32])
        .map_err(|_| Error::Crypto("invalid BIP32 master key".into()))?;
    let mut chain_code = [0u8; 32];
    chain_code.copy_from_slice(&out[32..]);
    Ok(ExtendedPriv { key, chain_code })
}

/// Parse a path such as `m/84'/0'/0'/0/0`.
pub fn parse_path(path: &str) -> Result<Vec<u32>> {
    let mut parts = path.split('/');
    let root = parts
        .next()
        .ok_or_else(|| Error::Crypto("empty derivation path".into()))?;
    if root != "m" && root != "M" {
        return Err(Error::Crypto(format!("path must start with m: {path}")));
    }
    let mut out = Vec::new();
    for part in parts {
        if part.is_empty() {
            return Err(Error::Crypto(format!("invalid derivation path: {path}")));
        }
        let (digits, hardened) = if let Some(stripped) = part.strip_suffix('\'') {
            (stripped, true)
        } else if let Some(stripped) = part.strip_suffix('h') {
            (stripped, true)
        } else {
            (part, false)
        };
        let index: u32 = digits
            .parse()
            .map_err(|_| Error::Crypto(format!("invalid path segment: {part}")))?;
        if index >= 0x8000_0000 {
            return Err(Error::Crypto("path index is too large".into()));
        }
        out.push(if hardened { index + 0x8000_0000 } else { index });
    }
    Ok(out)
}

pub fn derive_path(seed: &[u8], path: &str) -> Result<ExtendedPriv> {
    let mut key = master_from_seed(seed)?;
    for index in parse_path(path)? {
        key = derive_child(&key, index)?;
    }
    Ok(key)
}

pub fn derive_child(parent: &ExtendedPriv, index: u32) -> Result<ExtendedPriv> {
    let hardened = index >= 0x8000_0000;
    let mut data = Vec::with_capacity(37);
    if hardened {
        data.push(0x00);
        data.extend_from_slice(&parent.key.to_bytes());
    } else {
        let point = parent.key.public_key().to_encoded_point(true);
        data.extend_from_slice(point.as_bytes());
    }
    data.extend_from_slice(&index.to_be_bytes());

    let mut mac = HmacSha512::new_from_slice(&parent.chain_code)
        .map_err(|e| Error::Crypto(format!("HMAC setup failed: {e}")))?;
    mac.update(&data);
    let out = mac.finalize().into_bytes();

    let bytes = FieldBytes::from_slice(&out[..32]);
    let il = Option::<Scalar>::from(Scalar::from_repr(*bytes))
        .ok_or_else(|| Error::Crypto("invalid BIP32 child key (IL >= n)".into()))?;
    if bool::from(il.is_zero()) {
        return Err(Error::Crypto("invalid BIP32 child key (IL = 0)".into()));
    }
    let parent_scalar = *parent.key.to_nonzero_scalar();
    let child_scalar = il + parent_scalar;
    if bool::from(child_scalar.is_zero()) {
        return Err(Error::Crypto("invalid BIP32 child key (zero)".into()));
    }
    let child_key = SecretKey::from_slice(&child_scalar.to_bytes())
        .map_err(|_| Error::Crypto("invalid BIP32 child key".into()))?;
    let mut chain_code = [0u8; 32];
    chain_code.copy_from_slice(&out[32..]);
    Ok(ExtendedPriv {
        key: child_key,
        chain_code,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 16] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f,
    ];

    #[test]
    fn bip32_vector_1_master_and_child() {
        let master = master_from_seed(&SEED).unwrap();
        assert_eq!(
            hex::encode(master.key.to_bytes()),
            "e8f32e723decf4051aefac8e2c93c9c5b214313817cdb01a1494b917c8436b35"
        );
        assert_eq!(
            hex::encode(master.chain_code),
            "873dff81c02f525623fd1fe5167eac3a55a049de3d314bb42ee227ffed37d508"
        );

        let child = derive_path(&SEED, "m/0'").unwrap();
        assert_eq!(
            hex::encode(child.key.to_bytes()),
            "edb2e14f9ee77d26dd93b4ecede8d16ed408ce149b6cd80b0715a2d911a0afea"
        );
        assert_eq!(
            hex::encode(child.chain_code),
            "47fdacbd0f1097043b78c63c20c34ef4ed9a111d980047ad16282c7ae6236141"
        );
    }

    #[test]
    fn path_parser_rejects_bad_paths() {
        assert!(parse_path("84'/0'").is_err());
        assert!(parse_path("m/84'/0'").unwrap().len() == 2);
        assert!(parse_path("m/2147483648'").is_err());
    }
}
