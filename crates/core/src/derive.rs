use crate::address::{
    cashaddr_encode, compressed_pubkey, evm_address, p2pkh_address, p2wpkh_address, solana_address,
    uncompressed_pubkey,
};
use crate::bip32;
use crate::chains::{ChainId, ChainKind};
use crate::error::{Error, Result};
use crate::hash::{chain_hash, public_fingerprint};
use crate::slip10;
#[cfg(feature = "vault")]
use bip39::Language;
use bip39::Mnemonic;
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

/// Generate a fresh BIP-39 mnemonic. Not available in the WebAssembly build,
/// which has no entropy source and only re-derives existing phrases.
#[cfg(feature = "vault")]
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

/// How multiple accounts are spread out from one secret.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum BatchStrategy {
    /// Mnemonic wallets: bump the last BIP-32 address index
    /// (`m/84'/0'/0'/0/1`, `/2`, …). Passphrase wallets: the chain domain is
    /// mixed in together with the account index.
    #[default]
    Path,
    /// `H(domain‖phrase‖#index)` — an explicit salted hash. For mnemonic
    /// wallets this leaves the standard BIP-32 tree and hashes the phrase.
    Salted,
    /// `H^index(base)` — the chain hash applied `index` times to the *base
    /// account's key seed*, whichever way the base key was produced. Index
    /// starts at 1 because 0 would repeat the base account.
    Iterated,
}

impl BatchStrategy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Path => "路径派生",
            Self::Salted => "加盐派生",
            Self::Iterated => "迭代哈希",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::Path => "BIP-32 地址索引递增 · 标准路径",
            Self::Salted => "H(域名‖口令‖#序号) · 每个序号独立密钥",
            Self::Iterated => "H(…) 连续哈希 N 次 · 基于基础私钥",
        }
    }

    /// Upper bound for user-supplied indices. Keeps a mistyped count from
    /// freezing the UI (iterated scrypt at 14 rounds is ~50 ms per account,
    /// and BIP-39 to_seed is ~30 ms).
    pub fn max_count(self) -> u32 {
        match self {
            Self::Path | Self::Salted => 100,
            Self::Iterated => 50,
        }
    }

    /// Which strategies the UI should offer for a wallet mode. Mnemonic
    /// wallets walk the BIP-32 tree; passphrase wallets have no mnemonic and
    /// therefore no path to walk (`Path` there is a salted-hash alias).
    pub fn available_for(mode: WalletMode) -> &'static [BatchStrategy] {
        match mode {
            WalletMode::Mnemonic => &[Self::Path, Self::Salted, Self::Iterated],
            WalletMode::Passphrase => &[Self::Salted, Self::Iterated],
        }
    }

    /// The strategies a stored vault may still reference from older versions.
    /// `Path` entries in a passphrase wallet keep deriving as salted hashes.
    #[allow(dead_code)]
    pub fn is_valid_for(self, mode: WalletMode) -> bool {
        match mode {
            WalletMode::Mnemonic => true,
            // Tolerated for legacy specs: `Path` derives as a salted hash.
            WalletMode::Passphrase => true,
        }
    }
}

/// Request for [`derive_accounts_batch`]: which accounts to derive and how.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    #[serde(default)]
    pub strategy: BatchStrategy,
    /// The chain to derive on. One request adds addresses on one chain only.
    pub chain: ChainId,
    /// Number of accounts, 1..=strategy.max_count().
    pub count: u32,
    /// First account index (0-based). Defaults to 0.
    #[serde(default)]
    pub start: u32,
}

/// A derived account pinned to a wallet. Only the *recipe* is stored —
/// strategy + index — never key material; the addresses are recomputed
/// deterministically from the wallet secret on demand. The fingerprint is a
/// public value kept for the wallet list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAccountSpec {
    /// Stable identifier (`"{strategy}:{chain}:{index}"`) used for switching.
    pub id: String,
    pub strategy: BatchStrategy,
    /// The single chain this account belongs to. A derived account adds one
    /// address on one chain; other chains keep using the base derivation.
    pub chain: ChainId,
    pub index: u32,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub fingerprint: String,
}

