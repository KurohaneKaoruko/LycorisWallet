use crate::chains::{ChainId, HashAlgorithm};
use crate::error::{Error, Result};
use scrypt::{scrypt, Params as ScryptParams};
use sha2::{Digest, Sha256};
use sha3::Keccak256;
use unicode_normalization::UnicodeNormalization;

/// The constant salt used by scrypt. The chain domain, when enabled, is part
/// of the message rather than the salt so that the formula remains easy to
/// explain and reproduce.
const SCRYPT_SALT: &[u8] = b"lycoris-wallet/v1";

/// Canonicalise a memorised phrase in a way that is stable on every platform:
/// outer whitespace is removed, internal whitespace/case are preserved, and
/// the text is normalised to Unicode NFKD.
pub fn canonical_phrase(input: &str) -> Result<String> {
    let normalised: String = input.trim().nfkd().collect();
    if normalised.is_empty() {
        return Err(Error::InvalidPassphrase("phrase cannot be empty".into()));
    }
    if normalised.len() > 4096 {
        return Err(Error::InvalidPassphrase("phrase is too long".into()));
    }
    Ok(normalised)
}

/// The exact byte string that is fed to the chain-native hash.
pub fn derivation_message(phrase: &str, chain: ChainId, isolated: bool) -> Result<Vec<u8>> {
    let phrase = canonical_phrase(phrase)?;
    if isolated {
        Ok(format!("lycoris/v1/{}\0{}", chain.as_str(), phrase).into_bytes())
    } else {
        Ok(phrase.into_bytes())
    }
}

/// Derive 32 bytes of key material using the chain's own native hash.
///
/// - SHA-256 chains use SHA-256.
/// - Keccak-256 chains (EVM) use Keccak-256.
/// - Litecoin uses its proof-of-work hash, scrypt, with fixed interactive
///   parameters. The parameters are deliberately modest enough for phones.
///
/// This is a *direct* passphrase derivation. It is intentionally simple and
/// reproducible, and consequently far weaker than BIP-39 for low-entropy
/// phrases. The UI and docs repeat this warning.
pub fn chain_hash(phrase: &str, chain: ChainId, isolated: bool) -> Result<[u8; 32]> {
    let message = derivation_message(phrase, chain, isolated)?;
    hash_message(&message, chain)
}

/// The same chain-native hash, but the account index is mixed into the hashed
/// message (`…\0#<index>`). Every account gets an independent key while the
/// formula stays a one-line salted hash.
pub fn chain_hash_salted(
    phrase: &str,
    chain: ChainId,
    isolated: bool,
    index: u32,
) -> Result<[u8; 32]> {
    let mut message = derivation_message(phrase, chain, isolated)?;
    message.extend_from_slice(format!("\0#{}", index).as_bytes());
    hash_message(&message, chain)
}

/// [`chain_hash_salted`] 的字节版:输入是原始密钥字节而不是可读短语
/// (私钥导入钱包的加盐派生)。消息 = 域分隔 + 密钥 + 序号。
pub fn chain_hash_salted_bytes(key: &[u8; 32], chain: ChainId, index: u32) -> Result<[u8; 32]> {
    let mut message = format!("lycoris/v1/{}\0", chain.as_str()).into_bytes();
    message.extend_from_slice(key);
    message.extend_from_slice(format!("\0#{}", index).as_bytes());
    hash_message(&message, chain)
}

/// Iterated hashing (the "hash chain" strategy): the chain hash is applied to
/// the *base account's 32-byte key seed* `rounds` times. The base key is
/// whatever the base account already has — a mnemonic-path key or a passphrase
/// hash — so the chain never folds back onto it: round 1 already differs.
pub fn chain_key_iterated(base: [u8; 32], chain: ChainId, rounds: u32) -> Result<[u8; 32]> {
    let mut digest = base;
    for _ in 0..rounds {
        digest = hash_message(&digest, chain)?;
    }
    Ok(digest)
}

