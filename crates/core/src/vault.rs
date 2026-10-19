use crate::derive::WalletSecret;
use crate::error::{Error, Result};
use crate::models::{ActivityItem, Contact, Settings};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::path::Path;
use zeroize::Zeroizing;

/// Current vault format. Format 1 stored a single wallet at the top level,
/// format 2 stores a list of wallets sharing one password, one address book
/// and one settings block.
pub const VAULT_VERSION: u32 = 2;
const AAD: &[u8] = b"lycoris-vault-v1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfParams {
    pub algorithm: String,
    pub salt: String,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl KdfParams {
    fn new() -> Self {
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        Self {
            algorithm: "argon2id".into(),
            salt: hex::encode(salt),
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }

    pub fn derive_key(&self, password: &str) -> Result<Zeroizing<[u8; 32]>> {
        let salt = hex::decode(&self.salt)?;
        let params = Params::new(self.memory_kib, self.iterations, self.parallelism, Some(32))
            .map_err(|e| Error::Crypto(format!("invalid Argon2 parameters: {e}")))?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; 32]);
        argon
            .hash_password_into(password.as_bytes(), &salt, key.as_mut())
            .map_err(|e| Error::Crypto(format!("Argon2 failed: {e}")))?;
        Ok(key)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultFile {
    pub version: u32,
    pub kdf: KdfParams,
    pub nonce: String,
    pub ciphertext: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub hint: Option<String>,
}

/// A single wallet inside the vault. The secret is the only key material and
/// never leaves the encrypted payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletEntry {
    pub id: String,
    pub label: String,
    pub created_at: i64,
    pub secret: WalletSecret,
    #[serde(default)]
    pub activity: Vec<ActivityItem>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultData {
    pub version: u32,
    pub created_at: i64,
    pub settings: Settings,
    pub contacts: Vec<Contact>,
    pub wallets: Vec<WalletEntry>,
    pub active_wallet: String,
}

/// Wire format used when reading. It accepts both vault formats: format 2
/// (`wallets` + `activeWallet`) and format 1 (`label` + `secret` + `activity`
/// at the top level, migrated on the fly).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct VaultDataWire {
    #[serde(default)]
    created_at: i64,
    #[serde(default)]
    settings: Option<Settings>,
    #[serde(default)]
    contacts: Vec<Contact>,
    #[serde(default)]
    wallets: Vec<WalletEntry>,
    #[serde(default)]
    active_wallet: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    secret: Option<WalletSecret>,
    #[serde(default)]
    activity: Vec<ActivityItem>,
}

impl<'de> Deserialize<'de> for VaultData {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let wire = VaultDataWire::deserialize(deserializer)?;
        let created_at = if wire.created_at > 0 {
            wire.created_at
        } else {
            now()
        };

        let mut wallets = wire.wallets;
        if wallets.is_empty() {
            let secret = wire
                .secret
                .ok_or_else(|| D::Error::missing_field("wallets"))?;
            wallets.push(WalletEntry {
                id: random_id(),
                label: if wire.label.trim().is_empty() {
                    "钱包".to_string()
                } else {
                    wire.label.clone()
                },
                created_at,
                secret,
                activity: wire.activity,
            });
        }
        let active_wallet = if wallets.iter().any(|wallet| wallet.id == wire.active_wallet) {
            wire.active_wallet
        } else {
            wallets[0].id.clone()
        };

        Ok(VaultData {
            version: VAULT_VERSION,
            created_at,
            settings: wire.settings.unwrap_or_default(),
            contacts: wire.contacts,
            wallets,
            active_wallet,
        })
    }
}

impl VaultData {
    pub fn new(label: impl Into<String>, secret: WalletSecret) -> Self {
        let created_at = now();
        let entry = WalletEntry {
            id: random_id(),
            label: label.into(),
            created_at,
            secret,
            activity: Vec::new(),
        };
        Self {
            version: VAULT_VERSION,
            created_at,
            settings: Settings::default(),
            contacts: Vec::new(),
            active_wallet: entry.id.clone(),
            wallets: vec![entry],
        }
    }

    /// Append a wallet and make it active. Returns the new wallet id.
    pub fn add_wallet(&mut self, label: impl Into<String>, secret: WalletSecret) -> String {
        let id = random_id();
        let label = unique_label(&self.wallets, label.into());
        self.wallets.push(WalletEntry {
            id: id.clone(),
            label,
            created_at: now(),
            secret,
            activity: Vec::new(),
        });
        self.active_wallet = id.clone();
        id
    }

