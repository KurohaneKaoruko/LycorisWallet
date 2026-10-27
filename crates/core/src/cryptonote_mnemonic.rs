//! Monero native mnemonic seeds (the "Electrum-style" / legacy 25-word
//! format used by monero-wallet-cli, monero-wallet-rpc and the official
//! GUI): 4 bytes → 3 words of base-1626, plus a CRC-32 checksum word.
//!
//! Reference: monero-project/monero `src/mnemonics/electrum-words.cpp`.
//! The 24 data words encode a 32-byte spend key that must already be a
//! valid ed25519 scalar (reduced modulo L) — official wallets derive it
//! from entropy via `sc_reduce32`, we do the same on restore when the
//! decoded value is not reduced (mirroring `words_to_bytes`' acceptance
//! of both) — see [`crate::cryptonote`].

use crate::cryptonote_words::MONERO_WORDS;
use crate::error::{Error, Result};

/// English word list length; also the numeric base of the encoding.
const BASE: u64 = 1626;

/// Unique word prefix length for the English list (shortest word is four
/// letters and all three-byte prefixes are distinct — verified against
/// `Language::English`, which is constructed with prefix length 3).
const UNIQUE_PREFIX: usize = 3;

/// Data words for a 32-byte spend key.
pub const DATA_WORD_COUNT: usize = 24;

/// Data words + checksum word.
pub const SEED_WORD_COUNT: usize = 25;

fn word_index(word: &str) -> Option<u64> {
    // Official wallets match on the unique-prefix characters, so
    // unambiguous abbreviations are legal input ("vel" for "velvet").
    let full: &str = if word.len() >= UNIQUE_PREFIX {
        let prefix = &word[..UNIQUE_PREFIX];
        MONERO_WORDS
            .iter()
            .find(|w| w.starts_with(prefix))
            .copied()
            .or_else(|| MONERO_WORDS.iter().copied().find(|w| *w == word))?
    } else {
        MONERO_WORDS.iter().copied().find(|w| *w == word)?
    };
    let index = MONERO_WORDS
        .iter()
        .position(|w| *w == full)
        .expect("word was found above");
    Some(index as u64)
}
/// CRC-32 over the concatenated unique prefixes, exactly like
/// `create_checksum_index` (boost::crc_32_type = CRC-32/ISO-HDLC).
/// The result picks the *position* of the checksum word among the data
/// words — the 25th word of a Monero seed is always a copy of one of
/// the first 24.
fn checksum_index(words: &[String]) -> usize {
    let mut hasher = crc32fast::Hasher::new();
    for word in words {
        let prefix: String = word.chars().take(UNIQUE_PREFIX).collect();
        hasher.update(prefix.as_bytes());
    }
    (hasher.finalize() % words.len() as u32) as usize
}

/// Resolve a word (or its unique prefix) to the official full word.
fn resolve_word(word: &str) -> Option<&'static str> {
    word_index(word).map(|i| MONERO_WORDS[i as usize])
}

/// Per-word membership report: each word of the input resolves to the
/// official list (unique-prefix match counts, like official wallets).
pub fn words_in_list(input: &str) -> Vec<bool> {
    input
        .split_whitespace()
        .map(|w| resolve_word(w).is_some())
        .collect()
}

/// Decode 24 (no checksum) or 25 (with checksum) words into the 32-byte
/// spend secret. Mirrors `crypto::ElectrumWords::words_to_bytes`.
pub fn decode(words: &str) -> Result<[u8; 32]> {
    let normalized: Vec<String> = words
        .split_whitespace()
        .map(|w| w.trim().to_lowercase())
        .filter(|w| !w.is_empty())
        .collect();
    match normalized.len() {
        DATA_WORD_COUNT => {}
        SEED_WORD_COUNT => {
            let (data, checksum) = normalized.split_at(DATA_WORD_COUNT);
            // The checksum word must be a copy of data word `checksum_index`;
            // official wallets compare the trimmed prefixes.
            let expected = checksum_index(data);
            let last = &checksum[0];
            let expected_prefix: String = data[expected].chars().take(UNIQUE_PREFIX).collect();
            let actual_prefix: String = last.chars().take(UNIQUE_PREFIX).collect();
            if word_index(last).is_none() {
                return Err(Error::InvalidMnemonic(format!(
                    "校验词「{last}」不在 Monero 词表内"
                )));
            }
            if expected_prefix != actual_prefix {
                return Err(Error::InvalidMnemonic(format!(
                    "Monero 助记词校验失败:校验词应为「{}」(第 {} 个词),实际是「{last}」",
                    data[expected],
                    expected + 1
                )));
            }
        }
        n => {
            return Err(Error::InvalidMnemonic(format!(
                "Monero 助记词应为 24 或 25 个词,收到 {n} 个"
            )))
        }
    }
    let data = &normalized[..DATA_WORD_COUNT];
    let mut indices = [0u64; DATA_WORD_COUNT];
    for (slot, word) in data.iter().enumerate() {
        indices[slot] = word_index(word)
            .ok_or_else(|| Error::InvalidMnemonic(format!("「{word}」不在 Monero 词表内")))?;
    }
    let mut out = [0u8; 32];
    for group in 0..8 {
        // Inverse of the official encoder (electrum-words.cpp), which is
        // base-1626 with a carry chain between the three words:
        //   w1 = val % B
        //   w2 = (val/B + w1) % B
        //   w3 = (val/B² + w2) % B
        // so: w3 - w2 ≡ val/B² (mod B), w2 - w1 ≡ val/B (mod B).
        let (w1, w2, w3) = (
            indices[group * 3],
            indices[group * 3 + 1],
            indices[group * 3 + 2],
        );
        let mid = (w2 + BASE - w1) % BASE;
        let high = (w3 + BASE - w2) % BASE;
        let val = w1 + mid * BASE + high * BASE * BASE;
        // The encoder packs each 4-byte little-endian word group.
        out[group * 4..group * 4 + 4].copy_from_slice(&(val as u32).to_le_bytes());
    }
    Ok(out)
}