impl BatchAccountSpec {
    pub fn new(strategy: BatchStrategy, chain: ChainId, index: u32) -> Self {
        Self {
            id: format!("{}:{}:{}", strategy_id(strategy), chain.as_str(), index),
            strategy,
            chain,
            index,
            label: None,
            fingerprint: String::new(),
        }
    }

    pub fn display_label(&self) -> String {
        self.label
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("#{}", self.index))
    }
}

/// The snake_case spelling used in [`BatchAccountSpec::id`].
pub fn strategy_id(strategy: BatchStrategy) -> &'static str {
    match strategy {
        BatchStrategy::Path => "path",
        BatchStrategy::Salted => "salted",
        BatchStrategy::Iterated => "iterated",
    }
}

/// Human-readable formula for one derived account, shown in the UI next to
/// the derivation path.
pub fn spec_formula(spec: &BatchAccountSpec, mode: WalletMode) -> String {
    match (spec.strategy, mode) {
        (BatchStrategy::Path, WalletMode::Mnemonic) => {
            format!("BIP-32 地址索引 {} · 路径派生", spec.index)
        }
        (BatchStrategy::Path, _) => format!("H_chain(密语, #{}) · 加盐派生", spec.index),
        (BatchStrategy::Salted, _) => format!("H_chain(密语, #{}) · 加盐派生", spec.index),
        (BatchStrategy::Iterated, _) => format!("H_chain^{}(基础私钥) · 迭代哈希", spec.index),
    }
}

pub struct PrivateKeyMaterial {
    pub chain: ChainId,
    inner: PrivateKeyInner,
}

enum PrivateKeyInner {
    Secp(SecretKey),
    Ed25519([u8; 32]),
    Monero(crate::cryptonote::MoneroKeys),
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
        // MoneroKeys zeroises itself in its own Drop impl.
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