    /// Remove a wallet. The vault always keeps at least one.
    pub fn remove_wallet(&mut self, id: &str) -> Result<()> {
        let position = self
            .wallets
            .iter()
            .position(|wallet| wallet.id == id)
            .ok_or(Error::WalletNotFound)?;
        if self.wallets.len() <= 1 {
            return Err(Error::LastWallet);
        }
        self.wallets.remove(position);
        if self.active_wallet == id {
            self.active_wallet = self.wallets[0].id.clone();
        }
        Ok(())
    }

    pub fn rename_wallet(&mut self, id: &str, label: impl Into<String>) -> Result<()> {
        let wallet = self
            .wallets
            .iter_mut()
            .find(|wallet| wallet.id == id)
            .ok_or(Error::WalletNotFound)?;
        wallet.label = label.into();
        Ok(())
    }

    pub fn find(&self, id: &str) -> Option<&WalletEntry> {
        self.wallets.iter().find(|wallet| wallet.id == id)
    }

    pub fn active(&self) -> Result<&WalletEntry> {
        self.find(&self.active_wallet).ok_or(Error::CorruptVault)
    }

    pub fn active_mut(&mut self) -> Result<&mut WalletEntry> {
        let id = self.active_wallet.clone();
        self.wallets
            .iter_mut()
            .find(|wallet| wallet.id == id)
            .ok_or(Error::CorruptVault)
    }

    pub fn active_label(&self) -> String {
        self.active()
            .map(|wallet| wallet.label.clone())
            .unwrap_or_default()
    }

    /// Every secret must still parse, otherwise the vault is corrupt (or was
    /// tampered with despite the AEAD tag).
    pub fn validate(&self) -> Result<()> {
        if self.wallets.is_empty() {
            return Err(Error::CorruptVault);
        }
        for wallet in &self.wallets {
            wallet.secret.validate()?;
        }
        Ok(())
    }
}

impl std::fmt::Debug for VaultData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultData")
            .field("version", &self.version)
            .field("created_at", &self.created_at)
            .field("settings", &self.settings)
            .field("contacts", &self.contacts.len())
            .field("wallets", &self.wallets.len())
            .field("active_wallet", &self.active_wallet)
            .finish()
    }
}

fn random_id() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// "钱包" → "钱包 2" → "钱包 3" when names collide.
fn unique_label(wallets: &[WalletEntry], base: String) -> String {
    let base = if base.trim().is_empty() {
        "钱包".to_string()
    } else {
        base.trim().to_string()
    };
    if !wallets.iter().any(|wallet| wallet.label == base) {
        return base;
    }
    for index in 2..1000 {
        let candidate = format!("{base} {index}");
        if !wallets.iter().any(|wallet| wallet.label == candidate) {
            return candidate;
        }
    }
    base
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn encrypt_bytes(plaintext: &[u8], key: &[u8; 32]) -> Result<(String, String)> {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext,
                aad: AAD,
            },
        )
        .map_err(|_| Error::Crypto("vault encryption failed".into()))?;
    Ok((hex::encode(nonce), hex::encode(ciphertext)))
}

fn encrypt_with_key(data: &VaultData, key: &[u8; 32]) -> Result<(String, String)> {
    let mut plaintext = Zeroizing::new(serde_json::to_vec(data)?);
    let result = encrypt_bytes(plaintext.as_ref(), key);
    plaintext.clear();
    result
}

fn decrypt_bytes(
    nonce_hex: &str,
    ciphertext_hex: &str,
    key: &[u8; 32],
) -> Result<Zeroizing<Vec<u8>>> {
    let nonce = hex::decode(nonce_hex)?;
    if nonce.len() != NONCE_LEN {
        return Err(Error::CorruptVault);
    }
    let ciphertext = hex::decode(ciphertext_hex)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: AAD,
            },
        )
        .map_err(|_| Error::IncorrectPassword)?;
    Ok(Zeroizing::new(plaintext))
}

fn decrypt_with_key(nonce_hex: &str, ciphertext_hex: &str, key: &[u8; 32]) -> Result<VaultData> {
    let mut plaintext = decrypt_bytes(nonce_hex, ciphertext_hex, key)?;
    let data: VaultData = serde_json::from_slice(&plaintext).map_err(|_| Error::CorruptVault)?;
    plaintext.clear();
    data.validate()?;
    Ok(data)
}

impl VaultFile {
    pub fn create(data: &VaultData, password: &str, hint: Option<String>) -> Result<Self> {
        if password.chars().count() < 8 {
            return Err(Error::WeakPassword);
        }
        let kdf = KdfParams::new();
        let key = kdf.derive_key(password)?;
        let (nonce, ciphertext) = encrypt_with_key(data, &key)?;
        Ok(Self {
            version: VAULT_VERSION,
            kdf,
            nonce,
            ciphertext,
            created_at: now(),
            updated_at: now(),
            hint,
        })
    }

