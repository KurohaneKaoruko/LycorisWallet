//! SLIP-0010 ed25519 private derivation (hardened paths only).

use crate::error::{Error, Result};
use hmac::{Hmac, Mac};
use sha2::Sha512;

type HmacSha512 = Hmac<Sha512>;

#[derive(Clone)]
pub struct ExtendedEd25519 {
    pub key: [u8; 32],
    pub chain_code: [u8; 32],
}

impl std::fmt::Debug for ExtendedEd25519 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtendedEd25519")
            .field("key", &"[redacted]")
            .field("chain_code", &"[redacted]")
            .finish()
    }
}

fn hmac(key: &[u8], data: &[u8]) -> Result<[u8; 64]> {
    let mut mac = HmacSha512::new_from_slice(key)
        .map_err(|e| Error::Crypto(format!("HMAC setup failed: {e}")))?;
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut bytes = [0u8; 64];
    bytes.copy_from_slice(&out);
    Ok(bytes)
}

pub fn master_from_seed(seed: &[u8]) -> Result<ExtendedEd25519> {
    if seed.len() < 16 {
        return Err(Error::Crypto("seed is too short".into()));
    }
    let out = hmac(b"ed25519 seed", seed)?;
    let mut key = [0u8; 32];
    let mut chain_code = [0u8; 32];
    key.copy_from_slice(&out[..32]);
    chain_code.copy_from_slice(&out[32..]);
    Ok(ExtendedEd25519 { key, chain_code })
}

pub fn derive_path(seed: &[u8], path: &str) -> Result<ExtendedEd25519> {
    let mut key = master_from_seed(seed)?;
    let mut parts = path.split('/');
    let root = parts
        .next()
        .ok_or_else(|| Error::Crypto("empty derivation path".into()))?;
    if root != "m" && root != "M" {
        return Err(Error::Crypto(format!("path must start with m: {path}")));
    }
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
        if !hardened {
            return Err(Error::Crypto(
                "SLIP-0010 ed25519 only supports hardened derivation".into(),
            ));
        }
        let index: u32 = digits
            .parse()
            .map_err(|_| Error::Crypto(format!("invalid path segment: {part}")))?;
        if index >= 0x8000_0000 {
            return Err(Error::Crypto("path index is too large".into()));
        }
        key = derive_child(&key, index + 0x8000_0000)?;
    }
    Ok(key)
}

pub fn derive_child(parent: &ExtendedEd25519, index: u32) -> Result<ExtendedEd25519> {
    if index < 0x8000_0000 {
        return Err(Error::Crypto(
            "SLIP-0010 ed25519 only supports hardened derivation".into(),
        ));
    }
    let mut data = Vec::with_capacity(37);
    data.push(0x00);
    data.extend_from_slice(&parent.key);
    data.extend_from_slice(&index.to_be_bytes());
    let out = hmac(&parent.chain_code, &data)?;
    let mut key = [0u8; 32];
    let mut chain_code = [0u8; 32];
    key.copy_from_slice(&out[..32]);
    chain_code.copy_from_slice(&out[32..]);
    Ok(ExtendedEd25519 { key, chain_code })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEED: [u8; 16] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f,
    ];

    #[test]
    fn slip10_ed25519_vector_1() {
        let master = master_from_seed(&SEED).unwrap();
        assert_eq!(
            hex::encode(master.key),
            "2b4be7f19ee27bbf30c667b642d5f4aa69fd169872f8fc3059c08ebae2eb19e7"
        );
        assert_eq!(
            hex::encode(master.chain_code),
            "90046a93de5380a72b5e45010748567d5ea02bbf6522f979e05c0d8d8ca9fffb"
        );

        let child = derive_path(&SEED, "m/0'").unwrap();
        assert_eq!(
            hex::encode(child.key),
            "68e0fe46dfb67e368c75379acec591dad19df3cde26e63b93a8e704f1dade7a3"
        );
        assert_eq!(
            hex::encode(child.chain_code),
            "8b59aa11380b624e81507a27fedda59fea6d0b779a778918a2fd3590e16e9c69"
        );
    }

    #[test]
    fn slip10_rejects_non_hardened() {
        assert!(derive_path(&SEED, "m/44'/501'/0'/0").is_err());
    }
}
