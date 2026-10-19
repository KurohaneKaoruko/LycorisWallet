//! CryptoNote / Monero primitives: dual-key derivation and Monero base58.
//!
//! Monero is deliberately unlike every other chain here. There is no address
//! reuse, spending needs a one-time key image, and the address itself encodes
//! *two* public keys (spend + view). This module implements just enough for
//! deterministic receive addresses:
//!
//! - `sc_reduce32`: interpret 32 bytes as a little-endian scalar mod the
//!   edwards25519 group order `l` (the Monero `sc_reduce32`, *not* the RFC
//!   8032 clamping used by Ed25519 signatures).
//! - dual keys: `spend = reduce(H(seed))`, `view = reduce(H(spend))`.
//! - public keys: plain scalar multiplication on edwards25519 without
//!   clamping, compressed to 32 bytes.
//! - primary address: Monero base58 (11-bit blocks, block-size suffix,
//!   Keccak-4 checksum) over `0x12 ‖ pub_spend ‖ pub_view`.

use crate::address::keccak256;
use curve25519_dalek::constants::ED25519_BASEPOINT_POINT;
use curve25519_dalek::scalar::Scalar;

/// Monero mainnet network prefix for a primary address.
pub const MAINNET_ADDRESS_PREFIX: u8 = 0x12;

const BASE58_ALPHABET: &[u8; 58] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

/// Monero's `sc_reduce32`: little-endian integer mod the group order.
pub fn sc_reduce32(bytes: &[u8; 32]) -> [u8; 32] {
    let scalar = Scalar::from_bytes_mod_order(*bytes);
    scalar.to_bytes()
}

/// Monero point multiplication without clamping: `scalar * G`, 32-byte form.
fn point_mul(scalar_bytes: &[u8; 32]) -> [u8; 32] {
    // `from_bits` is deliberately avoided; `from_bytes_mod_order` matches the
    // Monero behaviour because the input is already reduced.
    let scalar = Scalar::from_canonical_bytes(*scalar_bytes)
        .unwrap_or_else(|| Scalar::from_bytes_mod_order(*scalar_bytes));
    (scalar * ED25519_BASEPOINT_POINT).compress().to_bytes()
}

/// A Monero keypair: private spend + derived private view key.
#[derive(Clone)]
pub struct MoneroKeys {
    pub private_spend: [u8; 32],
    pub private_view: [u8; 32],
    pub public_spend: [u8; 32],
    pub public_view: [u8; 32],
}

impl std::fmt::Debug for MoneroKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MoneroKeys")
            .field("private_spend", &"[redacted]")
            .field("private_view", &"[redacted]")
            .field("public_spend", &hex::encode(self.public_spend))
            .field("public_view", &hex::encode(self.public_view))
            .finish()
    }
}

impl Drop for MoneroKeys {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.private_spend.zeroize();
        self.private_view.zeroize();
    }
}

impl MoneroKeys {
    /// Derive from a private spend seed (`spend = reduce(seed)`).
    pub fn from_spend_seed(seed: &[u8; 32]) -> Self {
        let private_spend = sc_reduce32(seed);
        let private_view = sc_reduce32(&keccak256(&private_spend));
        let public_spend = point_mul(&private_spend);
        let public_view = point_mul(&private_view);
        Self {
            private_spend,
            private_view,
            public_spend,
            public_view,
        }
    }

    /// Derive from a full 64-byte kholaw extended key, following the
    /// bip_utils / hardware-wallet convention:
    /// `spend = reduce(Keccak-256(kL ‖ kR))`.
    pub fn from_kholaw_key(key: &[u8; 64]) -> Self {
        Self::from_spend_seed(&keccak256(key))
    }

    /// The primary (index 0/0) address in Monero base58.
    pub fn primary_address(&self) -> String {
        let mut data = Vec::with_capacity(69);
        data.push(MAINNET_ADDRESS_PREFIX);
        data.extend_from_slice(&self.public_spend);
        data.extend_from_slice(&self.public_view);
        // CryptoNote checksum: first 4 bytes of Keccak-0 (original, not
        // SHA3) over prefix ‖ spend ‖ view.
        let checksum = keccak256(&data);
        data.extend_from_slice(&checksum[..4]);
        monero_base58_encode(&data)
    }
}