    pub fn unlock(&self, password: &str) -> Result<(VaultData, Zeroizing<[u8; 32]>)> {
        if self.version == 0 || self.version > VAULT_VERSION {
            return Err(Error::CorruptVault);
        }
        let key = self.kdf.derive_key(password)?;
        let data = decrypt_with_key(&self.nonce, &self.ciphertext, &key)?;
        Ok((data, key))
    }

    /// True when the file on disk still uses an older format and should be
    /// rewritten in the current one.
    pub fn needs_upgrade(&self) -> bool {
        self.version < VAULT_VERSION
    }

    /// Re-encrypt with an existing key (used after every vault mutation).
    pub fn rewrap(&mut self, data: &VaultData, key: &[u8; 32]) -> Result<()> {
        let (nonce, ciphertext) = encrypt_with_key(data, key)?;
        self.nonce = nonce;
        self.ciphertext = ciphertext;
        self.version = VAULT_VERSION;
        self.updated_at = now();
        Ok(())
    }

    pub fn change_password(&mut self, data: &VaultData, new_password: &str) -> Result<()> {
        if new_password.chars().count() < 8 {
            return Err(Error::WeakPassword);
        }
        self.kdf = KdfParams::new();
        let key = self.kdf.derive_key(new_password)?;
        self.rewrap(data, &key)
    }

    pub fn read(path: &Path) -> Result<Self> {
        let bytes = std::fs::read(path)?;
        let file: VaultFile = serde_json::from_slice(&bytes)?;
        Ok(file)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        let temp = path.with_extension("tmp");
        std::fs::write(&temp, &bytes)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&temp, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derive::DerivationMode;
    use serde_json::json;

    fn secret() -> WalletSecret {
        WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated).unwrap()
    }

    fn other_secret() -> WalletSecret {
        WalletSecret::new_passphrase("a slow river in winter", DerivationMode::Strict).unwrap()
    }

    #[test]
    fn vault_roundtrip_and_wrong_password() {
        let data = VaultData::new("Test", secret());
        let file =
            VaultFile::create(&data, "correct horse battery", Some("garden".into())).unwrap();
        let (unlocked, _key) = file.unlock("correct horse battery").unwrap();
        assert_eq!(unlocked.active_label(), "Test");
        assert!(file.unlock("wrong password").is_err());
    }

    #[test]
    fn vault_rewrap_preserves_data() {
        let data = VaultData::new("Test", secret());
        let mut file = VaultFile::create(&data, "correct horse battery", None).unwrap();
        let (_unlocked, key) = file.unlock("correct horse battery").unwrap();
        let mut updated = VaultData::new("Renamed", secret());
        updated.settings.hide_balances = true;
        file.rewrap(&updated, &key).unwrap();
        let (again, _) = file.unlock("correct horse battery").unwrap();
        assert_eq!(again.active_label(), "Renamed");
        assert!(again.settings.hide_balances);
    }

    #[test]
    fn multiple_wallets_roundtrip_with_activity() {
        let mut data = VaultData::new("主钱包", secret());
        let second = data.add_wallet("备用", other_secret());
        assert_eq!(data.wallets.len(), 2);
        assert_eq!(data.active_wallet, second);
        data.active_mut().unwrap().activity.push(ActivityItem {
            id: "a1".into(),
            chain: crate::chains::ChainId::Bitcoin,
            direction: crate::models::Direction::Send,
            amount: "0.01".into(),
            symbol: "BTC".into(),
            counterparty: "bc1qexample".into(),
            txid: "deadbeef".into(),
            status: crate::models::ActivityStatus::Pending,
            fee: "0.0001".into(),
            timestamp: now(),
            explorer_url: "https://mempool.space/tx/deadbeef".into(),
        });

        let file = VaultFile::create(&data, "correct horse battery", None).unwrap();
        let (unlocked, _) = file.unlock("correct horse battery").unwrap();
        assert_eq!(unlocked.wallets.len(), 2);
        assert_eq!(unlocked.active_label(), "备用");
        assert_eq!(unlocked.active().unwrap().activity.len(), 1);
        let first = unlocked
            .wallets
            .iter()
            .find(|wallet| wallet.label == "主钱包")
            .unwrap();
        assert!(first.activity.is_empty());
        assert!(first.secret.passphrase().is_some());
    }

