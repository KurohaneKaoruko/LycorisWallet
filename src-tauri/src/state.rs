//! Unlocked-session state shared by every command.
//!
//! The plaintext vault only ever lives in this process. The frontend never
//! receives a secret: commands return addresses, balances and transaction
//! hashes, never keys.

use lycoris_core::chains::ChainId;
use lycoris_core::derive::{preview_derivation, Account, WalletSecret};
use lycoris_core::error::{Error, Result as CoreResult};
use lycoris_core::models::{Contact, Settings, VaultStatus, WalletView};
use lycoris_core::vault::{now, VaultData, VaultFile};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use zeroize::Zeroizing;

const VAULT_FILE: &str = "vault.json";
/// Failed unlock attempts before a forced cool-down.
const MAX_FAILURES: u32 = 5;
const LOCKOUT_SECONDS: i64 = 30;

struct Session {
    vault: Option<VaultFile>,
    key: Option<Zeroizing<[u8; 32]>>,
    data: Option<VaultData>,
    accounts: Vec<Account>,
    fingerprint: String,
    formula: String,
    last_active: i64,
    failures: u32,
    locked_until: i64,
}

impl Session {
    fn empty() -> Self {
        Self {
            vault: None,
            key: None,
            data: None,
            accounts: Vec::new(),
            fingerprint: String::new(),
            formula: String::new(),
            last_active: now(),
            failures: 0,
            locked_until: 0,
        }
    }

    fn is_unlocked(&self) -> bool {
        self.data.is_some() && self.key.is_some()
    }

    fn clear(&mut self) {
        self.key = None;
        self.data = None;
        self.accounts.clear();
        self.fingerprint.clear();
        self.formula.clear();
    }
}

/// Application state. Cheap to clone; all clones share one session.
#[derive(Clone)]
pub struct AppState {
    dir: PathBuf,
    session: Arc<Mutex<Session>>,
}

