//! Unlocked-session state shared by every command.
//!
//! The plaintext vault only ever lives in this process. The frontend never
//! receives a secret: commands return addresses, balances and transaction
//! hashes, never keys. One vault file can hold several wallets; they share the
//! same password, address book and settings, while transaction history stays
//! per wallet.

use lycoris_core::chains::ChainId;
use lycoris_core::derive::{preview_derivation, Account, WalletMode, WalletSecret};
use lycoris_core::error::{Error, Result as CoreResult};
use lycoris_core::models::{Contact, Settings, VaultStatus, WalletSummary, WalletView};
use lycoris_core::vault::{now, VaultData, VaultFile};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use zeroize::Zeroizing;

const VAULT_FILE: &str = "vault.json";
/// Failed unlock attempts before a forced cool-down.
const MAX_FAILURES: u32 = 5;
const LOCKOUT_SECONDS: i64 = 30;

/// Derived material for the wallet that is currently selected.
struct ActiveWallet {
    id: String,
    accounts: Vec<Account>,
    fingerprint: String,
    formula: String,
}

impl ActiveWallet {
    fn for_data(data: &VaultData) -> CoreResult<Self> {
        let wallet = data.active()?;
        let preview = preview_derivation(&wallet.secret)?;
        Ok(Self {
            id: wallet.id.clone(),
            accounts: preview.accounts,
            fingerprint: preview.fingerprint,
            formula: preview.formula,
        })
    }
}

