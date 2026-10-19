use crate::address::{
    cashaddr_encode, compressed_pubkey, evm_address, p2pkh_address, p2wpkh_address,
    solana_address, uncompressed_pubkey,
};
use crate::bip32;
use crate::chains::{ChainId, ChainKind};
use crate::error::{Error, Result};
use crate::hash::{chain_hash, public_fingerprint};
use crate::slip10;
use bip39::{Language, Mnemonic};
use ed25519_dalek::SigningKey as Ed25519SigningKey;
use k256::SecretKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletMode {
    Mnemonic,
    Passphrase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DerivationMode {
    /// The chain is mixed into the hashed message, so every chain gets a
    /// distinct key even when two chains use the same hash function.
    #[default]
    Isolated,
    /// `H(phrase)` exactly. This is the most literal form of the requested
    /// brain-wallet derivation, but chains that share a hash share a key.
    Strict,
}

/// Secret wallet material. This type is never serialised to the frontend; it
/// only exists inside the encrypted vault and the unlocked Rust session.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum WalletSecret {
    Mnemonic {
        mnemonic: String,
    },
    Passphrase {
        phrase: String,
        derivation: DerivationMode,
    },
}

impl std::fmt::Debug for WalletSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalletSecret::Mnemonic { .. } => f.write_str("WalletSecret::Mnemonic([redacted])"),
            WalletSecret::Passphrase { derivation, .. } => f
                .debug_struct("WalletSecret::Passphrase")
                .field("phrase", &"[redacted]")
                .field("derivation", derivation)
                .finish(),
        }
    }
}

impl Drop for WalletSecret {
    fn drop(&mut self) {
        match self {
            WalletSecret::Mnemonic { mnemonic } => mnemonic.zeroize(),
            WalletSecret::Passphrase { phrase, .. } => phrase.zeroize(),
        }
    }
}

impl WalletSecret {
    pub fn new_mnemonic(input: &str) -> Result<Self> {
        let mnemonic = parse_mnemonic(input)?;
        Ok(Self::Mnemonic {
            mnemonic: mnemonic.to_string(),
        })
    }

    pub fn new_passphrase(input: &str, derivation: DerivationMode) -> Result<Self> {
        let canonical = crate::hash::canonical_phrase(input)?;
        Ok(Self::Passphrase {
            phrase: canonical,
            derivation,
        })
    }

    pub fn mode(&self) -> WalletMode {
        match self {
            Self::Mnemonic { .. } => WalletMode::Mnemonic,
            Self::Passphrase { .. } => WalletMode::Passphrase,
        }
    }

    pub fn derivation_mode(&self) -> Option<DerivationMode> {
        match self {
            Self::Mnemonic { .. } => None,
            Self::Passphrase { derivation, .. } => Some(*derivation),
        }
    }

    pub fn mnemonic(&self) -> Result<Mnemonic> {
        match self {
            Self::Mnemonic { mnemonic } => parse_mnemonic(mnemonic),
            Self::Passphrase { .. } => Err(Error::InvalidMnemonic(
                "wallet is not a mnemonic wallet".into(),
            )),
        }
    }

    pub fn passphrase(&self) -> Option<&str> {
        match self {
            Self::Passphrase { phrase, .. } => Some(phrase),
            Self::Mnemonic { .. } => None,
        }
    }

    pub fn word_count(&self) -> Option<usize> {
        match self {
            Self::Mnemonic { mnemonic } => Some(mnemonic.split_whitespace().count()),
            Self::Passphrase { .. } => None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Mnemonic { mnemonic } => {
                parse_mnemonic(mnemonic)?;
                Ok(())
            }
            Self::Passphrase { phrase, .. } => {
                crate::hash::canonical_phrase(phrase)?;
                Ok(())
            }
        }
    }
}

fn parse_mnemonic(input: &str) -> Result<Mnemonic> {
    let normalized = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if normalized.is_empty() {
        return Err(Error::InvalidMnemonic("mnemonic is empty".into()));
    }
    Mnemonic::parse_normalized(&normalized).map_err(|e| Error::InvalidMnemonic(e.to_string()))
}