impl AppState {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            session: Arc::new(Mutex::new(Session::empty())),
        }
    }

    pub fn vault_path(&self) -> PathBuf {
        self.dir.join(VAULT_FILE)
    }

    fn session(&self) -> MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Never exposes secrets; safe to call while locked.
    pub fn status(&self) -> VaultStatus {
        let session = self.session();
        let file = VaultFile::read(&self.vault_path()).ok();
        VaultStatus {
            exists: file.is_some(),
            locked: !session.is_unlocked(),
            has_hint: file.as_ref().map(|f| f.hint.is_some()).unwrap_or(false),
            hint: file.as_ref().and_then(|f| f.hint.clone()),
            label: session.data.as_ref().map(|data| data.label.clone()),
        }
    }

    pub fn create(
        &self,
        secret: WalletSecret,
        password: &str,
        hint: Option<String>,
        label: Option<String>,
    ) -> CoreResult<WalletView> {
        let path = self.vault_path();
        if path.exists() {
            return Err(Error::VaultExists);
        }
        let label = label
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default_label(&secret));
        let data = VaultData::new(label, secret);
        let file = VaultFile::create(&data, password, hint)?;
        file.write(&path)?;
        self.adopt(file, password)
    }

    pub fn unlock(&self, password: &str) -> CoreResult<WalletView> {
        {
            let session = self.session();
            let remaining = session.locked_until - now();
            if remaining > 0 {
                return Err(Error::TooManyAttempts(remaining as u64));
            }
        }
        let path = self.vault_path();
        if !path.exists() {
            return Err(Error::VaultNotFound);
        }
        let file = VaultFile::read(&path)?;
        match file.unlock(password) {
            Ok((data, key)) => {
                let preview = preview_derivation(&data.secret)?;
                let mut session = self.session();
                session.failures = 0;
                session.locked_until = 0;
                session.vault = Some(file);
                session.key = Some(key);
                session.data = Some(data);
                session.accounts = preview.accounts.clone();
                session.fingerprint = preview.fingerprint.clone();
                session.formula = preview.formula.clone();
                session.last_active = now();
                Ok(view_from_parts(
                    session.data.as_ref().expect("data set"),
                    &session.accounts,
                    &session.fingerprint,
                    &session.formula,
                ))
            }
            Err(error) => {
                let mut session = self.session();
                session.failures += 1;
                if session.failures >= MAX_FAILURES {
                    session.failures = 0;
                    session.locked_until = now() + LOCKOUT_SECONDS;
                }
                Err(error)
            }
        }
    }

    /// Re-read the vault file and unlock with a freshly derived key. Used by
    /// commands that need to prove knowledge of the password again.
    pub fn verify_password(&self, password: &str) -> CoreResult<bool> {
        let file = VaultFile::read(&self.vault_path())?;
        match file.unlock(password) {
            Ok(_) => Ok(true),
            Err(Error::IncorrectPassword) => Ok(false),
            Err(error) => Err(error),
        }
    }

    fn adopt(&self, file: VaultFile, password: &str) -> CoreResult<WalletView> {
        let (data, key) = file.unlock(password)?;
        let preview = preview_derivation(&data.secret)?;
        let mut session = self.session();
        session.vault = Some(file);
        session.key = Some(key);
        session.data = Some(data);
        session.accounts = preview.accounts.clone();
        session.fingerprint = preview.fingerprint.clone();
        session.formula = preview.formula.clone();
        session.last_active = now();
        session.failures = 0;
        session.locked_until = 0;
        Ok(view_from_parts(
            session.data.as_ref().expect("data set"),
            &session.accounts,
            &session.fingerprint,
            &session.formula,
        ))
    }

    pub fn lock_now(&self) {
        let mut session = self.session();
        session.clear();
    }

    pub fn touch(&self) {
        let mut session = self.session();
        if session.is_unlocked() {
            session.last_active = now();
        }
    }

    /// True when the idle timeout elapsed for the unlocked vault.
    pub fn auto_lock_due(&self) -> bool {
        let session = self.session();
        if !session.is_unlocked() {
            return false;
        }
        let minutes = session
            .data
            .as_ref()
            .map(|data| data.settings.auto_lock_minutes)
            .unwrap_or(0);
        if minutes == 0 {
            return false;
        }
        now() - session.last_active >= i64::from(minutes) * 60
    }

    pub fn wallet(&self) -> CoreResult<WalletView> {
        let session = self.session();
        let data = session.data.as_ref().ok_or(Error::Locked)?;
        Ok(view_from_parts(
            data,
            &session.accounts,
            &session.fingerprint,
            &session.formula,
        ))
    }

    pub fn data(&self) -> CoreResult<VaultData> {
        let session = self.session();
        session.data.clone().ok_or(Error::Locked)
    }

    pub fn accounts(&self) -> CoreResult<Vec<Account>> {
        let session = self.session();
        if !session.is_unlocked() {
            return Err(Error::Locked);
        }
        Ok(session.accounts.clone())
    }

    pub fn account_for(&self, chain: ChainId) -> CoreResult<Account> {
        self.accounts()?
            .into_iter()
            .find(|account| account.chain == chain)
            .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))
    }

    /// Mutate the decrypted vault and persist it atomically.
    pub fn mutate(&self, edit: impl FnOnce(&mut VaultData)) -> CoreResult<()> {
        let mut session = self.session();
        let key = session.key.clone().ok_or(Error::Locked)?;
        let mut file = VaultFile::read(&self.vault_path())?;
        let mut data = session.data.clone().ok_or(Error::Locked)?;
        edit(&mut data);
        file.rewrap(&data, &key)?;
        file.write(&self.vault_path())?;
        session.vault = Some(file);
        session.data = Some(data);
        session.last_active = now();
        Ok(())
    }

    pub fn add_activity(&self, item: lycoris_core::models::ActivityItem) -> CoreResult<()> {
        self.mutate(|data| {
            data.activity.insert(0, item);
            data.activity.truncate(500);
        })
    }

    pub fn change_password(&self, current: &str, new_password: &str) -> CoreResult<()> {
        if !self.verify_password(current)? {
            return Err(Error::IncorrectPassword);
        }
        let mut session = self.session();
        let mut data = session.data.clone().ok_or(Error::Locked)?;
        let path = self.vault_path();
        let mut file = VaultFile::read(&path)?;
        // `change_password` re-derives a fresh Argon2 salt and key.
        file.change_password(&data, new_password)?;
        file.write(&path)?;
        let (unlocked, key) = file.unlock(new_password)?;
        data = unlocked;
        session.vault = Some(file);
        session.key = Some(key);
        session.data = Some(data);
        session.last_active = now();
        Ok(())
    }

    pub fn save_settings(&self, settings: Settings) -> CoreResult<Settings> {
        self.mutate(|data| data.settings = settings.clone())?;
        Ok(settings)
    }

    pub fn upsert_contact(&self, contact: Contact) -> CoreResult<()> {
        self.mutate(|data| match data.contacts.iter().position(|c| c.id == contact.id) {
            Some(index) => data.contacts[index] = contact,
            None => data.contacts.push(contact),
        })
    }

    pub fn delete_contact(&self, id: &str) -> CoreResult<()> {
        self.mutate(|data| data.contacts.retain(|contact| contact.id != id))
    }
}

