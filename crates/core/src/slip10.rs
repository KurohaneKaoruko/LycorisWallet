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

/* ---------------------------------------------------- Khovratovich/Law ed25519 */

/// BIP32-Khovratovich/Law ("kholaw") derivation, the unclamped ed25519 variant
/// Monero wallets use. Unlike SLIP-0010 it supports non-hardened derivation by
/// multiplying scalar halves, and the private key stays a raw 64-byte
/// `kL ‖ kR` pair; only `kL` is used as the spend seed.
///
/// Reference: "Khovratovich/Law BIP32-Ed25519" as implemented by bip_utils.
pub struct ExtendedKholaw {
    /// 64 bytes: left half `kL` (the scalar actually used by Monero).
    pub key: [u8; 64],
    pub chain_code: [u8; 32],
}

impl std::fmt::Debug for ExtendedKholaw {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExtendedKholaw")
            .field("key", &"[redacted]")
            .field("chain_code", &"[redacted]")
            .finish()
    }
}

impl Drop for ExtendedKholaw {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.key.zeroize();
        self.chain_code.zeroize();
    }
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Result<[u8; 32]> {
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key)
        .map_err(|e| Error::Crypto(format!("HMAC setup failed: {e}")))?;
    mac.update(data);
    let out = mac.finalize().into_bytes();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&out);
    Ok(bytes)
}

/// Minimal 256-bit integer over two u128 halves (little-endian), enough for
/// kholaw's `8 * zl + kL` and `zr + kR` arithmetic (mod 2^256).
#[derive(Clone, Copy, PartialEq, Eq)]
struct U256(u128, u128);

impl U256 {
    fn from_le_bytes(bytes: &[u8]) -> Self {
        let mut lo = [0u8; 16];
        let mut hi = [0u8; 16];
        let len = bytes.len().min(32);
        lo[..len.min(16)].copy_from_slice(&bytes[..len.min(16)]);
        if len > 16 {
            hi[..len - 16].copy_from_slice(&bytes[16..len]);
        }
        U256(u128::from_le_bytes(lo), u128::from_le_bytes(hi))
    }

    fn to_le_bytes(self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[..16].copy_from_slice(&self.0.to_le_bytes());
        out[16..].copy_from_slice(&self.1.to_le_bytes());
        out
    }

    /// self + other mod 2^256.
    fn wrapping_add(self, other: Self) -> Self {
        let (lo, carry) = self.0.overflowing_add(other.0);
        let hi = self.1.wrapping_add(other.1).wrapping_add(u128::from(carry));
        U256(lo, hi)
    }

    /// self * 8 mod 2^256 (the only multiplier kholaw needs).
    fn wrapping_mul_8(self) -> Self {
        // x * 8 == x << 3, so the bits carried into the high half are
        // exactly the top 3 bits of the low half.
        let (lo, _) = self.0.overflowing_mul(8);
        let carry = self.0 >> 125;
        let hi = self.1.wrapping_mul(8).wrapping_add(carry);
        U256(lo, hi)
    }

    fn is_zero(self) -> bool {
        self.0 == 0 && self.1 == 0
    }

    fn eq_bytes_le(self, bytes: &[u8; 32]) -> bool {
        self.to_le_bytes() == *bytes
    }
}

/// edwards25519 group order `l` (little-endian form of 2^252 + 27742...493).
const ED25519_ORDER_LE: [u8; 32] = [
    0xed, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9, 0xde, 0x14,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10,
];

fn le_mod_order(bytes: &[u8; 32]) -> [u8; 32] {
    let value = U256::from_le_bytes(bytes);
    // kholaw only needs the "multiple of order" test. A uniform random value
    // is never a multiple of l except 0 and l itself (l > 2^252).
    if value.is_zero() || value.eq_bytes_le(&ED25519_ORDER_LE) {
        return [0u8; 32];
    }
    *bytes
}

/// Generate the kholaw master key from a seed.
pub fn kholaw_master_from_seed(seed: &[u8]) -> Result<ExtendedKholaw> {
    if seed.len() < 16 {
        return Err(Error::Crypto("seed is too short".into()));
    }
    // kl ‖ kr = HMAC-SHA512("ed25519 seed", data), rehashing while the
    // third-highest bit of kl[31] is set.
    let mut data = seed.to_vec();
    let mut halves = [0u8; 64];
    loop {
        let out = hmac(b"ed25519 seed", &data)?;
        halves.copy_from_slice(&out);
        if halves[31] & 0x20 == 0 {
            break;
        }
        data = halves.to_vec();
    }
    let mut key = halves;
    // Tweak kL: clear lowest 3 bits of byte 0, clear highest & set
    // second-highest bit of byte 31.
    key[0] &= !0x07;
    key[31] &= !0x80;
    key[31] |= 0x40;
    let chain_code = {
        let mut buf = Vec::with_capacity(seed.len() + 1);
        buf.push(0x01);
        buf.extend_from_slice(seed);
        hmac_sha256(b"ed25519 seed", &buf)?
    };
    Ok(ExtendedKholaw { key, chain_code })
}

