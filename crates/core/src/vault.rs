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

pub const VAULT_VERSION: u32 = 1;
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

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultData {
    pub version: u32,
    pub label: String,
    pub created_at: i64,
    pub secret: WalletSecret,
    pub settings: Settings,
    pub contacts: Vec<Contact>,
    pub activity: Vec<ActivityItem>,
}

impl VaultData {
    pub fn new(label: impl Into<String>, secret: WalletSecret) -> Self {
        Self {
            version: VAULT_VERSION,
            label: label.into(),
            created_at: now(),
            secret,
            settings: Settings::default(),
            contacts: Vec::new(),
            activity: Vec::new(),
        }
    }
}

impl std::fmt::Debug for VaultData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VaultData")
            .field("version", &self.version)
            .field("label", &self.label)
            .field("created_at", &self.created_at)
            .field("secret", &self.secret)
            .field("settings", &self.settings)
            .field("contacts", &self.contacts.len())
            .field("activity", &self.activity.len())
            .finish()
    }
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

fn encrypt_with_key(data: &VaultData, key: &[u8; 32]) -> Result<(String, String)> {
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let mut plaintext = Zeroizing::new(serde_json::to_vec(data)?);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: plaintext.as_ref(),
                aad: AAD,
            },
        )
        .map_err(|_| Error::Crypto("vault encryption failed".into()))?;
    plaintext.clear();
    Ok((hex::encode(nonce), hex::encode(ciphertext)))
}

fn decrypt_with_key(nonce_hex: &str, ciphertext_hex: &str, key: &[u8; 32]) -> Result<VaultData> {
    let nonce = hex::decode(nonce_hex)?;
    if nonce.len() != NONCE_LEN {
        return Err(Error::CorruptVault);
    }
    let ciphertext = hex::decode(ciphertext_hex)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let mut plaintext = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: AAD,
            },
        )
        .map_err(|_| Error::IncorrectPassword)?;
    let data: VaultData = serde_json::from_slice(&plaintext).map_err(|_| Error::CorruptVault)?;
    plaintext.clear();
    data.secret.validate()?;
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
        if self.version != VAULT_VERSION {
            return Err(Error::CorruptVault);
        }
        let key = self.kdf.derive_key(password)?;
        let data = decrypt_with_key(&self.nonce, &self.ciphertext, &key)?;
        Ok((data, key))
    }

    /// Re-encrypt with an existing key (used after every vault mutation).
    pub fn rewrap(&mut self, data: &VaultData, key: &[u8; 32]) -> Result<()> {
        let (nonce, ciphertext) = encrypt_with_key(data, key)?;
        self.nonce = nonce;
        self.ciphertext = ciphertext;
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

    fn secret() -> WalletSecret {
        WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated).unwrap()
    }

    #[test]
    fn vault_roundtrip_and_wrong_password() {
        let data = VaultData::new("Test", secret());
        let file =
            VaultFile::create(&data, "correct horse battery", Some("garden".into())).unwrap();
        let (unlocked, _key) = file.unlock("correct horse battery").unwrap();
        assert_eq!(unlocked.label, "Test");
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
        assert_eq!(again.label, "Renamed");
        assert!(again.settings.hide_balances);
    }
}
