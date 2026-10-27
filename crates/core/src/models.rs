use crate::chains::ChainId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FiatCurrency {
    #[default]
    Usd,
    Cny,
}

impl FiatCurrency {
    pub fn code(self) -> &'static str {
        match self {
            FiatCurrency::Usd => "usd",
            FiatCurrency::Cny => "cny",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub fiat: FiatCurrency,
    pub theme: Theme,
    pub hide_balances: bool,
    pub auto_lock_minutes: u32,
    pub price_service: bool,
    /// UI 展示的链（链开关）。空 = 全部显示；派生始终覆盖全部链。
    #[serde(default)]
    pub visible_chains: Vec<ChainId>,
    /// Optional Blockchair API key. Blockchair asks for one nowadays; without
    /// it the Dogecoin and Bitcoin Cash endpoints refuse to answer.
    #[serde(default)]
    pub blockchair_api_key: String,
    /// Local `monero-wallet-rpc` endpoint. Monero spends need CLSAG ring
    /// signatures and Bulletproofs+ range proofs, so the app delegates signing
    /// to the official Monero tool and only keeps the keys here.
    #[serde(default = "default_monero_rpc_url")]
    pub monero_rpc_url: String,
    #[serde(default)]
    pub monero_rpc_user: String,
    #[serde(default)]
    pub monero_rpc_password: String,
    /// Local `zephyr-wallet-rpc` endpoint. Zephyr is a CryptoNote fork with
    /// its own daemon/wallet stack (default port 19797), so it needs a
    /// separate setting even though the wallet-rpc protocol is identical.
    #[serde(default = "default_zephyr_rpc_url")]
    pub zephyr_rpc_url: String,
    /// Block height the wallet scans from when `monero-wallet-rpc` creates the
    /// wallet file. 0 scans the whole chain (slow); the height at which the
    /// wallet was created is the useful value.
    #[serde(default)]
    pub monero_restore_height: u64,
}

pub fn default_monero_rpc_url() -> String {
    "http://127.0.0.1:18083/json_rpc".to_string()
}

/// zephyr-wallet-rpc's default JSON-RPC port is 19797.
pub fn default_zephyr_rpc_url() -> String {
    "http://127.0.0.1:19797/json_rpc".to_string()
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fiat: FiatCurrency::Usd,
            theme: Theme::System,
            hide_balances: false,
            auto_lock_minutes: 5,
            price_service: true,
            visible_chains: Vec::new(),
            blockchair_api_key: String::new(),
            monero_rpc_url: default_monero_rpc_url(),
            monero_rpc_user: String::new(),
            monero_rpc_password: String::new(),
            zephyr_rpc_url: default_zephyr_rpc_url(),
            monero_restore_height: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contact {
    pub id: String,
    pub name: String,
    pub chain: ChainId,
    pub address: String,
    #[serde(default)]
    pub note: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Send,
    Receive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActivityStatus {
    Pending,
    Confirmed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityItem {
    pub id: String,
    pub chain: ChainId,
    pub direction: Direction,
    pub amount: String,
    pub symbol: String,
    pub counterparty: String,
    pub txid: String,
    pub status: ActivityStatus,
    #[serde(default)]
    pub fee: String,
    pub timestamp: i64,
    pub explorer_url: String,
}

/// One entry in the wallet list. Contains no secret material, so the UI can
/// show every wallet without touching the others' key material.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletSummary {
    pub id: String,
    pub label: String,
    pub mode: crate::derive::WalletMode,
    pub derivation: Option<crate::derive::DerivationMode>,
    pub fingerprint: String,
    pub created_at: i64,
    pub activity_count: usize,
    pub active: bool,
}

/// A derived account as seen by the UI (recipe + public values only).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivedAccountView {
    pub id: String,
    pub strategy: crate::derive::BatchStrategy,
    /// The single chain this account adds an address on.
    pub chain: ChainId,
    pub index: u32,
    pub label: String,
    pub fingerprint: String,
    /// True when this account is the one the wallet currently shows.
    pub active: bool,
}

/// One chain's address across the wallet's accounts (base + every derived
/// account), used by the per-asset detail page.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainAddressView {
    /// `None` = the base derivation.
    pub id: Option<String>,
    pub label: String,
    pub address: String,
    pub path: String,
    /// True when this account is the one the wallet currently shows.
    pub active: bool,
}

/// Full detail for one address of the active wallet: public info plus the
/// private key, which the backend only returns after a password check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDetailView {
    pub chain: ChainId,
    pub chain_name: String,
    pub symbol: String,
    /// `None` = the base derivation.
    pub id: Option<String>,
    pub label: String,
    pub address: String,
    pub address_format: String,
    pub path: String,
    pub derivation: String,
    pub hash_algorithm: String,
    pub public_key: String,
    /// Export format ("WIF" / "Hex (0x…)" / "Base58 keypair" / …).
    pub private_key_format: String,
    pub private_key: String,
    /// Where the key comes from: the secret type ("口令" / "助记词") plus a
    /// redacted preview, so the user can tell *which* phrase produced it.
    pub source_kind: String,
    pub source_preview: String,
    /// Ordered, human-readable steps from the secret to this private key.
    pub steps: Vec<String>,
    /// One-line formula, e.g. `SHA-256("lycoris/v1/bitcoin\0" ‖ 口令 ‖ "\0#1")`.
    pub formula: String,
    pub warning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletView {
    /// Id of the active wallet inside the vault.
    pub id: String,
    pub mode: crate::derive::WalletMode,
    pub derivation: Option<crate::derive::DerivationMode>,
    pub label: String,
    pub fingerprint: String,
    pub created_at: i64,
    pub formula: String,
    /// 私钥钱包的钥匙环快照(只暴露「槽是否已填」,不含钥匙本身);
    /// 其他模式为 `None`,序列化成 `null`。
    #[serde(default)]
    pub key_ring: Option<crate::derive::KeyRingPresence>,
    /// 聚合钱包的钥匙清单(id/链/备注,不含钥匙本体);其他模式为 `None`。
    #[serde(default)]
    pub aggregate_entries: Option<Vec<crate::derive::AggregateEntryView>>,
    pub accounts: Vec<crate::derive::Account>,
    pub settings: Settings,
    pub contacts: Vec<Contact>,
    pub activity: Vec<ActivityItem>,
    /// Every wallet stored in the vault, active one flagged.
    pub wallets: Vec<WalletSummary>,
    /// Derived accounts pinned to the active wallet (base account excluded).
    #[serde(default)]
    pub batch_accounts: Vec<DerivedAccountView>,
    /// `None` = showing the base derivation.
    #[serde(default)]
    pub active_batch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetBalance {
    pub chain: ChainId,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub accent: String,
    pub address: String,
    pub amount: String,
    pub price_usd: Option<f64>,
    pub price_cny: Option<f64>,
    pub value_usd: Option<f64>,
    pub value_cny: Option<f64>,
    pub explorer_url: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Portfolio {
    pub assets: Vec<AssetBalance>,
    pub total_usd: Option<f64>,
    pub total_cny: Option<f64>,
    pub fetched_at: i64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FeeOption {
    pub level: String,
    pub label: String,
    pub fee: String,
    pub fee_fiat: Option<String>,
    pub eta: String,
    pub note: String,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendQuote {
    pub chain: ChainId,
    pub symbol: String,
    pub decimals: u8,
    pub balance: String,
    pub max_amount: String,
    pub amount: String,
    pub to: String,
    pub fee_options: Vec<FeeOption>,
    pub can_send: bool,
    pub warning: Option<String>,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendResult {
    pub chain: ChainId,
    pub symbol: String,
    pub txid: String,
    pub explorer_url: String,
    pub amount: String,
    pub fee: String,
    pub to: String,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressRequest {
    pub chain: ChainId,
    pub address: String,
    pub amount: Option<String>,
    pub label: Option<String>,
}

/// Status returned to the onboarding/unlock screen. It never contains secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub exists: bool,
    pub locked: bool,
    pub has_hint: bool,
    pub hint: Option<String>,
    pub label: Option<String>,
}