    #[test]
    fn duplicate_labels_are_numbered() {
        let mut data = VaultData::new("钱包", secret());
        data.add_wallet("钱包", other_secret());
        data.add_wallet("   ", other_secret());
        let labels: Vec<_> = data
            .wallets
            .iter()
            .map(|wallet| wallet.label.as_str())
            .collect();
        assert_eq!(labels, ["钱包", "钱包 2", "钱包 3"]);
    }

    #[test]
    fn last_wallet_cannot_be_removed() {
        let mut data = VaultData::new("Only", secret());
        let id = data.wallets[0].id.clone();
        assert!(matches!(data.remove_wallet(&id), Err(Error::LastWallet)));
        data.add_wallet("B", other_secret());
        assert!(matches!(
            data.remove_wallet("missing"),
            Err(Error::WalletNotFound)
        ));
    }

    #[test]
    fn removing_the_active_wallet_falls_back_to_the_first() {
        let mut data = VaultData::new("A", secret());
        let second = data.add_wallet("B", other_secret());
        assert_eq!(second.len(), 32);
        assert!(second.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(data.active_wallet, second);
        data.remove_wallet(&second).unwrap();
        assert_eq!(data.active_label(), "A");
        assert!(data.rename_wallet("nope", "x").is_err());
    }

    #[test]
    fn legacy_single_wallet_payload_migrates() {
        // A vault format 1 payload: one wallet stored at the top level.
        let legacy = json!({
            "version": 1,
            "label": "旧钱包",
            "createdAt": 1_700_000_000,
            "secret": { "mode": "passphrase", "phrase": "a quiet garden at dusk", "derivation": "isolated" },
            "settings": {
                "fiat": "cny",
                "theme": "dark",
                "hideBalances": true,
                "autoLockMinutes": 5,
                "priceService": true
            },
            "contacts": [{
                "id": "c1",
                "name": "Alice",
                "chain": "bitcoin",
                "address": "bc1qexample",
                "note": "",
                "createdAt": 1_700_000_100
            }],
            "activity": [{
                "id": "a1",
                "chain": "bitcoin",
                "direction": "send",
                "amount": "0.01",
                "symbol": "BTC",
                "counterparty": "bc1qexample",
                "txid": "deadbeef",
                "status": "pending",
                "fee": "0.0001",
                "timestamp": 1_700_000_200,
                "explorerUrl": "https://mempool.space/tx/deadbeef"
            }]
        });
        let data: VaultData = serde_json::from_value(legacy).unwrap();
        assert_eq!(data.version, VAULT_VERSION);
        assert_eq!(data.wallets.len(), 1);
        assert_eq!(data.active_label(), "旧钱包");
        assert_eq!(data.created_at, 1_700_000_000);
        assert_eq!(data.settings.fiat, crate::models::FiatCurrency::Cny);
        assert_eq!(data.contacts.len(), 1);
        // The activity list moved into the wallet it belonged to.
        assert_eq!(data.wallets[0].activity.len(), 1);
        assert_eq!(data.wallets[0].activity[0].txid, "deadbeef");
        assert_eq!(data.active_wallet, data.wallets[0].id);
        data.validate().unwrap();
    }

    #[test]
    fn legacy_vault_file_upgrades_on_rewrap() {
        let data = VaultData::new("Legacy", secret());
        let mut file = VaultFile::create(&data, "correct horse battery", None).unwrap();
        let key = file.kdf.derive_key("correct horse battery").unwrap();

        // Replace the payload with a format-1 plaintext encrypted with the
        // same key, and mark the file as version 1.
        let legacy = serde_json::to_vec(&json!({
            "version": 1,
            "label": "旧钱包",
            "createdAt": 1_700_000_000,
            "secret": { "mode": "passphrase", "phrase": "a quiet garden at dusk", "derivation": "isolated" },
            "settings": {
                "fiat": "usd",
                "theme": "system",
                "hideBalances": false,
                "autoLockMinutes": 5,
                "priceService": true
            },
            "contacts": [],
            "activity": []
        }))
        .unwrap();
        let (nonce, ciphertext) = encrypt_bytes(&legacy, &key).unwrap();
        file.nonce = nonce;
        file.ciphertext = ciphertext;
        file.version = 1;
        assert!(file.needs_upgrade());

        let (mut unlocked, key) = file.unlock("correct horse battery").unwrap();
        assert_eq!(unlocked.active_label(), "旧钱包");
        unlocked
            .rename_wallet(&unlocked.active_wallet.clone(), "新名字")
            .unwrap();
        file.rewrap(&unlocked, &key).unwrap();
        assert!(!file.needs_upgrade());

        let (again, _) = file.unlock("correct horse battery").unwrap();
        assert_eq!(again.active_label(), "新名字");
        assert_eq!(again.wallets.len(), 1);
    }
}