fn hash_message(message: &[u8], chain: ChainId) -> Result<[u8; 32]> {
    let algorithm = chain.info().hash;
    match algorithm {
        HashAlgorithm::Sha256 => {
            let digest = Sha256::digest(message);
            let mut out = [0u8; 32];
            out.copy_from_slice(&digest);
            Ok(out)
        }
        HashAlgorithm::Keccak256 => {
            let digest = Keccak256::digest(message);
            let mut out = [0u8; 32];
            out.copy_from_slice(&digest);
            Ok(out)
        }
        HashAlgorithm::Scrypt => {
            let params = ScryptParams::new(14, 8, 1, 32)
                .map_err(|e| Error::Crypto(format!("invalid scrypt parameters: {e}")))?;
            let mut out = [0u8; 32];
            scrypt(message, SCRYPT_SALT, &params, &mut out)
                .map_err(|e| Error::Crypto(format!("scrypt failed: {e}")))?;
            Ok(out)
        }
        // Dash (X11), Groestlcoin, Vertcoin/Monacoin/Fujicoin (Lyra2RE) and
        // Ravencoin (KawPow) proof-of-work hashes are not reproducible in
        // this build without pulling in their full hash cascades. Passphrase
        // wallets on these chains deliberately fall back to a
        // domain-separated SHA-256 so the derivation stays stable and
        // auditable; mnemonic wallets are unaffected (they use BIP-32).
        HashAlgorithm::X11
        | HashAlgorithm::Groestl
        | HashAlgorithm::Lyra2re
        | HashAlgorithm::KawPow => {
            let mut hasher = Sha256::new();
            hasher.update(format!("lycoris/pow-fallback/v1/{}\0", chain.as_str()));
            hasher.update(message);
            let digest = hasher.finalize();
            let mut out = [0u8; 32];
            out.copy_from_slice(&digest);
            Ok(out)
        }
    }
}

/// A short, public verification value for a phrase/derivation combination.
/// It is derived from the resulting addresses, never from private key bytes.
pub fn public_fingerprint(addresses: &[String]) -> String {
    let mut hasher = Sha256::new();
    for address in addresses {
        hasher.update(address.as_bytes());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    // 全大写十六进制,与钱包界面其他指纹展示一致,也更易读.
    let hex = hex::encode(&digest[..8]).to_uppercase();
    format!(
        "{}-{}-{}-{}",
        &hex[0..4],
        &hex[4..8],
        &hex[8..12],
        &hex[12..16]
    )
}

/// Human readable formula shown in the app.
pub fn formula(chain: ChainId) -> String {
    format!("{} → 32 bytes → private key", chain.info().hash.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrase_is_normalised() {
        assert_eq!(canonical_phrase("  café  ").unwrap(), "cafe\u{301}");
        assert!(canonical_phrase("   ").is_err());
    }

    #[test]
    fn chain_hashes_are_stable_and_distinct() {
        let a = chain_hash("a quiet garden at dusk", ChainId::Bitcoin, true).unwrap();
        let b = chain_hash("a quiet garden at dusk", ChainId::Bitcoin, true).unwrap();
        assert_eq!(a, b);

        let eth = chain_hash("a quiet garden at dusk", ChainId::Ethereum, true).unwrap();
        let btc = chain_hash("a quiet garden at dusk", ChainId::Bitcoin, true).unwrap();
        assert_ne!(eth, btc);

        let btc_isolated = chain_hash("a quiet garden at dusk", ChainId::Bitcoin, true).unwrap();
        let btc_strict = chain_hash("a quiet garden at dusk", ChainId::Bitcoin, false).unwrap();
        assert_ne!(btc_isolated, btc_strict);
    }

    #[test]
    fn known_sha256_vector() {
        // SHA-256("abc")
        let digest = chain_hash("abc", ChainId::Bitcoin, false).unwrap();
        assert_eq!(
            hex::encode(digest),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn known_keccak_vector() {
        // Keccak-256("") = c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470
        let digest = chain_hash("", ChainId::Ethereum, false);
        assert!(digest.is_err()); // empty phrase is rejected by the canonicaliser
    }
}