pub fn generate_mnemonic(word_count: usize) -> Result<String> {
    if word_count != 12 && word_count != 24 {
        return Err(Error::InvalidMnemonic("word count must be 12 or 24".into()));
    }
    Mnemonic::generate_in(Language::English, word_count)
        .map(|m| m.to_string())
        .map_err(|e| Error::InvalidMnemonic(e.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub chain: ChainId,
    pub name: String,
    pub symbol: String,
    pub address: String,
    pub path: String,
    pub derivation: String,
    pub hash_algorithm: String,
    pub hash_detail: String,
    pub address_format: String,
    pub public_key: String,
    pub accent: String,
    pub explorer_url: String,
    pub send_supported: bool,
    pub decimals: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivationPreview {
    pub mode: WalletMode,
    pub derivation: Option<DerivationMode>,
    pub accounts: Vec<Account>,
    pub fingerprint: String,
    pub formula: String,
}

pub struct PrivateKeyMaterial {
    pub chain: ChainId,
    inner: PrivateKeyInner,
}

enum PrivateKeyInner {
    Secp(SecretKey),
    Ed25519([u8; 32]),
}

impl std::fmt::Debug for PrivateKeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateKeyMaterial")
            .field("chain", &self.chain)
            .field("key", &"[redacted]")
            .finish()
    }
}

impl Drop for PrivateKeyMaterial {
    fn drop(&mut self) {
        if let PrivateKeyInner::Ed25519(bytes) = &mut self.inner {
            bytes.zeroize();
        }
    }
}

impl PrivateKeyMaterial {
    pub fn secp(&self) -> Result<&SecretKey> {
        match &self.inner {
            PrivateKeyInner::Secp(key) => Ok(key),
            _ => Err(Error::Crypto("key is not secp256k1".into())),
        }
    }

    pub fn ed25519_seed(&self) -> Result<[u8; 32]> {
        match &self.inner {
            PrivateKeyInner::Ed25519(seed) => Ok(*seed),
            _ => Err(Error::Crypto("key is not ed25519".into())),
        }
    }

    /// Export format. UTXO chains return WIF, EVM chains return a 0x-prefixed
    /// hex key, and Solana returns a base58 64-byte keypair (the format most
    /// Solana wallets import).
    pub fn export(&self) -> Result<String> {
        match &self.inner {
            PrivateKeyInner::Secp(key) => match self.chain {
                ChainId::Bitcoin | ChainId::Litecoin => crate::address::wif(key, self.chain),
                _ => Ok(format!("0x{}", hex::encode(key.to_bytes()))),
            },
            PrivateKeyInner::Ed25519(seed) => {
                let signing = Ed25519SigningKey::from_bytes(seed);
                let mut pair = [0u8; 64];
                pair[..32].copy_from_slice(seed);
                pair[32..].copy_from_slice(signing.verifying_key().as_bytes());
                let encoded = bs58::encode(pair).into_string();
                pair.zeroize();
                Ok(encoded)
            }
        }
    }
}

fn secp_from_digest(digest: [u8; 32]) -> Result<SecretKey> {
    if let Ok(key) = SecretKey::from_slice(&digest) {
        return Ok(key);
    }
    // The probability of this branch is ~2^-128, but it is specified so the
    // derivation is total and deterministic for any 32-byte digest.
    let mut candidate = digest;
    for counter in 0u8..=255 {
        let mut hasher = Sha256::new();
        hasher.update(b"lycoris/key-fallback/v1");
        hasher.update(candidate);
        hasher.update([counter]);
        candidate.copy_from_slice(&hasher.finalize());
        if let Ok(key) = SecretKey::from_slice(&candidate) {
            return Ok(key);
        }
    }
    Err(Error::Crypto(
        "unable to derive a valid secp256k1 key".into(),
    ))
}

fn account_from_secp(
    chain: ChainId,
    key: &SecretKey,
    path: &str,
    derivation: &str,
    hash_algorithm: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    let address = match chain {
        ChainId::Bitcoin | ChainId::Litecoin => {
            let params = chain.utxo_params().expect("utxo params");
            p2wpkh_address(&compressed_pubkey(key), params.hrp)?
        }
        ChainId::Dogecoin => {
            let params = chain.utxo_params().expect("utxo params");
            p2pkh_address(&compressed_pubkey(key), params.p2pkh_version)
        }
        ChainId::BitcoinCash => {
            let params = chain.utxo_params().expect("utxo params");
            cashaddr_encode(&crate::address::hash160(&compressed_pubkey(key)), params.hrp)?
        }
        ChainId::Ethereum
        | ChainId::EthereumClassic
        | ChainId::Bsc
        | ChainId::Polygon
        | ChainId::Arbitrum
        | ChainId::Optimism
        | ChainId::Base
        | ChainId::Avalanche => evm_address(&uncompressed_pubkey(key))?,
        ChainId::Solana => return Err(Error::UnsupportedChain(chain.to_string())),
    };
    Ok(Account {
        chain,
        name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        address,
        path: path.to_string(),
        derivation: derivation.to_string(),
        hash_algorithm: hash_algorithm.to_string(),
        hash_detail: hash_detail.to_string(),
        address_format: info.kind.address_format().to_string(),
        public_key: hex::encode(compressed_pubkey(key)),
        accent: info.accent.to_string(),
        explorer_url: info.explorer_address.to_string(),
        send_supported: info.send_supported(),
        decimals: info.decimals,
    })
}

fn account_from_ed25519(
    chain: ChainId,
    seed: &[u8; 32],
    path: &str,
    derivation: &str,
    hash_algorithm: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    let signing = Ed25519SigningKey::from_bytes(seed);
    let pubkey = signing.verifying_key().to_bytes();
    let address = solana_address(&pubkey);
    Ok(Account {
        chain,
        name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        address,
        path: path.to_string(),
        derivation: derivation.to_string(),
        hash_algorithm: hash_algorithm.to_string(),
        hash_detail: hash_detail.to_string(),
        address_format: info.kind.address_format().to_string(),
        public_key: hex::encode(pubkey),
        accent: info.accent.to_string(),
        explorer_url: info.explorer_address.to_string(),
        send_supported: info.send_supported(),
        decimals: info.decimals,
    })
}

pub fn derive_accounts(secret: &WalletSecret) -> Result<Vec<Account>> {
    secret.validate()?;
    match secret {
        WalletSecret::Mnemonic { .. } => {
            let mnemonic = secret.mnemonic()?;
            let seed = mnemonic.to_seed("");
            let mut accounts = Vec::with_capacity(ChainId::ALL.len());
            for chain in ChainId::ALL {
                let info = chain.info();
                match info.kind {
                    ChainKind::Utxo | ChainKind::Evm => {
                        let derived = bip32::derive_path(&seed, info.derivation_path)?;
                        let algorithm = match info.kind {
                            ChainKind::Utxo => "BIP-32 / BIP-84",
                            ChainKind::Evm => "BIP-32 / BIP-44",
                            ChainKind::Ed25519 => unreachable!(),
                        };
                        let detail = if chain == ChainId::Bsc || chain == ChainId::Polygon {
                            "BIP-44 coin 60 · shared EVM account"
                        } else {
                            "BIP-39 seed · standard path"
                        };
                        accounts.push(account_from_secp(
                            chain,
                            &derived.key,
                            info.derivation_path,
                            algorithm,
                            algorithm,
                            detail,
                        )?);
                    }
                    ChainKind::Ed25519 => {
                        let derived = slip10::derive_path(&seed, info.derivation_path)?;
                        accounts.push(account_from_ed25519(
                            chain,
                            &derived.key,
                            info.derivation_path,
                            "BIP-39 / SLIP-0010",
                            "SLIP-0010 ed25519",
                            "Hardened ed25519 derivation",
                        )?);
                    }
                }
            }
            Ok(accounts)
        }
        WalletSecret::Passphrase { phrase, derivation } => {
            let isolated = *derivation == DerivationMode::Isolated;
            let mut accounts = Vec::with_capacity(ChainId::ALL.len());
            for chain in ChainId::ALL {
                let info = chain.info();
                let digest = chain_hash(phrase, chain, isolated)?;
                let path = if isolated {
                    "direct · isolated"
                } else {
                    "direct · strict"
                };
                match info.kind {
                    ChainKind::Utxo | ChainKind::Evm => {
                        let key = secp_from_digest(digest)?;
                        accounts.push(account_from_secp(
                            chain,
                            &key,
                            path,
                            info.hash.label(),
                            info.hash.detail(),
                            info.hash.label(),
                        )?);
                    }
                    ChainKind::Ed25519 => {
                        accounts.push(account_from_ed25519(
                            chain,
                            &digest,
                            path,
                            info.hash.label(),
                            info.hash.detail(),
                            info.hash.label(),
                        )?);
                    }
                }
            }
            Ok(accounts)
        }
    }
}

pub fn derive_private_key(secret: &WalletSecret, chain: ChainId) -> Result<PrivateKeyMaterial> {
    secret.validate()?;
    match secret {
        WalletSecret::Mnemonic { .. } => {
            let mnemonic = secret.mnemonic()?;
            let seed = mnemonic.to_seed("");
            let info = chain.info();
            match info.kind {
                ChainKind::Utxo | ChainKind::Evm => {
                    let derived = bip32::derive_path(&seed, info.derivation_path)?;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Secp(derived.key),
                    })
                }
                ChainKind::Ed25519 => {
                    let derived = slip10::derive_path(&seed, info.derivation_path)?;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Ed25519(derived.key),
                    })
                }
            }
        }
        WalletSecret::Passphrase { phrase, derivation } => {
            let digest = chain_hash(phrase, chain, *derivation == DerivationMode::Isolated)?;
            match chain.info().kind {
                ChainKind::Utxo | ChainKind::Evm => Ok(PrivateKeyMaterial {
                    chain,
                    inner: PrivateKeyInner::Secp(secp_from_digest(digest)?),
                }),
                ChainKind::Ed25519 => Ok(PrivateKeyMaterial {
                    chain,
                    inner: PrivateKeyInner::Ed25519(digest),
                }),
            }
        }
    }
}