    /// The Monero spend/view keypair. `monero-wallet-rpc` needs both halves
    /// (and the address) to import a wallet that belongs to this phrase.
    pub fn monero_keys(&self) -> Result<&crate::cryptonote::MoneroKeys> {
        match &self.inner {
            PrivateKeyInner::Monero(keys) => Ok(keys),
            _ => Err(Error::Crypto("key is not a Monero keypair".into())),
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
            PrivateKeyInner::Monero(keys) => {
                // Monero wallets conventionally export `spend‖view` hex.
                let mut raw = [0u8; 64];
                raw[..32].copy_from_slice(&keys.private_spend);
                raw[32..].copy_from_slice(&keys.private_view);
                let encoded = hex::encode(raw);
                raw.zeroize();
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
            cashaddr_encode(
                &crate::address::hash160(&compressed_pubkey(key)),
                params.hrp,
            )?
        }
        ChainId::Ethereum | ChainId::EthereumClassic | ChainId::Bsc | ChainId::Polygon => {
            evm_address(&uncompressed_pubkey(key))?
        }
        ChainId::Solana | ChainId::Monero => {
            return Err(Error::UnsupportedChain(chain.to_string()))
        }
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

fn account_from_monero(
    chain: ChainId,
    keys: &crate::cryptonote::MoneroKeys,
    path: &str,
    derivation: &str,
    hash_algorithm: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    Ok(Account {
        chain,
        name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        address: keys.primary_address(),
        path: path.to_string(),
        derivation: derivation.to_string(),
        hash_algorithm: hash_algorithm.to_string(),
        hash_detail: hash_detail.to_string(),
        address_format: info.kind.address_format().to_string(),
        public_key: hex::encode(keys.public_spend),
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
                            ChainKind::Ed25519 | ChainKind::Cryptonote => unreachable!(),
                        };
                        let detail = if chain == ChainId::Bsc || chain == ChainId::Polygon {
                            "BIP-44 coin 60 · 与 Ethereum 共享账户"
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
                    ChainKind::Cryptonote => {
                        // SLIP-10 ed25519-kholaw (Monero's unclamped variant),
                        // then the standard spend/view expansion:
                        // spend = reduce(Keccak(kL ‖ kR)), view = reduce(Keccak(spend)).
                        let mut k64 = [0u8; 64];
                        k64.copy_from_slice(&crate::slip10::derive_kholaw_path(
                            &seed,
                            info.derivation_path,
                        )?);
                        let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                        k64.zeroize();
                        accounts.push(account_from_monero(
                            chain,
                            &keys,
                            info.derivation_path,
                            "BIP-39 / SLIP-0010 kholaw",
                            "Keccak-256 · CryptoNote",
                            "spend = reduce(kholaw) · view = reduce(Keccak(spend))",
                        )?);
                    }
                }
            }
            Ok(accounts)
        }
        WalletSecret::Passphrase { phrase, derivation } => {
            let isolated = *derivation == DerivationMode::Isolated;
            let derivation_label = if isolated {
                "H_chain(phrase)"
            } else {
                "H(phrase)"
            };
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
                            derivation_label,
                            info.hash.label(),
                            info.hash.detail(),
                        )?);
                    }
                    ChainKind::Ed25519 => {
                        accounts.push(account_from_ed25519(
                            chain,
                            &digest,
                            path,
                            derivation_label,
                            info.hash.label(),
                            info.hash.detail(),
                        )?);
                    }
                    ChainKind::Cryptonote => {
                        let keys = crate::cryptonote::MoneroKeys::from_spend_seed(&digest);
                        accounts.push(account_from_monero(
                            chain,
                            &keys,
                            path,
                            derivation_label,
                            info.hash.label(),
                            // Monero shares Keccak-256 with the EVM chains, but
                            // not their description.
                            "CryptoNote Keccak-256 · 32 byte digest",
                        )?);
                    }
                }
            }
            Ok(accounts)
        }
    }
}

/// Derive one account of `chain` at 0-based `index` for a mnemonic wallet,
/// bumping the last (address) component of the standard path.
fn mnemonic_account_at_index(seed: &[u8; 64], chain: ChainId, index: u32) -> Result<Account> {
    let info = chain.info();
    let base = info.derivation_path;
    match info.kind {
        ChainKind::Utxo | ChainKind::Evm => {
            let path = replace_last_index(base, index);
            let derived = bip32::derive_path(seed, &path)?;
            let algorithm = match info.kind {
                ChainKind::Utxo => "BIP-32 / BIP-84",
                ChainKind::Evm => "BIP-32 / BIP-44",
                ChainKind::Ed25519 | ChainKind::Cryptonote => unreachable!(),
            };
            account_from_secp(
                chain,
                &derived.key,
                &path,
                algorithm,
                algorithm,
                "地址索引递增",
            )
        }
        ChainKind::Ed25519 => {
            let path = replace_last_index(base, index);
            let derived = slip10::derive_path(seed, &path)?;
            account_from_ed25519(
                chain,
                &derived.key,
                &path,
                "BIP-39 / SLIP-0010",
                "SLIP-0010 ed25519",
                "Hardened ed25519 derivation",
            )
        }
        ChainKind::Cryptonote => {
            // Monero has no usable multi-account convention here: its standard
            // path ends in non-hardened steps that kholaw rejects, and the
            // ecosystem treats one (spend, view) pair as *the* wallet. Account
            // 0 is the normal wallet; further indices derive subaddresses via
            // the account index where support exists. Keep repeating account 0
            // so the batch stays aligned; the UI explains this.
            let _ = index;
            let mut k64 = [0u8; 64];
            k64.copy_from_slice(&crate::slip10::derive_kholaw_path(seed, base)?);
            let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
            k64.zeroize();
            account_from_monero(
                chain,
                &keys,
                base,
                "BIP-39 / SLIP-0010 kholaw",
                "Keccak-256 · CryptoNote",
                "spend = reduce(kholaw) · view = reduce(Keccak(spend))",
            )
        }
    }
}