fn kholaw_public_key(key: &[u8; 64]) -> [u8; 32] {
    // pub = kL * G with kL reduced mod l (unclamped scalar).
    use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
    use curve25519_dalek::scalar::Scalar;
    let mut scalar_bytes = [0u8; 32];
    scalar_bytes.copy_from_slice(&key[..32]);
    let scalar = Scalar::from_bytes_mod_order(scalar_bytes);
    (scalar * ED25519_BASEPOINT_POINT).compress().to_bytes()
}

/// Derive a child key (hardened or normal) per kholaw.
pub fn kholaw_derive_child(parent: &ExtendedKholaw, index: u32) -> Result<ExtendedKholaw> {
    let hardened = index >= 0x8000_0000;
    let index_le = index.to_le_bytes();
    let public = kholaw_public_key(&parent.key);

    let mut z_input = Vec::with_capacity(1 + 64 + 4);
    let mut cc_input = Vec::with_capacity(1 + 64 + 4);
    if hardened {
        z_input.push(0x00);
        cc_input.push(0x01);
        z_input.extend_from_slice(&parent.key);
        cc_input.extend_from_slice(&parent.key);
    } else {
        z_input.push(0x02);
        cc_input.push(0x03);
        z_input.extend_from_slice(&public);
        cc_input.extend_from_slice(&public);
    }
    z_input.extend_from_slice(&index_le);
    cc_input.extend_from_slice(&index_le);

    let z = hmac(&parent.chain_code, &z_input)?;
    let cc_out = hmac(&parent.chain_code, &cc_input)?;
    let mut chain_code = [0u8; 32];
    chain_code.copy_from_slice(&cc_out[32..]);

    let zr = U256::from_le_bytes(&z[32..]);
    let kl = U256::from_le_bytes(&parent.key[..32]);
    let kr = U256::from_le_bytes(&parent.key[32..]);

    // kL' = 8 * zl[:28] + kL   (kholaw uses only the first 28 bytes of zl)
    let mut zl28 = [0u8; 32];
    zl28[..28].copy_from_slice(&z[..28]);
    let kl_new = U256::from_le_bytes(&zl28).wrapping_mul_8().wrapping_add(kl);
    // kR' = zr + kR mod 2^256
    let kr_new = zr.wrapping_add(kr);

    let mut key = [0u8; 64];
    key[..32].copy_from_slice(&kl_new.to_le_bytes());
    key[32..].copy_from_slice(&kr_new.to_le_bytes());
    // Discard the child if kL' is a multiple of the group order.
    let mut kl_check = [0u8; 32];
    kl_check.copy_from_slice(&key[..32]);
    if le_mod_order(&kl_check) == [0u8; 32] {
        return Err(Error::Crypto(
            "kholaw child key is a multiple of the curve order".into(),
        ));
    }
    Ok(ExtendedKholaw { key, chain_code })
}

/// Derive a kholaw path like `m/44'/128'/0'/0/0` (mixed hardened/normal).
pub fn derive_kholaw_path(seed: &[u8], path: &str) -> Result<[u8; 64]> {
    let mut node = kholaw_master_from_seed(seed)?;
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
        let index: u32 = digits
            .parse()
            .map_err(|_| Error::Crypto(format!("invalid path segment: {part}")))?;
        if index >= 0x8000_0000 {
            return Err(Error::Crypto("path index is too large".into()));
        }
        node = kholaw_derive_child(&node, if hardened { index + 0x8000_0000 } else { index })?;
    }
    Ok(node.key)
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

    #[test]
    fn kholaw_bip44_monero_matches_bip_utils() {
        // Independent vector: bip_utils Bip32KholawEd25519 on the BIP-39 seed of
        // the standard test mnemonic, path m/44'/128'/0'/0/0.
        let mnemonic =
            "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let seed = bip39::Mnemonic::parse_normalized(mnemonic)
            .unwrap()
            .to_seed("");
        let derived = derive_kholaw_path(&seed, "m/44'/128'/0'/0/0").unwrap();
        assert_eq!(
            hex::encode(derived),
            "d0c0987bdf08c215dae48c1a2a009d03910c57753bcdfed8ed6fcd5dbb27aa5c6463e38dca60da861e116a2b671233c9b4e9f6a81e25f7d5b319ad36870ab3e6"
        );
    }
}