pub fn derive_fingerprint(accounts: &[Account]) -> String {
    public_fingerprint(
        &accounts
            .iter()
            .map(|account| account.address.clone())
            .collect::<Vec<_>>(),
    )
}

pub fn preview_derivation(secret: &WalletSecret) -> Result<DerivationPreview> {
    let accounts = derive_accounts(secret)?;
    let fingerprint = derive_fingerprint(&accounts);
    let mode = secret.mode();
    let derivation = secret.derivation_mode();
    let formula = match secret {
        WalletSecret::Mnemonic { .. } => {
            "BIP-39 mnemonic → 512-bit seed → BIP-32 / SLIP-0010 paths".to_string()
        }
        WalletSecret::Passphrase { derivation, .. } => {
            if *derivation == DerivationMode::Isolated {
                "H_chain(\"lycoris/v1/<chain>\\0\" + phrase) → 32 bytes → private key".to_string()
            } else {
                "H_chain(phrase) → 32 bytes → private key".to_string()
            }
        }
    };
    Ok(DerivationPreview {
        mode,
        derivation,
        accounts,
        fingerprint,
        formula,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip39_wallet_has_known_btc_and_eth_addresses() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let btc = accounts
            .iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        let eth = accounts
            .iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        assert_eq!(btc.address, "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu");
        assert_eq!(
            eth.address.to_lowercase(),
            "0x9858effd232b4033e47d90003d41ec34ecaeda94"
        );
    }

    #[test]
    fn passphrase_is_deterministic_and_isolated() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let a = derive_accounts(&secret).unwrap();
        let b = derive_accounts(&secret).unwrap();
        assert_eq!(a[0].address, b[0].address);
        let btc = &a
            .iter()
            .find(|x| x.chain == ChainId::Bitcoin)
            .unwrap()
            .address;
        let ltc = &a
            .iter()
            .find(|x| x.chain == ChainId::Litecoin)
            .unwrap()
            .address;
        assert_ne!(btc, ltc);
    }

    #[test]
    fn bip39_wallet_has_known_ltc_and_sol_addresses() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let ltc = accounts
            .iter()
            .find(|a| a.chain == ChainId::Litecoin)
            .unwrap();
        let sol = accounts
            .iter()
            .find(|a| a.chain == ChainId::Solana)
            .unwrap();
        assert_eq!(ltc.address, "ltc1qjmxnz78nmc8nq77wuxh25n2es7rzm5c2rkk4wh");
        assert_eq!(sol.address, "HAgk14JpMQLgt6rVgv7cBQFJWFto5Dqxi472uT3DKpqk");
    }

    #[test]
    fn passphrase_isolated_vectors() {
        // Cross-checked against an independent Python implementation
        // (bip_utils) of the same formula.
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let address = |chain: ChainId| {
            accounts
                .iter()
                .find(|a| a.chain == chain)
                .unwrap()
                .address
                .clone()
        };
        assert_eq!(
            address(ChainId::Bitcoin),
            "bc1qx0q0wdh4z5a64twecyg0kqyqkpc3h66nv8n2gt"
        );
        assert_eq!(
            address(ChainId::Litecoin),
            "ltc1qg06z6x74fe6f2qyqkfqk7kle8llgty3s78tfql"
        );
        assert_eq!(
            address(ChainId::Ethereum),
            "0x9231e3Fa53ABF00CeC08DB45a7dEDe19f939a0CB"
        );
        assert_eq!(
            address(ChainId::Solana),
            "5HeimWLA7nee4kKKL8mWbPjMHPXKYTY6gM43LgMk9Q4a"
        );
    }

    #[test]
    fn strict_mode_shares_evm_key() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Strict).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let eth = accounts
            .iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        let bsc = accounts.iter().find(|a| a.chain == ChainId::Bsc).unwrap();
        assert_eq!(eth.address, bsc.address);
    }

    #[test]
    fn bip39_extended_chain_vectors() {
        // Independent vectors generated with bip_utils (BIP-44 account 0,
        // external index 0) for the mnemonic used across the test suite.
        let mnemonic = "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let address = |chain: ChainId| {
            accounts
                .iter()
                .find(|a| a.chain == chain)
                .unwrap()
                .address
                .clone()
        };

        // EVM L2s share the m/44'/60' key, so they reuse the ETH address.
        let eth = address(ChainId::Ethereum);
        assert_eq!(eth, "0x58A57ed9d8d624cBD12e2C467D34787555bB1b25");
        for chain in [
            ChainId::Bsc,
            ChainId::Polygon,
            ChainId::Arbitrum,
            ChainId::Optimism,
            ChainId::Base,
            ChainId::Avalanche,
        ] {
            assert_eq!(address(chain), eth, "{chain:?} must share the ETH address");
        }
        // Ethereum Classic uses its own coin type 61.
        assert_eq!(
            address(ChainId::EthereumClassic),
            "0x80857C0c11b30003Cf74F99Ed4D68DA41C7224aa"
        );
        // Legacy P2PKH (base58) and cashaddr formats.
        assert_eq!(
            address(ChainId::Dogecoin),
            "DMM78DyiwEPzWmykiQQUFcPcUv13YPe2ZK"
        );
        assert_eq!(
            address(ChainId::BitcoinCash),
            "bitcoincash:qpdtccrxx78kcuc65mceurfwg60gmmqu9cwpjdt25n"
        );
    }

    #[test]
    fn passphrase_extended_chains_are_isolated() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let find = |chain: ChainId| accounts.iter().find(|a| a.chain == chain).unwrap();
        // Keccak chains in isolated mode mix the chain name into the message,
        // so every EVM chain gets its own key.
        let eth = find(ChainId::Ethereum).address.clone();
        for chain in [
            ChainId::Bsc,
            ChainId::Polygon,
            ChainId::Arbitrum,
            ChainId::Optimism,
            ChainId::Base,
            ChainId::Avalanche,
            ChainId::EthereumClassic,
        ] {
            assert_ne!(
                find(chain).address,
                eth,
                "{chain:?} must differ from Ethereum in isolated mode"
            );
        }
        // Dogecoin shares the scrypt parameters with Litecoin but is still a
        // different chain name, hence a different key.
        assert_ne!(
            find(ChainId::Dogecoin).address,
            find(ChainId::Litecoin).address
        );
        assert!(!find(ChainId::Dogecoin).send_supported);
        assert!(!find(ChainId::BitcoinCash).send_supported);
    }
}
