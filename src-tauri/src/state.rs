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
use lycoris_core::models::{
    AccountDetailView, ChainAddressView, Contact, Settings, VaultStatus, WalletSummary, WalletView,
};
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
    /// Derive the account set the wallet currently shows: either the base
    /// derivation (index-0 plain paths) or the selected derived account.
    fn for_data(data: &VaultData) -> CoreResult<Self> {
        let wallet = data.active()?;
        let selected = wallet
            .active_batch
            .as_deref()
            .and_then(|id| wallet.batch_accounts.iter().find(|s| s.id == id))
            .cloned();
        match selected {
            None => {
                let preview = preview_derivation(&wallet.secret)?;
                Ok(Self {
                    id: wallet.id.clone(),
                    accounts: preview.accounts,
                    fingerprint: preview.fingerprint,
                    formula: preview.formula,
                })
            }
            Some(spec) => {
                let accounts =
                    lycoris_core::derive::derive_accounts_for_spec(&wallet.secret, &spec)?;
                let fingerprint = if spec.fingerprint.is_empty() {
                    lycoris_core::derive::derive_fingerprint(&accounts)
                } else {
                    spec.fingerprint.clone()
                };
                Ok(Self {
                    id: wallet.id.clone(),
                    accounts,
                    fingerprint,
                    formula: lycoris_core::derive::spec_formula(&spec, wallet.secret.mode()),
                })
            }
        }
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

/// Redact a secret for display: keep the first/last two characters so the
/// user can recognise which phrase it is without exposing it.
fn redact(phrase: &str) -> String {
    phrase.trim().to_string()
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
        self.session
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
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

    /// Delete the wallet file and drop the session.
    ///
    /// This is the escape hatch for a forgotten password: the file is
    /// encrypted with a key that only the password can derive, so without it
    /// the file is unreadable anyway. The caller must make the consequences
    /// explicit to the user before calling this.
    pub fn reset_vault(&self) -> CoreResult<()> {
        let path = self.vault_path();
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        let mut session = self.session();
        session.vault = None;
        session.key = None;
        session.data = None;
        session.active = None;
        session.summaries.clear();
        session.failures = 0;
        session.locked_until = 0;
        session.last_active = now();
        Ok(())
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

    /// The user's settings, including the optional Blockchair API key.
    pub fn settings(&self) -> CoreResult<Settings> {
        Ok(self.data()?.settings)
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

    /// One chain's addresses across the wallet's accounts: the base
    /// derivation first, then every pinned derived account. Cheap even for
    /// large batches because the chain derivation shares the seed expansion.
    pub fn chain_addresses(&self, chain: ChainId) -> CoreResult<Vec<ChainAddressView>> {
        let session = self.session();
        let data = session.data.as_ref().ok_or(Error::Locked)?;
        let wallet = data.active()?;
        let mut out = Vec::with_capacity(1 + wallet.batch_accounts.len());

        let base = preview_derivation(&wallet.secret)?;
        let account = base
            .accounts
            .iter()
            .find(|a| a.chain == chain)
            .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
        out.push(ChainAddressView {
            id: None,
            label: "基础账户".into(),
            address: account.address.clone(),
            path: account.path.clone(),
            active: wallet.active_batch.is_none(),
        });

        if !wallet.batch_accounts.is_empty() {
            let specs = &wallet.batch_accounts;
            let mut groups: std::collections::BTreeMap<
                lycoris_core::derive::BatchStrategy,
                Vec<u32>,
            > = std::collections::BTreeMap::new();
            for spec in specs {
                groups.entry(spec.strategy).or_default().push(spec.index);
            }
            let mut derived: std::collections::HashMap<String, (String, String)> =
                std::collections::HashMap::new();
            for (strategy, indices) in groups {
                let accounts = lycoris_core::derive::derive_chain_accounts_batch(
                    &wallet.secret,
                    strategy,
                    &indices,
                    chain,
                )?;
                let ids: Vec<&str> = specs
                    .iter()
                    .filter(|s| s.strategy == strategy)
                    .map(|s| s.id.as_str())
                    .collect();
                for (id, account) in ids.into_iter().zip(accounts) {
                    derived.insert(id.to_string(), (account.address, account.path));
                }
            }
            for spec in specs {
                let (address, path) = derived
                    .get(&spec.id)
                    .cloned()
                    .unwrap_or_else(|| (String::new(), String::new()));
                out.push(ChainAddressView {
                    id: Some(spec.id.clone()),
                    label: spec.display_label(),
                    address,
                    path,
                    active: wallet.active_batch.as_deref() == Some(spec.id.as_str()),
                });
            }
        }
        Ok(out)
    }

    /// Full detail for one address of the active wallet: public derivation
    /// info plus the private key. `id` selects the derived account (`None` =
    /// base account); the password gate lives in the command layer.
    pub fn account_detail(
        &self,
        chain: ChainId,
        id: Option<&str>,
    ) -> CoreResult<AccountDetailView> {
        let session = self.session();
        let data = session.data.as_ref().ok_or(Error::Locked)?;
        let wallet = data.active()?;

        let spec = id.and_then(|id| {
            wallet
                .batch_accounts
                .iter()
                .find(|spec| spec.id == id)
                .cloned()
        });
        if id.is_some() && spec.is_none() {
            return Err(Error::BatchAccountNotFound);
        }
        let label = match &spec {
            Some(spec) => spec.display_label(),
            None => "基础账户".to_string(),
        };

        // Public info comes from the same derivation the address list uses.
        let account = if let Some(spec) = &spec {
            lycoris_core::derive::derive_chain_accounts_batch(
                &wallet.secret,
                spec.strategy,
                &[spec.index],
                chain,
            )?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Message("派生结果为空".into()))?
        } else {
            preview_derivation(&wallet.secret)?
                .accounts
                .into_iter()
                .find(|account| account.chain == chain)
                .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?
        };

        let material = lycoris_core::derive::derive_chain_private_key_for_spec(
            &wallet.secret,
            spec.as_ref(),
            chain,
        )?;
        let private_key = material.export()?;
        let private_key_format = match chain {
            ChainId::Bitcoin | ChainId::Litecoin => "WIF".to_string(),
            ChainId::Solana => "Base58 keypair".to_string(),
            ChainId::Monero => "Hex (spend‖view)".to_string(),
            _ => "Hex (0x…)".to_string(),
        };

        // 来源与计算步骤：让用户能说清这把私钥「从哪来、怎么算」。
        let isolated = wallet.secret.derivation_mode()
            == Some(lycoris_core::derive::DerivationMode::Isolated);
        let domain = format!("lycoris/v1/{}", chain.as_str());
        let hash_name = match chain {
            ChainId::Litecoin | ChainId::Dogecoin => "scrypt",
            ChainId::Monero => "Keccak-256",
            _ => "SHA-256",
        };
        let (source_kind, source_preview, steps, formula) = match (&wallet.secret, &spec) {
            (
                lycoris_core::derive::WalletSecret::Passphrase { phrase, .. },
                Some(spec),
            ) => {
                let preview = redact(phrase);
                match spec.strategy {
                    lycoris_core::derive::BatchStrategy::Iterated => (
                        "口令",
                        preview,
                        vec![
                            format!("取口令（NFKD 规范化）：{}", redact(phrase)),
                            format!("计算基础私钥 base = {hash_name}(口令)"),
                            format!("连续哈希 base 自己 {} 次 → digest", spec.index),
                            "digest 即该链 32 字节私钥种子".to_string(),
                        ],
                        format!("{hash_name}^{}(base)", spec.index),
                    ),
                    _ => (
                        "口令",
                        preview,
                        vec![
                            format!("取口令（NFKD 规范化）：{}", redact(phrase)),
                            if isolated {
                                format!("拼消息：\"{domain}\\0\" + 口令")
                            } else {
                                "拼消息：口令原文".to_string()
                            },
                            format!("追加账户序号：消息 + \"\\0#{}\"", spec.index),
                            format!("digest = {hash_name}(消息)"),
                            "digest 即该链 32 字节私钥种子".to_string(),
                        ],
                        if isolated {
                            format!("{hash_name}(\"{domain}\\0\" ‖ 口令 ‖ \"\\0#{i}\")", i = spec.index)
                        } else {
                            format!("{hash_name}(口令 ‖ \"\\0#{i}\")", i = spec.index)
                        },
                    ),
                }
            }
            (lycoris_core::derive::WalletSecret::Passphrase { phrase, .. }, None) => (
                "口令",
                redact(phrase),
                vec![
                    format!("取口令（NFKD 规范化）：{}", redact(phrase)),
                    if isolated {
                        format!("拼消息：\"{domain}\\0\" + 口令")
                    } else {
                        "拼消息：口令原文".to_string()
                    },
                    format!("digest = {hash_name}(消息)"),
                    "digest 即该链 32 字节私钥种子".to_string(),
                ],
                if isolated {
                    format!("{hash_name}(\"{domain}\\0\" ‖ 口令)")
                } else {
                    format!("{hash_name}(口令)")
                },
            ),
            (lycoris_core::derive::WalletSecret::Mnemonic { .. }, Some(spec)) => {
                match spec.strategy {
                    lycoris_core::derive::BatchStrategy::Path => (
                        "助记词",
                        "BIP-39 助记词（见 设置 → 导出助记词）".to_string(),
                        vec![
                            "助记词 → PBKDF2-HMAC-SHA512 → 512 位种子".to_string(),
                            format!("沿路径 {} 做BIP-32/SLIP-0010 子钥派生", account.path),
                            "派生出的私钥字节即该链私钥".to_string(),
                        ],
                        format!("BIP-32({})", account.path),
                    ),
                    lycoris_core::derive::BatchStrategy::Salted => (
                        "助记词",
                        "BIP-39 助记词（见 设置 → 导出助记词）".to_string(),
                        vec![
                            "取助记词原文（12/24 个单词）".to_string(),
                            format!("拼消息：\"{domain}\\0\" + 助记词 + \"\\0#{i}\"", i = spec.index),
                            format!("digest = {hash_name}(消息)"),
                            "digest 即该链 32 字节私钥种子（离开标准 BIP-32 树）".to_string(),
                        ],
                        format!("{hash_name}(\"{domain}\\0\" ‖ 助记词 ‖ \"\\0#{i}\")", i = spec.index),
                    ),
                    lycoris_core::derive::BatchStrategy::Iterated => (
                        "助记词",
                        "BIP-39 助记词（见 设置 → 导出助记词）".to_string(),
                        vec![
                            "助记词 → PBKDF2-HMAC-SHA512 → 512 位种子".to_string(),
                            format!("沿基础路径 {} 派生基础私钥 base", {
                                let info = chain.info();
                                info.derivation_path
                            }),
                            format!("连续哈希 base 自己 {} 次 → digest", spec.index),
                            "digest 即该链 32 字节私钥种子".to_string(),
                        ],
                        format!("{hash_name}^{}(base)", spec.index),
                    ),
                }
            }
            (lycoris_core::derive::WalletSecret::Mnemonic { .. }, None) => (
                "助记词",
                "BIP-39 助记词（见 设置 → 导出助记词）".to_string(),
                vec![
                    "助记词 → PBKDF2-HMAC-SHA512 → 512 位种子".to_string(),
                    format!("沿路径 {} 做BIP-32/SLIP-0010 子钥派生", account.path),
                    "派生出的私钥字节即该链私钥".to_string(),
                ],
                format!("BIP-32({})", account.path),
            ),
        };

        Ok(AccountDetailView {
            chain,
            chain_name: account.name,
            symbol: account.symbol,
            id: spec.map(|spec| spec.id),
            label,
            address: account.address,
            address_format: account.address_format,
            path: account.path,
            derivation: account.derivation,
            hash_algorithm: account.hash_algorithm,
            public_key: account.public_key,
            private_key_format,
            private_key,
            source_kind: source_kind.to_string(),
            source_preview,
            steps,
            formula,
            warning:
                "任何持有该私钥的人都能控制此地址的全部资产。请勿截图、勿粘贴到网页，用完请清理剪贴板。"
                    .to_string(),
        })
    }

    /// Every account of the active wallet: the base derivation plus each
    /// pinned derived account. This is what the portfolio aggregates so the
    /// dashboard total covers all derived addresses.
    pub fn all_accounts(&self) -> CoreResult<Vec<(Option<String>, Account)>> {
        let session = self.session();
        let data = session.data.as_ref().ok_or(Error::Locked)?;
        let wallet = data.active()?;

        let mut groups: std::collections::BTreeMap<
            lycoris_core::derive::BatchStrategy,
            Vec<u32>,
        > = std::collections::BTreeMap::new();
        for spec in &wallet.batch_accounts {
            groups
                .entry(spec.strategy)
                .or_default()
                .push(spec.index);
        }

        let mut out = Vec::with_capacity((1 + wallet.batch_accounts.len()) * ChainId::ALL.len());
        // Base account (the plain derivation) first.
        for account in preview_derivation(&wallet.secret)?.accounts {
            out.push((None, account));
        }
        for (strategy, indices) in groups {
            for chain in ChainId::ALL {
                let accounts = lycoris_core::derive::derive_chain_accounts_batch(
                    &wallet.secret,
                    strategy,
                    &indices,
                    chain,
                )?;
                let ids: Vec<&str> = wallet
                    .batch_accounts
                    .iter()
                    .filter(|s| s.strategy == strategy)
                    .map(|s| s.id.as_str())
                    .collect();
                for (id, account) in ids.iter().zip(accounts) {
                    out.push((Some((*id).to_string()), account));
                }
            }
        }
        Ok(out)
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

    /// Derive `request.count` accounts with `request.strategy` and pin them to
    /// the **active wallet** — the wallet keeps one secret and gains a list of
    /// derived accounts it can switch between. Idempotent per account id.
    /// The last added account becomes the selected one.
    pub fn add_batch_accounts(
        &self,
        request: &lycoris_core::derive::BatchRequest,
        base_label: Option<String>,
    ) -> CoreResult<WalletView> {
        // Validate against the active secret first (derive also enforces
        // bounds, and fingerprints get filled in below).
        let (secret, existing) = {
            let session = self.session();
            let data = session.data.as_ref().ok_or(Error::Locked)?;
            let wallet = data.active()?;
            (wallet.secret.clone(), wallet.batch_accounts.clone())
        };
        let batch = lycoris_core::derive::derive_accounts_batch(&secret, request)?;
        if batch.is_empty() {
            return Err(Error::Message("批量派生结果为空".into()));
        }
        let base_label = base_label
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let start = request.start;
        let specs: Vec<lycoris_core::derive::BatchAccountSpec> = batch
            .iter()
            .enumerate()
            .map(|(offset, accounts)| {
                let index = start + offset as u32;
                let mut spec = lycoris_core::derive::BatchAccountSpec::new(request.strategy, index);
                spec.fingerprint = lycoris_core::derive::derive_fingerprint(accounts);
                // Label: 用户命名 > 「钱包名 #序号」(与钱包列表可区分)
                spec.label = base_label
                    .clone()
                    .map(|label| format!("{label} #{index}"));
                let _ = &existing;
                spec
            })
            .collect();
        self.rebuild(move |data| {
            data.add_batch_accounts(specs)?;
            Ok(())
        })?;
        self.wallet()
    }

    /// Select one of the wallet's derived accounts (`id` from
    /// [`WalletView::batch_accounts`]); `None` returns to the base account.
    pub fn select_batch_account(&self, id: Option<String>) -> CoreResult<WalletView> {
        self.rebuild(move |data| match id.as_deref() {
            Some(id) => data.select_batch_account(id).map(|_| ()),
            None => {
                let wallet = data.active_mut()?;
                wallet.active_batch = None;
                Ok(())
            }
        })?;
        self.wallet()
    }

    pub fn rename_batch_account(&self, id: &str, label: &str) -> CoreResult<WalletView> {
        let trimmed = label.trim().to_string();
        if trimmed.is_empty() {
            return Err(Error::Message("账户名称不能为空".into()));
        }
        self.rebuild(|data| data.rename_batch_account(id, &trimmed).map(|_| ()))?;
        self.wallet()
    }

    pub fn remove_batch_account(&self, id: &str) -> CoreResult<WalletView> {
        self.rebuild(|data| data.remove_batch_account(id))?;
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
    pub fn mutate(&self, edit: impl FnOnce(&mut VaultData) -> CoreResult<()>) -> CoreResult<()> {
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
    let wallet = data.find(&active.id).ok_or(Error::WalletNotFound)?;
    let batch_accounts: Vec<lycoris_core::models::DerivedAccountView> = wallet
        .batch_accounts
        .iter()
        .map(|spec| lycoris_core::models::DerivedAccountView {
            id: spec.id.clone(),
            strategy: spec.strategy,
            index: spec.index,
            label: spec.display_label(),
            // Older vaults stored lowercase fingerprints; normalise on read.
            fingerprint: spec.fingerprint.to_uppercase(),
            active: wallet.active_batch.as_deref() == Some(spec.id.as_str()),
        })
        .collect();
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
        batch_accounts,
        active_batch: wallet.active_batch.clone(),
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

        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let view = state
            .create(
                secret.clone(),
                "correct horse battery",
                Some("花园".into()),
                None,
            )
            .unwrap();
        assert_eq!(view.accounts.len(), ChainId::ALL.len());
        assert_eq!(view.wallets.len(), 1);
        assert!(view.wallets[0].active);
        assert!(state.status().exists);
        assert!(!state.status().locked);
        assert_eq!(state.status().hint.as_deref(), Some("花园"));

        // A second create must not clobber the existing vault.
        assert!(state
            .create(secret.clone(), "correct horse battery", None, None)
            .is_err());

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
    fn reset_vault_removes_the_file_and_the_session() {
        let dir = temp_dir("reset");
        let state = AppState::new(&dir);
        state
            .create(
                passphrase("a quiet garden at dusk"),
                "correct horse battery",
                None,
                None,
            )
            .unwrap();
        assert!(state.status().exists);
        assert!(state.vault_path().exists());

        // The "password lost" escape hatch wipes the file and the session.
        state.reset_vault().unwrap();
        assert!(!state.status().exists);
        assert!(!state.vault_path().exists());
        assert!(state.wallet().is_err());

        // A fresh vault can be created afterwards.
        state
            .create(
                passphrase("a quiet garden at dusk"),
                "correct horse battery",
                None,
                None,
            )
            .unwrap();
        assert!(state.status().exists);
        assert!(!state.status().locked);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn failed_attempts_trigger_lockout() {
        let dir = temp_dir("lockout");
        let state = AppState::new(&dir);
        let secret =
            WalletSecret::new_passphrase("another quiet garden", DerivationMode::Isolated).unwrap();
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
            .create(
                passphrase("a quiet garden at dusk"),
                "correct horse battery",
                None,
                Some("主钱包".into()),
            )
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
        let view = state
            .delete_wallet(&second_id, "correct horse battery")
            .unwrap();
        assert_eq!(view.wallets.len(), 1);
        // The last wallet is protected.
        assert!(state
            .delete_wallet(&first_id, "correct horse battery")
            .is_err());
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
            .create(
                passphrase("a quiet garden at dusk"),
                "correct horse battery",
                None,
                Some("A".into()),
            )
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