fn account_from_digest(
    chain: ChainId,
    digest: [u8; 32],
    path: &str,
    derivation_label: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    match info.kind {
        ChainKind::Utxo | ChainKind::Evm => {
            let key = secp_from_digest(digest)?;
            account_from_secp(
                chain,
                &key,
                path,
                derivation_label,
                info.hash.label(),
                hash_detail,
            )
        }
        ChainKind::Ed25519 => account_from_ed25519(
            chain,
            &digest,
            path,
            derivation_label,
            info.hash.label(),
            hash_detail,
        ),
        ChainKind::Cryptonote => {
            let keys = crate::cryptonote::MoneroKeys::from_spend_seed(&digest);
            account_from_monero(
                chain,
                &keys,
                path,
                derivation_label,
                info.hash.label(),
                hash_detail,
            )
        }
    }
}

/// Derive every chain for one account `index` of a mnemonic wallet using the
/// salted strategy. The mnemonic words are hashed with the chain domain and
/// the index mixed in; this deliberately leaves the BIP-32 tree.
fn mnemonic_salted_account(mnemonic: &str, chain: ChainId, index: u32) -> Result<Account> {
    let digest = crate::hash::chain_hash_salted(mnemonic, chain, true, index)?;
    account_from_digest(
        chain,
        digest,
        &format!("salted · #{}", index),
        "H(phrase, #index)",
        "加盐哈希 · 32 byte digest",
    )
}

fn mnemonic_iterated_account(seed: &[u8; 64], chain: ChainId, index: u32) -> Result<Account> {
    let digest = mnemonic_iterated_digest(seed, chain, index)?;
    account_from_digest(
        chain,
        digest,
        &format!("iterated · #{}", index),
        "H^index(base)",
        "迭代哈希 · 32 byte digest",
    )
}

/// The iterated strategy for mnemonic wallets: hash the *base account's key
/// seed* `index` times. The base key comes from the same derivation the base
/// account uses (BIP-32/SLIP-0010 path; Monero via its kholaw spend seed), so
/// the chain is anchored to the wallet's base key, not to the bare words.
fn mnemonic_iterated_digest(seed: &[u8; 64], chain: ChainId, index: u32) -> Result<[u8; 32]> {
    let base = match chain.info().kind {
        ChainKind::Utxo | ChainKind::Evm => bip32::derive_path(seed, chain.info().derivation_path)?
            .key
            .to_bytes()
            .into(),
        ChainKind::Ed25519 => slip10::derive_path(seed, chain.info().derivation_path)?.key,
        ChainKind::Cryptonote => {
            // Anchor to the base account's actual spend key, the same value
            // `from_kholaw_key` reduces to for the base address.
            let mut k64 = slip10::derive_kholaw_path(seed, chain.info().derivation_path)?;
            let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
            k64.zeroize();
            keys.private_spend
        }
    };
    crate::hash::chain_key_iterated(base, chain, index)
}

/// The iterated strategy for passphrase wallets: hash the *base account's
/// 32-byte digest* `index` times, so account #1 already differs from the base.
fn passphrase_iterated_digest(
    phrase: &str,
    chain: ChainId,
    isolated: bool,
    index: u32,
) -> Result<[u8; 32]> {
    let base = crate::hash::chain_hash(phrase, chain, isolated)?;
    crate::hash::chain_key_iterated(base, chain, index)
}