/// Monero base58: the payload is split into 8-byte blocks (the last block may
/// be shorter), each encoded independently so the output length is fixed per
/// block size; a shorter block is left-padded with '1'. No checksum suffix —
/// the block boundaries are recoverable from the total length.
pub fn monero_base58_encode(data: &[u8]) -> String {
    let encoded_block = |block: &[u8]| -> String {
        let mut num: u64 = 0;
        for byte in block {
            num = (num << 8) | u64::from(*byte);
        }
        // Encoded size table from the Monero reference (block size → chars).
        let encoded_size = match block.len() {
            0 => 0,
            1 => 2,
            2 => 3,
            3 => 5,
            4 => 6,
            5 => 7,
            6 => 9,
            7 => 10,
            _ => 11,
        };
        let mut chars = vec![b'1'; encoded_size];
        let mut rest = num;
        for slot in chars.iter_mut().rev() {
            *slot = BASE58_ALPHABET[(rest % 58) as usize];
            rest /= 58;
        }
        String::from_utf8(chars).expect("alphabet is ascii")
    };

    let mut out = String::with_capacity(95);
    for block in data.chunks(8) {
        out.push_str(&encoded_block(block));
    }
    out
}

/// Validate and decode a Monero primary address (mainnet, 0x12 prefix).
pub fn monero_address_validate(address: &str) -> bool {
    let decoded = match monero_base58_decode(address) {
        Some(bytes) => bytes,
        None => return false,
    };
    if decoded.len() != 69 || decoded[0] != MAINNET_ADDRESS_PREFIX {
        return false;
    }
    // checksum: Keccak-4 over prefix + spend + view
    let checksum = keccak256(&decoded[..65]);
    decoded[65..] == checksum[..4]
}

