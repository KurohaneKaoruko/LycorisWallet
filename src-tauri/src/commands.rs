//! Tauri commands. Every one returns `ApiResult` so the UI always receives a
//! `{ code, message }` structure on failure.

use crate::error::{ApiError, ApiResult};
use crate::portfolio::{self, QuoteRequest, SendRequest};
use crate::state::{default_label, fresh_mnemonic};
use crate::window;
use lycoris_core::chains::ChainId;
use lycoris_core::derive::{DerivationMode, DerivationPreview, WalletSecret};
use lycoris_core::models::{
    Contact, Portfolio, SendQuote, SendResult, Settings, VaultStatus, WalletView,
};
use lycoris_core::qr;
use lycoris_core::vault::now;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "mode", rename_all = "camelCase")]
pub enum SecretRequest {
    #[serde(rename_all = "camelCase")]
    Mnemonic { mnemonic: String },
    #[serde(rename_all = "camelCase")]
    Passphrase {
        phrase: String,
        #[serde(default)]
        derivation: DerivationMode,
    },
}

impl SecretRequest {
    fn into_secret(self) -> ApiResult<WalletSecret> {
        match self {
            SecretRequest::Mnemonic { mnemonic } => {
                Ok(WalletSecret::new_mnemonic(mnemonic.trim())?)
            }
            SecretRequest::Passphrase { phrase, derivation } => {
                Ok(WalletSecret::new_passphrase(phrase.trim(), derivation)?)
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateVaultRequest {
    pub password: String,
    #[serde(default)]
    pub hint: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    pub secret: SecretRequest,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedKey {
    pub chain: ChainId,
    pub format: String,
    pub value: String,
    pub warning: String,
}

#[tauri::command]
pub fn vault_status(state: State<'_, crate::state::AppState>) -> VaultStatus {
    state.status()
}

#[tauri::command]
pub fn create_vault(
    state: State<'_, crate::state::AppState>,
    request: CreateVaultRequest,
) -> ApiResult<WalletView> {
    let secret = request.secret.into_secret()?;
    let label = request.label.clone().unwrap_or_else(|| default_label(&secret));
    Ok(state.create(secret, &request.password, request.hint.clone(), Some(label))?)
}

#[tauri::command]
pub fn unlock_vault(
    state: State<'_, crate::state::AppState>,
    password: String,
) -> ApiResult<WalletView> {
    Ok(state.unlock(&password)?)
}

#[tauri::command]
pub fn lock_vault(state: State<'_, crate::state::AppState>) -> ApiResult<()> {
    state.lock_now();
    Ok(())
}

#[tauri::command]
pub fn wallet(state: State<'_, crate::state::AppState>) -> ApiResult<WalletView> {
    Ok(state.wallet()?)
}

#[tauri::command]
pub fn generate_mnemonic(word_count: Option<usize>) -> ApiResult<String> {
    Ok(fresh_mnemonic(word_count.unwrap_or(12))?)
}

/// Live preview for onboarding. Runs entirely in Rust so the seeds never touch
/// JavaScript.
#[tauri::command]
pub fn preview_derivation(secret: SecretRequest) -> ApiResult<DerivationPreview> {
    let secret = secret.into_secret()?;
    Ok(lycoris_core::derive::preview_derivation(&secret)?)
}

#[tauri::command]
pub fn reveal_mnemonic(
    state: State<'_, crate::state::AppState>,
    password: String,
) -> ApiResult<String> {
    if !state.verify_password(&password)? {
        return Err(ApiError::from(lycoris_core::Error::IncorrectPassword));
    }
    let data = state.data()?;
    let mnemonic = data.secret.mnemonic()?;
    Ok(mnemonic.to_string())
}

#[tauri::command]
pub fn export_private_key(
    state: State<'_, crate::state::AppState>,
    chain: ChainId,
    password: String,
) -> ApiResult<ExportedKey> {
    if !state.verify_password(&password)? {
        return Err(ApiError::from(lycoris_core::Error::IncorrectPassword));
    }
    let data = state.data()?;
    let material = lycoris_core::derive::derive_private_key(&data.secret, chain)?;
    let value = material.export()?;
    let format = match chain {
        ChainId::Bitcoin | ChainId::Litecoin => "WIF".to_string(),
        ChainId::Solana => "Base58 keypair".to_string(),
        _ => "Hex (0x…)".to_string(),
    };
    Ok(ExportedKey {
        chain,
        format,
        value,
        warning: "任何持有该私钥的人都能控制此链上的全部资产。请勿截图、勿粘贴到网页，用完请清理剪贴板。"
            .to_string(),
    })
}

#[tauri::command]
pub fn change_password(
    state: State<'_, crate::state::AppState>,
    current: String,
    new_password: String,
) -> ApiResult<()> {
    state.change_password(&current, &new_password)?;
    Ok(())
}

#[tauri::command]
pub fn rename_wallet(
    state: State<'_, crate::state::AppState>,
    label: String,
) -> ApiResult<WalletView> {
    let trimmed = label.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::new("INVALID_LABEL", "钱包名称不能为空"));
    }
    state.mutate(|data| data.label = trimmed)?;
    Ok(state.wallet()?)
}

#[tauri::command]
pub async fn refresh_portfolio(state: State<'_, crate::state::AppState>) -> ApiResult<Portfolio> {
    portfolio::refresh(&state).await
}

#[tauri::command]
pub async fn quote_send(
    state: State<'_, crate::state::AppState>,
    request: QuoteRequest,
) -> ApiResult<SendQuote> {
    portfolio::quote(&state, &request).await
}

#[tauri::command]
pub async fn send_transaction(
    state: State<'_, crate::state::AppState>,
    request: SendRequest,
) -> ApiResult<SendResult> {
    portfolio::send(&state, &request).await
}

#[tauri::command]
pub fn save_contact(
    state: State<'_, crate::state::AppState>,
    contact: ContactDraft,
) -> ApiResult<Vec<Contact>> {
    let name = contact.name.trim();
    if name.is_empty() {
        return Err(ApiError::new("INVALID_CONTACT", "联系人名称不能为空"));
    }
    let address = contact.address.trim();
    if address.is_empty() {
        return Err(ApiError::from(lycoris_core::Error::InvalidAddress(
            "地址不能为空".into(),
        )));
    }
    lycoris_core::address::validate_address(contact.chain, address)?;
    let existing = state.data()?.contacts;
    let id = contact
        .id
        .clone()
        .filter(|value| !value.is_empty())
        .or_else(|| {
            existing
                .iter()
                .find(|item| item.chain == contact.chain && item.address == address)
                .map(|item| item.id.clone())
        })
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let created_at = existing
        .iter()
        .find(|item| item.id == id)
        .map(|item| item.created_at)
        .unwrap_or_else(now);
    state.upsert_contact(Contact {
        id,
        name: name.to_string(),
        chain: contact.chain,
        address: address.to_string(),
        note: contact.note.trim().to_string(),
        created_at,
    })?;
    Ok(state.data()?.contacts)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContactDraft {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub chain: ChainId,
    pub address: String,
    #[serde(default)]
    pub note: String,
}

#[tauri::command]
pub fn delete_contact(
    state: State<'_, crate::state::AppState>,
    id: String,
) -> ApiResult<Vec<Contact>> {
    state.delete_contact(&id)?;
    Ok(state.data()?.contacts)
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, crate::state::AppState>,
    settings: Settings,
) -> ApiResult<Settings> {
    Ok(state.save_settings(settings)?)
}

#[tauri::command]
pub fn qr_svg(text: String, dark: Option<String>, light: Option<String>) -> ApiResult<String> {
    Ok(qr::svg(
        &text,
        dark.as_deref().unwrap_or("#1b1a19"),
        light.as_deref().unwrap_or("#f7f1e8"),
    )?)
}

/// Open an explorer link in the system browser. Only `http(s)` URLs are
/// allowed, everything else is rejected before it reaches the OS.
#[tauri::command]
pub fn open_external(app: tauri::AppHandle, url: String) -> ApiResult<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(ApiError::new("INVALID_URL", "只能打开 http(s) 链接"));
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| ApiError::new("OPEN_URL", error.to_string()))
}

/// Erase the vault file and wipe the session. Guarded by the password so a
/// stray click cannot destroy funds.
#[tauri::command]
pub fn delete_vault(
    state: State<'_, crate::state::AppState>,
    password: String,
) -> ApiResult<()> {
    if state.status().exists && !state.verify_password(&password)? {
        return Err(ApiError::from(lycoris_core::Error::IncorrectPassword));
    }
    let path = state.vault_path();
    state.lock_now();
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    Ok(())
}

/// Called by the frontend on any user interaction so the idle timer resets.
#[tauri::command]
pub fn touch_session(state: State<'_, crate::state::AppState>) {
    state.touch();
}

#[tauri::command]
pub fn window_minimize(window: tauri::Window) -> ApiResult<()> {
    window::minimize(&window)
}

#[tauri::command]
pub fn window_toggle_maximize(window: tauri::Window) -> ApiResult<bool> {
    window::toggle_maximize(&window)
}

#[tauri::command]
pub fn window_close(window: tauri::Window) -> ApiResult<()> {
    window::close(&window)
}