struct Session {
    vault: Option<VaultFile>,
    key: Option<Zeroizing<[u8; 32]>>,
    data: Option<VaultData>,
    active: Option<ActiveWallet>,
    summaries: Vec<WalletSummary>,
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
            active: None,
            summaries: Vec::new(),
            last_active: now(),
            failures: 0,
            locked_until: 0,
        }
    }

    fn is_unlocked(&self) -> bool {
        self.data.is_some() && self.key.is_some() && self.active.is_some()
    }

    fn clear(&mut self) {
        self.key = None;
        self.data = None;
        self.active = None;
        self.summaries.clear();
    }

    /// Recompute the active wallet preview and the list of wallet summaries.
    fn refresh(&mut self) -> CoreResult<()> {
        let data = self.data.clone().ok_or(Error::Locked)?;
        self.active = Some(ActiveWallet::for_data(&data)?);
        self.summaries = summaries_for(&data)?;
        Ok(())
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
            label: session.data.as_ref().map(|data| data.active_label()),
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
        let mut file = VaultFile::read(&path)?;
        match file.unlock(password) {
            Ok((data, key)) => {
                // Vault format 1 held a single wallet; rewrite it as soon as
                // the password proves the user owns the file.
                if file.needs_upgrade() && file.rewrap(&data, &key).is_ok() {
                    let _ = file.write(&path);
                }
                let mut session = self.session();
                session.failures = 0;
                session.locked_until = 0;
                session.vault = Some(file);
                session.key = Some(key);
                session.data = Some(data);
                session.last_active = now();
                session.refresh()?;
                view_locked(&session)
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
        let mut session = self.session();
        session.vault = Some(file);
        session.key = Some(key);
        session.data = Some(data);
        session.last_active = now();
        session.failures = 0;
        session.locked_until = 0;
        session.refresh()?;
        view_locked(&session)
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
        view_locked(&session)
    }

    pub fn data(&self) -> CoreResult<VaultData> {
        let session = self.session();
        session.data.clone().ok_or(Error::Locked)
    }

    /// Secret of the wallet that is currently selected.
    pub fn active_secret(&self) -> CoreResult<WalletSecret> {
        let session = self.session();
        let data = session.data.as_ref().ok_or(Error::Locked)?;
        Ok(data.active()?.secret.clone())
    }

    pub fn accounts(&self) -> CoreResult<Vec<Account>> {
        let session = self.session();
        session
            .active
            .as_ref()
            .map(|active| active.accounts.clone())
            .ok_or(Error::Locked)
    }

    pub fn account_for(&self, chain: ChainId) -> CoreResult<Account> {
        self.accounts()?
            .into_iter()
            .find(|account| account.chain == chain)
            .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))
    }

    /* ------------------------------------------------------------- wallets */

    /// Add a wallet to the vault and select it.
    pub fn add_wallet(
        &self,
        label: Option<String>,
        secret: WalletSecret,
    ) -> CoreResult<WalletView> {
        let label = label
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| default_label(&secret));
        self.rebuild(move |data| {
            data.add_wallet(label, secret);
            Ok(())
        })?;
        self.wallet()
    }

    pub fn switch_wallet(&self, id: &str) -> CoreResult<WalletView> {
        self.rebuild(|data| {
            if data.find(id).is_none() {
                return Err(Error::WalletNotFound);
            }
            data.active_wallet = id.to_string();
            Ok(())
        })?;
        self.wallet()
    }

    pub fn rename_wallet(&self, id: &str, label: &str) -> CoreResult<WalletView> {
        let trimmed = label.trim().to_string();
        if trimmed.is_empty() {
            return Err(Error::Message("钱包名称不能为空".into()));
        }
        self.rebuild(|data| data.rename_wallet(id, &trimmed))?;
        self.wallet()
    }

    /// Remove a wallet. Requires the vault password because it is destructive.
    pub fn delete_wallet(&self, id: &str, password: &str) -> CoreResult<WalletView> {
        if !self.verify_password(password)? {
            return Err(Error::IncorrectPassword);
        }
        self.rebuild(|data| data.remove_wallet(id))?;
        self.wallet()
    }

    /* ------------------------------------------------------------ mutation */

    /// Mutate the decrypted vault and persist it atomically.
    pub fn mutate(
        &self,
        edit: impl FnOnce(&mut VaultData) -> CoreResult<()>,
    ) -> CoreResult<()> {
        let mut session = self.session();
        let key = session.key.clone().ok_or(Error::Locked)?;
        let mut file = VaultFile::read(&self.vault_path())?;
        let mut data = session.data.clone().ok_or(Error::Locked)?;
        edit(&mut data)?;
        file.rewrap(&data, &key)?;
        file.write(&self.vault_path())?;
        session.vault = Some(file);
        session.data = Some(data);
        session.last_active = now();
        Ok(())
    }

    /// Like `mutate`, but also recomputes the derived session state. Used when
    /// the wallet list (and therefore the active wallet) may have changed.
    fn rebuild(&self, edit: impl FnOnce(&mut VaultData) -> CoreResult<()>) -> CoreResult<()> {
        let mut session = self.session();
        let key = session.key.clone().ok_or(Error::Locked)?;
        let mut file = VaultFile::read(&self.vault_path())?;
        let mut data = session.data.clone().ok_or(Error::Locked)?;
        edit(&mut data)?;
        file.rewrap(&data, &key)?;
        file.write(&self.vault_path())?;
        session.vault = Some(file);
        session.data = Some(data);
        session.last_active = now();
        session.refresh()
    }

    pub fn add_activity(&self, item: lycoris_core::models::ActivityItem) -> CoreResult<()> {
        self.mutate(|data| {
            let wallet = data.active_mut()?;
            wallet.activity.insert(0, item);
            wallet.activity.truncate(500);
            Ok(())
        })?;
        // Keep the wallet list badge in sync without re-deriving every wallet.
        let mut session = self.session();
        let count = session
            .data
            .as_ref()
            .and_then(|data| data.active().ok().map(|wallet| wallet.activity.len()));
        if let (Some(count), Some(active)) = (
            count,
            session.data.as_ref().map(|data| data.active_wallet.clone()),
        ) {
            if let Some(summary) = session.summaries.iter_mut().find(|s| s.id == active) {
                summary.activity_count = count;
            }
        }
        Ok(())
    }

    pub fn change_password(&self, current: &str, new_password: &str) -> CoreResult<()> {
        if !self.verify_password(current)? {
            return Err(Error::IncorrectPassword);
        }
        let mut session = self.session();
        let data = session.data.clone().ok_or(Error::Locked)?;
        let path = self.vault_path();
        let mut file = VaultFile::read(&path)?;
        // `change_password` re-derives a fresh Argon2 salt and key.
        file.change_password(&data, new_password)?;
        file.write(&path)?;
        let (unlocked, key) = file.unlock(new_password)?;
        session.vault = Some(file);
        session.key = Some(key);
        session.data = Some(unlocked);
        session.last_active = now();
        session.refresh()
    }

    pub fn save_settings(&self, settings: Settings) -> CoreResult<Settings> {
        self.mutate(|data| {
            data.settings = settings.clone();
            Ok(())
        })?;
        Ok(settings)
    }

    pub fn upsert_contact(&self, contact: Contact) -> CoreResult<()> {
        self.mutate(|data| {
            match data.contacts.iter().position(|c| c.id == contact.id) {
                Some(index) => data.contacts[index] = contact,
                None => data.contacts.push(contact),
            }
            Ok(())
        })
    }

    pub fn delete_contact(&self, id: &str) -> CoreResult<()> {
        self.mutate(|data| {
            data.contacts.retain(|contact| contact.id != id);
            Ok(())
        })
    }
}

/// Build the view model handed to the UI. Contains no secret material.
fn view_locked(session: &Session) -> CoreResult<WalletView> {
    let data = session.data.as_ref().ok_or(Error::Locked)?;
    let active = session.active.as_ref().ok_or(Error::Locked)?;
    let wallet = data
        .find(&active.id)
        .ok_or(Error::WalletNotFound)?;
    Ok(WalletView {
        id: wallet.id.clone(),
        mode: wallet.secret.mode(),
        derivation: wallet.secret.derivation_mode(),
        label: wallet.label.clone(),
        fingerprint: active.fingerprint.clone(),
        created_at: wallet.created_at,
        formula: active.formula.clone(),
        accounts: active.accounts.clone(),
        settings: data.settings.clone(),
        contacts: data.contacts.clone(),
        activity: wallet.activity.clone(),
        wallets: session.summaries.clone(),
    })
}