fn passphrase_account_at_index(
    phrase: &str,
    chain: ChainId,
    isolated: bool,
    strategy: BatchStrategy,
    index: u32,
) -> Result<Account> {
    let (digest, path, label, detail) = match strategy {
        BatchStrategy::Path | BatchStrategy::Salted => (
            crate::hash::chain_hash_salted(phrase, chain, isolated, index)?,
            format!("salted · #{}", index),
            if isolated {
                "H_chain(phrase, #i)"
            } else {
                "H(phrase, #i)"
            },
            "加盐哈希 · 32 byte digest",
        ),
        BatchStrategy::Iterated => (
            passphrase_iterated_digest(phrase, chain, isolated, index)?,
            format!("iterated · #{}", index),
            "H_chain^i(base)",
            "迭代哈希 · 32 byte digest",
        ),
    };
    account_from_digest(chain, digest, &path, label, detail)
}

/// Derive `request.count` batch accounts starting at `request.start` for the
/// single chain in the request. The returned vector has one entry per account.
pub fn derive_accounts_batch(
    secret: &WalletSecret,
    request: &BatchRequest,
) -> Result<Vec<Account>> {
    secret.validate()?;
    let max = request.strategy.max_count();
    if request.count == 0 || request.count > max {
        return Err(Error::Message(format!("账户数量必须在 1..={max} 之间")));
    }
    // The iterated chain is anchored at the base key: index 0 would repeat the
    // base account exactly, so it is not a valid starting point.
    if request.strategy == BatchStrategy::Iterated && request.start == 0 {
        return Err(Error::Message(
            "迭代哈希从序号 1 开始（0 号即基础账户）".into(),
        ));
    }
    let end = request.start.saturating_add(request.count).min(max);
    if end - request.start < request.count {
        return Err(Error::Message(format!("起始序号 + 数量不能超过 {max}")));
    }
    (request.start..end)
        .map(|index| derive_chain_account(secret, request.strategy, request.chain, index))
        .collect()
}

/// Derive one chain's account for one derived-account index. This is the only
/// derivation a stored spec needs: a derived account adds one address on one
/// chain, leaving every other chain on the base derivation.
pub fn derive_chain_account(
    secret: &WalletSecret,
    strategy: BatchStrategy,
    chain: ChainId,
    index: u32,
) -> Result<Account> {
    match secret {
        WalletSecret::Mnemonic { .. } => match strategy {
            BatchStrategy::Path => {
                let seed = secret.mnemonic()?.to_seed("");
                mnemonic_account_at_index(&seed, chain, index)
            }
            BatchStrategy::Salted => {
                let mnemonic = secret.mnemonic()?.to_string();
                mnemonic_salted_account(&mnemonic, chain, index)
            }
            BatchStrategy::Iterated => {
                let seed = secret.mnemonic()?.to_seed("");
                mnemonic_iterated_account(&seed, chain, index)
            }
        },
        WalletSecret::Passphrase { phrase, derivation } => {
            let isolated = *derivation == DerivationMode::Isolated;
            passphrase_account_at_index(phrase, chain, isolated, strategy, index)
        }
    }
}

/// Derive the full per-chain account list for one stored
/// [`BatchAccountSpec`].
/// A spec covers a single chain, so the result is the base derivation with
/// that one chain replaced by the derived account.
pub fn derive_accounts_for_spec(
    secret: &WalletSecret,
    spec: &BatchAccountSpec,
) -> Result<Vec<Account>> {
    let mut accounts = derive_accounts(secret)?;
    if let Some(slot) = accounts
        .iter_mut()
        .find(|account| account.chain == spec.chain)
    {
        *slot = derive_chain_account(secret, spec.strategy, spec.chain, spec.index)?;
    }
    Ok(accounts)
}

