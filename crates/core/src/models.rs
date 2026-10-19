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
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            fiat: FiatCurrency::Usd,
            theme: Theme::System,
            hide_balances: false,
            auto_lock_minutes: 5,
            price_service: true,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletView {
    pub mode: crate::derive::WalletMode,
    pub derivation: Option<crate::derive::DerivationMode>,
    pub label: String,
    pub fingerprint: String,
    pub created_at: i64,
    pub formula: String,
    pub accounts: Vec<crate::derive::Account>,
    pub settings: Settings,
    pub contacts: Vec<Contact>,
    pub activity: Vec<ActivityItem>,
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