/// Decode Monero base58 back into raw bytes (checksum not verified here).
/// Encoding size table: 8-byte blocks → 11 chars, and the trailing partial
/// block maps 1→2, 2→3, 3→5, 4→6, 5→7, 6→9, 7→10 chars.
fn monero_base58_decode(text: &str) -> Option<Vec<u8>> {
    if text.is_empty() || text.len() > 96 {
        return None;
    }
    const FULL_BLOCK_CHARS: usize = 11;
    const ENCODED_SIZES: [(usize, usize); 8] = [
        (0, 0),
        (1, 2),
        (2, 3),
        (3, 5),
        (4, 6),
        (5, 7),
        (6, 9),
        (7, 10),
    ];
    let char_value = |c: u8| -> Option<u64> {
        // Alphabet (no 0, O, I, l), verified by enumeration:
        // 123456789 ABCDEFGH JKLMN PQRSTUVWXYZ abcdefghijk mnopqrstuvwxyz
        match c {
            b'1'..=b'9' => Some(u64::from(c - b'1')),       // 0..=8
            b'A'..=b'H' => Some(u64::from(c - b'A') + 9),   // 9..=16
            b'J'..=b'N' => Some(u64::from(c - b'J') + 17),  // 17..=21
            b'P'..=b'Z' => Some(u64::from(c - b'P') + 22),  // 22..=32
            b'a'..=b'k' => Some(u64::from(c - b'a') + 33),  // 33..=43
            b'm'..=b'z' => Some(u64::from(c - b'm') + 44),  // 44..=57
            _ => None,
        }
    };
    let decoded_block = |block: &[u8], byte_len: usize| -> Option<Vec<u8>> {
        let mut num: u64 = 0;
        for &c in block {
            // '1' is index 0 in the alphabet (also used as zero padding).
            let value = if c == b'1' {
                0
            } else {
                char_value(c)?
            };
            num = num.checked_mul(58)?.checked_add(value)?;
        }
        let bytes = num.to_be_bytes();
        Some(bytes[8 - byte_len..].to_vec())
    };
    let bytes = text.as_bytes();
    let full_blocks = bytes.len() / FULL_BLOCK_CHARS;
    let tail_chars = bytes.len() % FULL_BLOCK_CHARS;
    let tail_bytes = ENCODED_SIZES
        .iter()
        .find(|(_, chars)| *chars == tail_chars)
        .map(|(byte_len, _)| *byte_len)?;
    let mut out = Vec::with_capacity(full_blocks * 8 + tail_bytes);
    for block in bytes[..full_blocks * FULL_BLOCK_CHARS].chunks(FULL_BLOCK_CHARS) {
        out.extend_from_slice(&decoded_block(block, 8)?);
    }
    if tail_chars > 0 {
        let tail = &bytes[full_blocks * FULL_BLOCK_CHARS..];
        out.extend_from_slice(&decoded_block(tail, tail_bytes)?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_to_bytes(text: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        let bytes = hex::decode(text).unwrap();
        out.copy_from_slice(&bytes);
        out
    }

    #[test]
    fn monero_base58_block_layout() {
        // Single full block roundtrip sanity (python cross-checked:
        // 0xAB×8 → "ViRM378GHFx").
        let data = [0xABu8; 8];
        let encoded = monero_base58_encode(&data);
        assert_eq!(encoded, "ViRM378GHFx");
        assert_eq!(monero_base58_decode(&encoded).unwrap(), data.to_vec());

        // 69 bytes = 8 full blocks + 5-byte tail → 8*11 + 7 = 95 chars.
        let full = vec![0xABu8; 69];
        let encoded69 = monero_base58_encode(&full);
        assert_eq!(encoded69.len(), 95);
        assert_eq!(monero_base58_decode(&encoded69).unwrap(), full);
    }

    #[test]
    fn monero_primary_address_from_hex_seed() {
        // Independent vector produced with bip_utils
        // (Monero.FromSeed: spend = reduce(Keccak(seed))).
        let seed = hex_to_bytes("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let keys = MoneroKeys::from_spend_seed(&keccak256(&seed));
        assert_eq!(
            keys.primary_address(),
            "436SWnkB5AVF5ASVAa2SLuAVf8YwdKbsreFgiaTSrMKJcfNwiXqKVXdESqbUkbWmF49sgtQDPFAJYXDYqrsjHXU9QLW85Br"
        );
    }

    #[test]
    fn monero_keys_from_bip44_seed() {
        // Full 64-byte kholaw key at m/44'/128'/0'/0/0 fed through the
        // bip_utils FromBip44PrivateKey convention: spend = reduce(Keccak(k64)).
        let mut k64 = [0u8; 64];
        k64.copy_from_slice(&hex::decode("d0c0987bdf08c215dae48c1a2a009d03910c57753bcdfed8ed6fcd5dbb27aa5c6463e38dca60da861e116a2b671233c9b4e9f6a81e25f7d5b319ad36870ab3e6").unwrap());
        let keys = MoneroKeys::from_kholaw_key(&k64);
        assert_eq!(
            hex::encode(keys.private_spend),
            "88b4fd40d51424731b888d26a37e839fa3da0b26f37ab3dcdd4fbf750109b800"
        );
        assert_eq!(
            hex::encode(keys.private_view),
            "814e8ef9c931b37e7b383a7c58a2a9279670f38d26eb8fdb55d65d2780c24207"
        );
        assert_eq!(
            keys.primary_address(),
            "46XrxrAa5aXcfkFrdSACS2EfmfxHdJkXLcGuDD2ezLNFfrfMgBJPoP5NAEz3N9WSJuhPox5S78AY3PGVeZQczueyDDT66tY"
        );
    }

    #[test]
    fn passphrase_isolated_matches_reference() {
        // keccak256("lycoris/v1/monero\0a quiet garden at dusk") → reduce → keys
        let digest = keccak256(b"lycoris/v1/monero\0a quiet garden at dusk");
        let keys = MoneroKeys::from_spend_seed(&digest);
        assert_eq!(
            keys.primary_address(),
            "48zQvBdmMxj9ByEJWD3uPAVdSKJEnJEsMS8ppnVdJXuh5GjcU9wRcuBck7NTZidFd44bFJmpumZRRi5yufFhd3dP7mHe5Eb"
        );
    }

    #[test]
    fn passphrase_strict_matches_reference() {
        let digest = keccak256(b"a quiet garden at dusk");
        let keys = MoneroKeys::from_spend_seed(&digest);
        assert_eq!(
            keys.primary_address(),
            "44pS63n9rS97fu89RXzMS51TJXWPj7wWn2yNcyEjFDZpNczmbZTR4R2FmuSZLyyRQ3H8wBdAgyWKB6jEk75SjKy83vokybe"
        );
    }

    #[test]
    fn address_validation_roundtrip() {
        let seed = hex_to_bytes("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let keys = MoneroKeys::from_spend_seed(&seed);
        let address = keys.primary_address();
        assert!(monero_address_validate(&address));
        assert!(!monero_address_validate("46NotARealMoneroAddress0000000000000000000000000000000000000"));
        // wrong checksum (flip a char in the tail)
        let mut broken = address.clone();
        let last = broken.pop().unwrap();
        broken.push(if last == 'A' { 'B' } else { 'A' });
        assert!(!monero_address_validate(&broken));
    }
}


#[cfg(test)]
mod debug_decode {}