fn summaries_for(data: &VaultData) -> CoreResult<Vec<WalletSummary>> {
    let mut summaries = Vec::with_capacity(data.wallets.len());
    for wallet in &data.wallets {
        let preview = preview_derivation(&wallet.secret)?;
        summaries.push(WalletSummary {
            id: wallet.id.clone(),
            label: wallet.label.clone(),
            mode: wallet.secret.mode(),
            derivation: wallet.secret.derivation_mode(),
            fingerprint: preview.fingerprint,
            created_at: wallet.created_at,
            activity_count: wallet.activity.len(),
            active: wallet.id == data.active_wallet,
        });
    }
    Ok(summaries)
}

pub fn default_label(secret: &WalletSecret) -> String {
    match secret.mode() {
        WalletMode::Mnemonic => "助记词钱包".to_string(),
        WalletMode::Passphrase => "口令钱包".to_string(),
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

    fn passphrase(text: &str) -> WalletSecret {
        WalletSecret::new_passphrase(text, DerivationMode::Isolated).unwrap()
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
        assert_eq!(view.wallets.len(), 1);
        assert!(view.wallets[0].active);
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

        state
            .upsert_contact(Contact {
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

    #[test]
    fn multiple_wallets_switch_rename_delete() {
        let dir = temp_dir("multi-wallet");
        let state = AppState::new(&dir);

        let view = state
            .create(passphrase("a quiet garden at dusk"), "correct horse battery", None, Some("主钱包".into()))
            .unwrap();
        let first_id = view.id.clone();

        // Adding a wallet selects it and keeps the first one.
        let view = state
            .add_wallet(None, passphrase("a slow river in winter"))
            .unwrap();
        assert_eq!(view.wallets.len(), 2);
        assert_eq!(view.label, "口令钱包");
        assert_eq!(view.id, view.wallets[1].id);
        assert!(view.wallets[1].active);
        let second_id = view.id.clone();

        // Switching back restores the derived accounts of the first wallet.
        let view = state.switch_wallet(&first_id).unwrap();
        assert_eq!(view.label, "主钱包");
        assert!(view.wallets[0].active);
        assert_eq!(view.fingerprint, state.wallet().unwrap().fingerprint);

        // Renaming works on any wallet, including inactive ones.
        let view = state.rename_wallet(&second_id, "备用").unwrap();
        assert!(view.wallets.iter().any(|wallet| wallet.label == "备用"));

        // Deleting requires the password.
        assert!(state.delete_wallet(&second_id, "wrong password").is_err());
        assert_eq!(state.wallet().unwrap().wallets.len(), 2);
        let view = state.delete_wallet(&second_id, "correct horse battery").unwrap();
        assert_eq!(view.wallets.len(), 1);
        // The last wallet is protected.
        assert!(state.delete_wallet(&first_id, "correct horse battery").is_err());
        assert!(matches!(
            state.delete_wallet("missing", "correct horse battery"),
            Err(Error::WalletNotFound)
        ));

        // Everything survives a lock/unlock round trip.
        state.lock_now();
        let view = state.unlock("correct horse battery").unwrap();
        assert_eq!(view.wallets.len(), 1);
        assert_eq!(view.label, "主钱包");
        assert!(state.status().label.as_deref() == Some("主钱包"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn activity_belongs_to_the_active_wallet() {
        let dir = temp_dir("wallet-activity");
        let state = AppState::new(&dir);
        let view = state
            .create(passphrase("a quiet garden at dusk"), "correct horse battery", None, Some("A".into()))
            .unwrap();
        let first_id = view.id.clone();
        let view = state
            .add_wallet(Some("B".into()), passphrase("a slow river in winter"))
            .unwrap();
        assert_eq!(view.id, view.wallets[1].id);

        let item = |txid: &str| lycoris_core::models::ActivityItem {
            id: txid.into(),
            chain: ChainId::Bitcoin,
            direction: lycoris_core::models::Direction::Send,
            amount: "0.01".into(),
            symbol: "BTC".into(),
            counterparty: "bc1qexample".into(),
            txid: txid.into(),
            status: lycoris_core::models::ActivityStatus::Pending,
            fee: "0.0001".into(),
            timestamp: now(),
            explorer_url: format!("https://mempool.space/tx/{txid}"),
        };
        state.add_activity(item("bbbb")).unwrap();
        state.switch_wallet(&first_id).unwrap();
        state.add_activity(item("aaaa")).unwrap();

        let view = state.wallet().unwrap();
        assert_eq!(view.activity.len(), 1);
        assert_eq!(view.activity[0].txid, "aaaa");
        assert_eq!(view.wallets[0].activity_count, 1);
        assert_eq!(view.wallets[1].activity_count, 1);

        // Switching wallets does not leak the other wallet's history.
        let second_id = view.wallets[1].id.clone();
        let view = state.switch_wallet(&second_id).unwrap();
        assert_eq!(view.activity.len(), 1);
        assert_eq!(view.activity[0].txid, "bbbb");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