/// Build the view model handed to the UI. Contains no secret material.
pub fn view_from_parts(
    data: &VaultData,
    accounts: &[Account],
    fingerprint: &str,
    formula: &str,
) -> WalletView {
    WalletView {
        mode: data.secret.mode(),
        derivation: data.secret.derivation_mode(),
        label: data.label.clone(),
        fingerprint: fingerprint.to_string(),
        created_at: data.created_at,
        formula: formula.to_string(),
        accounts: accounts.to_vec(),
        settings: data.settings.clone(),
        contacts: data.contacts.clone(),
        activity: data.activity.clone(),
    }
}

pub fn default_label(secret: &WalletSecret) -> String {
    match secret.mode() {
        lycoris_core::derive::WalletMode::Mnemonic => "助记词钱包".to_string(),
        lycoris_core::derive::WalletMode::Passphrase => "口令钱包".to_string(),
    }
}

/// Generate a fresh mnemonic from the OS entropy source.
pub fn fresh_mnemonic(word_count: usize) -> CoreResult<String> {
    lycoris_core::derive::generate_mnemonic(word_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lycoris_core::derive::DerivationMode;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("lycoris-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn session_lifecycle_and_persistence() {
        let dir = temp_dir("lifecycle");
        let state = AppState::new(&dir);

        assert!(!state.status().exists);
        assert!(state.status().locked);
        assert!(state.wallet().is_err());

        let secret = WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
            .unwrap();
        let view = state
            .create(secret.clone(), "correct horse battery", Some("花园".into()), None)
            .unwrap();
        assert_eq!(view.accounts.len(), ChainId::ALL.len());
        assert!(state.status().exists);
        assert!(!state.status().locked);
        assert_eq!(state.status().hint.as_deref(), Some("花园"));

        // A second create must not clobber the existing vault.
        assert!(state.create(secret.clone(), "correct horse battery", None, None).is_err());

        state.lock_now();
        assert!(state.status().locked);
        assert!(state.data().is_err());

        assert!(state.unlock("nope nope nope").is_err());
        let reopened = state.unlock("correct horse battery").unwrap();
        assert_eq!(reopened.fingerprint, view.fingerprint);

        state.upsert_contact(Contact {
            id: "c1".into(),
            name: "Alice".into(),
            chain: ChainId::Bitcoin,
            address: "bc1qexample".into(),
            note: String::new(),
            created_at: now(),
        })
        .unwrap();
        assert_eq!(state.wallet().unwrap().contacts.len(), 1);
        state.delete_contact("c1").unwrap();
        assert!(state.wallet().unwrap().contacts.is_empty());

        // The mutation must be visible after a fresh unlock (it was persisted).
        state.lock_now();
        let again = state.unlock("correct horse battery").unwrap();
        assert!(again.contacts.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_attempts_trigger_lockout() {
        let dir = temp_dir("lockout");
        let state = AppState::new(&dir);
        let secret = WalletSecret::new_passphrase("another quiet garden", DerivationMode::Isolated)
            .unwrap();
        state
            .create(secret, "correct horse battery", None, None)
            .unwrap();
        state.lock_now();
        for _ in 0..MAX_FAILURES {
            let _ = state.unlock("wrong password");
        }
        match state.unlock("correct horse battery") {
            Err(Error::TooManyAttempts(_)) => {}
            other => panic!("expected lockout, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