/// Derive one chain's private key for a stored [`BatchAccountSpec`] (or the
/// base account when `strategy`/`index` are `None`). Powers the per-address
/// detail dialog; the caller must gate it behind a password check like any
/// other private-key export.
pub fn derive_chain_private_key_for_spec(
    secret: &WalletSecret,
    spec: Option<&BatchAccountSpec>,
    chain: ChainId,
) -> Result<PrivateKeyMaterial> {
    secret.validate()?;
    let (strategy, index) = match spec {
        None => (None, 0u32),
        Some(spec) => (Some(spec.strategy), spec.index),
    };
    match secret {
        WalletSecret::Mnemonic { .. } => {
            let mnemonic = secret.mnemonic()?;
            match (chain.info().kind, strategy) {
                (ChainKind::Utxo | ChainKind::Evm, None) => {
                    let seed = mnemonic.to_seed("");
                    let path = chain.info().derivation_path;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Secp(bip32::derive_path(&seed, path)?.key),
                    })
                }
                (ChainKind::Ed25519, None) => {
                    let seed = mnemonic.to_seed("");
                    let path = chain.info().derivation_path;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Ed25519(slip10::derive_path(&seed, path)?.key),
                    })
                }
                (ChainKind::Cryptonote, None) => {
                    let seed = mnemonic.to_seed("");
                    let base = chain.info().derivation_path;
                    let mut k64 = [0u8; 64];
                    k64.copy_from_slice(&crate::slip10::derive_kholaw_path(&seed, base)?);
                    let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                    k64.zeroize();
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Monero(keys),
                    })
                }
                // Derived accounts below: Path shares the seed, the rest are
                // pure hashes of the phrase.
                (ChainKind::Utxo | ChainKind::Evm, Some(BatchStrategy::Path)) => {
                    let seed = mnemonic.to_seed("");
                    let info = chain.info();
                    let path = replace_last_index(info.derivation_path, index);
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Secp(bip32::derive_path(&seed, &path)?.key),
                    })
                }
                (ChainKind::Ed25519, Some(BatchStrategy::Path)) => {
                    let seed = mnemonic.to_seed("");
                    let path = replace_last_index(chain.info().derivation_path, index);
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Ed25519(slip10::derive_path(&seed, &path)?.key),
                    })
                }
                (ChainKind::Cryptonote, Some(BatchStrategy::Path)) => {
                    // Same convention as `mnemonic_account_at_index`: repeat
                    // the base Monero account for every index.
                    let seed = mnemonic.to_seed("");
                    let base = chain.info().derivation_path;
                    let mut k64 = [0u8; 64];
                    k64.copy_from_slice(&crate::slip10::derive_kholaw_path(&seed, base)?);
                    let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                    k64.zeroize();
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Monero(keys),
                    })
                }
                (_, Some(BatchStrategy::Salted)) => {
                    let digest =
                        crate::hash::chain_hash_salted(&mnemonic.to_string(), chain, true, index)?;
                    Ok(digest_to_key(chain, digest))
                }
                (_, Some(BatchStrategy::Iterated)) => {
                    let seed = mnemonic.to_seed("");
                    let digest = mnemonic_iterated_digest(&seed, chain, index)?;
                    Ok(digest_to_key(chain, digest))
                }
            }
        }
        WalletSecret::Passphrase { phrase, derivation } => {
            let isolated = *derivation == DerivationMode::Isolated;
            let digest = match strategy {
                None => crate::hash::chain_hash(phrase, chain, isolated)?,
                Some(BatchStrategy::Path | BatchStrategy::Salted) => {
                    crate::hash::chain_hash_salted(phrase, chain, isolated, index)?
                }
                Some(BatchStrategy::Iterated) => {
                    passphrase_iterated_digest(phrase, chain, isolated, index)?
                }
            };
            Ok(digest_to_key(chain, digest))
        }
    }
}

/// Wrap a raw 32-byte digest into chain-appropriate key material. Mirrors the
/// key branches of [`derive_private_key`].
fn digest_to_key(chain: ChainId, digest: [u8; 32]) -> PrivateKeyMaterial {
    match chain.info().kind {
        ChainKind::Utxo | ChainKind::Evm => PrivateKeyMaterial {
            chain,
            // `secp_from_digest` is total: it rehashes in the ~2^-128 case.
            inner: PrivateKeyInner::Secp(
                secp_from_digest(digest).expect("digest -> secp256k1 is total"),
            ),
        },
        ChainKind::Ed25519 => PrivateKeyMaterial {
            chain,
            inner: PrivateKeyInner::Ed25519(digest),
        },
        ChainKind::Cryptonote => PrivateKeyMaterial {
            chain,
            inner: PrivateKeyInner::Monero(crate::cryptonote::MoneroKeys::from_spend_seed(&digest)),
        },
    }
}