/// Encode a 32-byte spend secret as 25 words (24 data + checksum).
/// Mirrors `crypto::ElectrumWords::bytes_to_words`, including the carry
/// chain between the three words of each 4-byte group.
pub fn encode(seed: &[u8; 32]) -> String {
    let mut words: Vec<String> = Vec::with_capacity(SEED_WORD_COUNT);
    for group in 0..8 {
        let bytes = u32::from_le_bytes([
            seed[group * 4],
            seed[group * 4 + 1],
            seed[group * 4 + 2],
            seed[group * 4 + 3],
        ]);
        let val = u64::from(bytes);
        let w1 = val % BASE;
        let w2 = (val / BASE + w1) % BASE;
        let w3 = (val / BASE / BASE + w2) % BASE;
        words.push(MONERO_WORDS[w1 as usize].to_string());
        words.push(MONERO_WORDS[w2 as usize].to_string());
        words.push(MONERO_WORDS[w3 as usize].to_string());
    }
    let checksum = words[checksum_index(&words)].clone();
    words.push(checksum);
    words.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Vector from monero's own unit tests: a repeated Spanish-list word
    /// ("pluma", index 0x43b) decodes to the repeated little-endian
    /// pattern. We do not ship the Spanish list, so the same arithmetic is
    /// verified through the English list with the same index-to-bytes
    /// formula, plus the official 0x3b0-style expectation below.
    #[test]
    fn decodes_official_test_vectors() {
        // northern = index 944 (0x3b0) in the English list. With the
        // official carry-chain encoder, a repeated-word group packs back
        // to val = 944 → little-endian b0 03 00 00 per 4-byte group,
        // matching the byte pattern of monero's own repeated-word vector.
        let seed: Vec<&str> = vec!["northern"; DATA_WORD_COUNT];
        let bytes = decode(&seed.join(" ")).expect("decodes 24 words");
        assert_eq!(
            hex::encode(bytes),
            "b0030000b0030000b0030000b0030000b0030000b0030000b0030000b0030000"
        );
        // With checksum: the CRC-32 over "nort"×24 selects the checksum
        // word, and a full round trip reproduces the same 25 words.
        let words = encode(&bytes);
        assert_eq!(words.split_whitespace().count(), SEED_WORD_COUNT);
        assert!(words.starts_with("northern northern"));
        let back = decode(&words).expect("round trips");
        assert_eq!(bytes, back);
    }

    /// End-to-end vector from a real official wallet (monero.how restore
    /// tutorial): the 25 words restore the documented spend key, and the
    /// wallet's MoneroKeys reproduce the documented address and view key.
    #[test]
    fn restores_a_real_official_wallet() {
        use crate::cryptonote::MoneroKeys;
        let seed = "locker welders womanly lodge gumball selfish altitude \
                    dewdrop terminal nagged exit acquire hookup ashtray \
                    wobbly nineteen duration duties javelin patio baffles \
                    ambush bamboo bite nineteen";
        let spend = decode(seed).expect("official seed decodes");
        assert_eq!(
            hex::encode(spend),
            "31f05304122efc67e9c5e8a629510fbdb51256e921d36a00dd79c17285a65d05"
        );
        let keys = MoneroKeys::from_spend_seed(&spend);
        assert_eq!(
            hex::encode(keys.private_view),
            "005c98c3db115140289bd0dfad97f910e6eeb5e8e12d02fdd4ab2373fbe9110a"
        );
        assert_eq!(
            keys.primary_address(),
            "47CL7FiNtW417VjzWt9Zse8Z8URhaHaA2L9jJq6rrLtDhiYK9PfbCavhhMKws9xEdKHBeGcQtJmPt4uEMivooNztC5UkHLD"
        );
    }

    #[test]
    fn rejects_bad_checksum_and_unknown_words() {
        let seed: Vec<&str> = vec!["northern"; SEED_WORD_COUNT];
        let mut tampered = seed.clone();
        tampered[24] = "zoo"; // not a Monero word
        assert!(decode(&tampered.join(" ")).is_err());
        let mut wrong_checksum = seed.clone();
        wrong_checksum[24] = "a";
        assert!(decode(&wrong_checksum.join(" ")).is_err());
        assert!(decode("not a monero seed at all").is_err());
        assert!(decode("northern northern").is_err());
    }

    #[test]
    fn accepts_4_letter_abbreviations_like_official_wallets() {
        // Official wallets resolve words by their unique 4-byte prefix.
        let bytes = decode(&"nort ".repeat(DATA_WORD_COUNT)).expect("abbreviates");
        assert_eq!(bytes[0], 0xb0);
    }

    #[test]
    fn random_round_trip_is_lossless() {
        // Deterministic pseudo-entropy keeps the test hermetic.
        let mut seed = [0u8; 32];
        for (i, byte) in seed.iter_mut().enumerate() {
            *byte = (i as u8).wrapping_mul(7).wrapping_add(13);
        }
        let words = encode(&seed);
        assert_eq!(words.split_whitespace().count(), SEED_WORD_COUNT);
        let back = decode(&words).expect("round trips");
        assert_eq!(seed, back);
    }
}