/// Replace the final path component: `m/84'/0'/0'/0/0` + index 3 becomes
/// `m/84'/0'/0'/0/3`. Hardened markers on the replaced component are kept:
/// `m/44'/501'/0'/0'` + 2 becomes `m/44'/501'/0'/2'`.
fn replace_last_index(path: &str, index: u32) -> String {
    match path.rsplit_once('/') {
        Some((head, last)) => {
            let marker = if last.ends_with(['\'', 'h']) {
                &last[last.len() - 1..]
            } else {
                ""
            };
            format!("{head}/{index}{marker}")
        }
        None => path.to_string(),
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
                ChainKind::Cryptonote => {
                    let mut k64 = [0u8; 64];
                    k64.copy_from_slice(&slip10::derive_kholaw_path(&seed, info.derivation_path)?);
                    let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                    k64.zeroize();
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Monero(keys),
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
                ChainKind::Cryptonote => Ok(PrivateKeyMaterial {
                    chain,
                    inner: PrivateKeyInner::Monero(crate::cryptonote::MoneroKeys::from_spend_seed(
                        &digest,
                    )),
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
    fn batch_path_bumps_the_bip32_address_index() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Path,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 0,
            },
        )
        .unwrap();
        assert_eq!(batch.len(), 2);
        // Account #0 must equal the plain derivation on that chain.
        let plain = derive_accounts(&secret)
            .unwrap()
            .into_iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        assert_eq!(plain.address, batch[0].address);
        // Account #1 must differ and bump the address index.
        assert_ne!(plain.address, batch[1].address);
        assert!(
            batch[1].path.ends_with("/1") || batch[1].path.ends_with("/1'"),
            "path should end in /1 or /1', got {}",
            batch[1].path
        );
        // Other chains are untouched by a single-chain batch: their base
        // derivation has no "/1" variant because no spec was created for them.
        let spec = BatchAccountSpec::new(BatchStrategy::Path, ChainId::Bitcoin, 1);
        let accounts = derive_accounts_for_spec(&secret, &spec).unwrap();
        let eth_base = derive_accounts(&secret)
            .unwrap()
            .into_iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        let eth = accounts
            .into_iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        assert_eq!(
            eth.address, eth_base.address,
            "non-target chains stay on base"
        );
    }

    #[test]
    fn batch_salted_is_deterministic_and_index_disjoint() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 3,
                start: 0,
            },
        )
        .unwrap();
        let again = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 3,
                start: 0,
            },
        )
        .unwrap();
        for (a, b) in batch.iter().zip(&again) {
            assert_eq!(a.address, b.address);
        }
        // All three accounts differ.
        assert_ne!(batch[0].address, batch[1].address);
        assert_ne!(batch[1].address, batch[2].address);
        assert_ne!(batch[0].address, batch[2].address);
    }

    #[test]
    fn batch_iterated_is_anchored_to_base_key() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let plain = derive_accounts(&secret)
            .unwrap()
            .into_iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        // Index 1 must already differ from the base account: the chain starts
        // at the base key and hashes it once more.
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Iterated,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 1,
            },
        )
        .unwrap();
        assert_ne!(
            plain.address, batch[0].address,
            "H(base) must differ from base"
        );
        // And index 0 is rejected outright — it would repeat the base.
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Iterated,
                chain: ChainId::Bitcoin,
                count: 1,
                start: 0,
            },
        )
        .is_err());
    }

    #[test]
    fn batch_start_window_is_respected_and_bounded() {
        let secret = WalletSecret::new_passphrase("seed phrase", DerivationMode::Isolated).unwrap();
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 5,
            },
        )
        .unwrap();
        assert_eq!(batch.len(), 2);
        assert!(batch[0].path.ends_with("#5"), "got {}", batch[0].path);
        assert!(batch[1].path.ends_with("#6"), "got {}", batch[1].path);

        // Out-of-range requests are rejected.
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 0,
                start: 0,
            }
        )
        .is_err());
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 101,
                start: 0,
            }
        )
        .is_err());
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Iterated,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 49,
            }
        )
        .is_err());
    }

    #[test]
    fn replace_last_index_swaps_only_the_final_component() {
        assert_eq!(replace_last_index("m/84'/0'/0'/0/0", 7), "m/84'/0'/0'/0/7");
        assert_eq!(
            replace_last_index("m/44'/60'/0'/0/0", 12),
            "m/44'/60'/0'/0/12"
        );
        // Hardened final components keep their marker.
        assert_eq!(
            replace_last_index("m/44'/501'/0'/0'", 2),
            "m/44'/501'/0'/2'"
        );
    }

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
    fn passphrase_accounts_expose_the_chain_hash_not_the_derivation_detail() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let accounts = derive_accounts(&secret).unwrap();

        let ltc = accounts
            .iter()
            .find(|x| x.chain == ChainId::Litecoin)
            .unwrap();
        assert_eq!(ltc.derivation, "H_chain(phrase)");
        assert_eq!(ltc.hash_algorithm, "scrypt");
        assert_eq!(ltc.hash_detail, "N=16384, r=8, p=1 · 32 byte digest");

        let xmr = accounts
            .iter()
            .find(|x| x.chain == ChainId::Monero)
            .unwrap();
        assert_eq!(xmr.derivation, "H_chain(phrase)");
        assert_eq!(xmr.hash_algorithm, "Keccak-256");
        assert_eq!(xmr.hash_detail, "CryptoNote Keccak-256 · 32 byte digest");
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
        // Strict mode hashes the phrase alone, so every chain that uses the
        // same native hash ends up with the very same key — Ethereum Classic
        // and Polygon included. Only the chain's own parameters (chain id for
        // replay protection, explorer, symbol) differ.
        for chain in [ChainId::EthereumClassic, ChainId::Polygon] {
            let account = accounts.iter().find(|a| a.chain == chain).unwrap();
            assert_eq!(
                account.address, eth.address,
                "{chain:?} must share the ETH address"
            );
        }
        // Keccak chains are unaffected by the chain list; a SHA-256 chain is not.
        let btc = accounts
            .iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        assert_ne!(btc.address, eth.address);
    }

    #[test]
    fn bip39_extended_chain_vectors() {
        // Independent vectors generated with bip_utils (BIP-44 account 0,
        // external index 0) for the mnemonic used across the test suite.
        let mnemonic =
            "legal winner thank year wave sausage worth useful legal winner thank yellow";
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

        // EVM chains sharing coin type 60 reuse the ETH address.
        let eth = address(ChainId::Ethereum);
        assert_eq!(eth, "0x58A57ed9d8d624cBD12e2C467D34787555bB1b25");
        for chain in [ChainId::Bsc, ChainId::Polygon] {
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
        // CryptoNote dual-key address (SLIP-10 kholaw m/44'/128'/0'/0/0,
        // spend = reduce(Keccak(kL ‖ kR)) — verified against bip_utils).
        assert_eq!(
            address(ChainId::Monero),
            "46XrxrAa5aXcfkFrdSACS2EfmfxHdJkXLcGuDD2ezLNFfrfMgBJPoP5NAEz3N9WSJuhPox5S78AY3PGVeZQczueyDDT66tY"
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
        for chain in [ChainId::Bsc, ChainId::Polygon, ChainId::EthereumClassic] {
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
        assert!(find(ChainId::Dogecoin).send_supported);
        assert!(find(ChainId::BitcoinCash).send_supported);
        // Monero spends are signed by a local monero-wallet-rpc.
        assert!(find(ChainId::Monero).send_supported);
    }
}
