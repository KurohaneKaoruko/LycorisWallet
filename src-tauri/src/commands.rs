//! Tauri commands. Every one returns `ApiResult` so the UI always receives a
//! `{ code, message }` structure on failure.

use crate::error::{ApiError, ApiResult};
use crate::portfolio::{self, QuoteRequest, SendRequest};
use crate::state::{default_label, fresh_mnemonic};
use crate::window;
use lycoris_core::chains::ChainId;
use lycoris_core::derive::KeySlotKind;
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
    #[serde(rename_all = "camelCase")]
    PrivateKey { key: String },
    /// 聚合钱包:第一把私钥(链 + 原始钥匙 + 可选备注)。
    #[serde(rename_all = "camelCase")]
    Aggregate {
        chain: ChainId,
        key: String,
        #[serde(default)]
        label: Option<String>,
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
            SecretRequest::PrivateKey { key } => Ok(WalletSecret::new_private_key(key.trim())?),
            SecretRequest::Aggregate { chain, key, label } => {
                Ok(WalletSecret::new_aggregate(chain, key.trim(), label)?)
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
    let label = request
        .label
        .clone()
        .unwrap_or_else(|| default_label(&secret));
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
    let secret = state.active_secret()?;
    let mnemonic = secret.mnemonic()?;
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
    let secret = state.active_secret()?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
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
        warning:
            "任何持有该私钥的人都能控制此链上的全部资产。请勿截图、勿粘贴到网页，用完请清理剪贴板。"
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddWalletRequest {
    #[serde(default)]
    pub name: Option<String>,
    pub secret: SecretRequest,
}

/// Add another wallet to the unlocked vault and select it.
#[tauri::command]
pub fn add_wallet(
    state: State<'_, crate::state::AppState>,
    request: AddWalletRequest,
) -> ApiResult<WalletView> {
    let secret = request.secret.into_secret()?;
    Ok(state.add_wallet(request.name, secret)?)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchCreateRequest {
    pub batch: lycoris_core::derive::BatchRequest,
    /// Optional label prefix; accounts default to 「钱包名 #序号」.
    #[serde(default)]
    pub name: Option<String>,
}

/// Preview a batch derivation **for the active wallet's secret** without
/// touching the vault. Returns one fingerprint + address list per account.
#[tauri::command]
pub fn preview_batch(
    state: State<'_, crate::state::AppState>,
    batch: lycoris_core::derive::BatchRequest,
) -> ApiResult<Vec<lycoris_core::derive::DerivationPreview>> {
    let secret = state.active_secret()?;
    let accounts = lycoris_core::derive::derive_accounts_batch(&secret, &batch)?;
    Ok(accounts
        .iter()
        .map(|account| {
            let fingerprint =
                lycoris_core::derive::derive_fingerprint(std::slice::from_ref(account));
            lycoris_core::derive::DerivationPreview {
                mode: secret.mode(),
                derivation: secret.derivation_mode(),
                accounts: vec![account.clone()],
                fingerprint,
                formula: String::new(),
            }
        })
        .collect())
}

/// Pin derived accounts onto the active wallet and select the last one.
#[tauri::command]
pub fn add_wallets_batch(
    state: State<'_, crate::state::AppState>,
    request: BatchCreateRequest,
) -> ApiResult<WalletView> {
    Ok(state.add_batch_accounts(&request.batch, request.name)?)
}

/// One chain's addresses across the active wallet's accounts (base + every
/// derived account). Powers the per-asset detail page.
#[tauri::command]
pub fn chain_addresses(
    state: State<'_, crate::state::AppState>,
    chain: ChainId,
) -> ApiResult<Vec<lycoris_core::models::ChainAddressView>> {
    Ok(state.chain_addresses(chain)?)
}

/// Full detail (derivation info + keys) for one address of the active
/// wallet. Requires the vault password like every other key export.
#[tauri::command]
pub fn account_detail(
    state: State<'_, crate::state::AppState>,
    chain: ChainId,
    id: Option<String>,
    password: String,
) -> ApiResult<lycoris_core::models::AccountDetailView> {
    if !state.verify_password(&password)? {
        return Err(ApiError::from(lycoris_core::Error::IncorrectPassword));
    }
    Ok(state.account_detail(chain, id.as_deref())?)
}

/// Switch which derived account the wallet displays (`None` = base account).
#[tauri::command]
pub fn select_batch_account(
    state: State<'_, crate::state::AppState>,
    id: Option<String>,
) -> ApiResult<WalletView> {
    Ok(state.select_batch_account(id)?)
}

/// Rename a derived account of the active wallet.
#[tauri::command]
pub fn rename_batch_account(
    state: State<'_, crate::state::AppState>,
    id: String,
    label: String,
) -> ApiResult<WalletView> {
    Ok(state.rename_batch_account(&id, &label)?)
}

/// Remove a derived account from the active wallet.
#[tauri::command]
pub fn remove_batch_account(
    state: State<'_, crate::state::AppState>,
    id: String,
) -> ApiResult<WalletView> {
    Ok(state.remove_batch_account(&id)?)
}

#[tauri::command]
pub fn switch_wallet(
    state: State<'_, crate::state::AppState>,
    id: String,
) -> ApiResult<WalletView> {
    Ok(state.switch_wallet(&id)?)
}

#[tauri::command]
pub fn rename_wallet(
    state: State<'_, crate::state::AppState>,
    id: String,
    label: String,
) -> ApiResult<WalletView> {
    let trimmed = label.trim().to_string();
    if trimmed.is_empty() {
        return Err(ApiError::new("INVALID_LABEL", "钱包名称不能为空"));
    }
    Ok(state.rename_wallet(&id, &trimmed)?)
}

/// Remove a wallet from the vault. The password guard makes sure a stray click
/// cannot drop a wallet the user still needs.
#[tauri::command]
pub fn delete_wallet(
    state: State<'_, crate::state::AppState>,
    id: String,
    password: String,
) -> ApiResult<WalletView> {
    Ok(state.delete_wallet(&id, &password)?)
}

/// Delete the whole wallet file. Used when the password is lost: the file
/// cannot be decrypted without it, so the only way forward is to start over.
/// `confirm` must be true, because this destroys the only copy of the file.
#[tauri::command]
pub fn reset_vault(state: State<'_, crate::state::AppState>, confirm: bool) -> ApiResult<()> {
    if !confirm {
        return Err(ApiError::new(
            "CONFIRM_REQUIRED",
            "删除钱包文件需要用户明确确认",
        ));
    }
    Ok(state.reset_vault()?)
}

#[tauri::command]
pub async fn refresh_portfolio(state: State<'_, crate::state::AppState>) -> ApiResult<Portfolio> {
    portfolio::refresh(&state).await
}

#[tauri::command]
pub async fn refresh_activity(
    state: State<'_, crate::state::AppState>,
) -> ApiResult<portfolio::ActivitySyncReport> {
    portfolio::refresh_activity(&state).await
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
pub async fn monero_status(
    state: State<'_, crate::state::AppState>,
) -> ApiResult<crate::monero::MoneroStatus> {
    crate::monero::status(&state).await
}

#[tauri::command]
pub fn qr_svg(text: String, dark: Option<String>, light: Option<String>) -> ApiResult<String> {
    Ok(qr::svg(
        &text,
        dark.as_deref().unwrap_or("#1b1a19"),
        light.as_deref().unwrap_or("#f7f1e8"),
    )?)
}

/* -------------------------------------------------------------- 工具箱命令 */

/// 地址体检:识别任意地址属于哪条链 + 校验详情。纯函数,无密钥。
#[tauri::command]
pub fn tool_inspect_address(address: String) -> ApiResult<lycoris_core::tools::AddressReport> {
    Ok(lycoris_core::tools::inspect_address(&address)?)
}

/// 单位换算:显示单位 ↔ 最小单位。
#[tauri::command]
pub fn tool_convert_units(
    chain: ChainId,
    value: String,
    from_smallest: bool,
) -> ApiResult<lycoris_core::tools::UnitConversion> {
    Ok(lycoris_core::tools::convert_units(
        chain,
        &value,
        from_smallest,
    )?)
}

/// 助记词校验(BIP-39 词表 + 校验和)。
#[tauri::command]
pub fn tool_check_mnemonic(mnemonic: String) -> ApiResult<lycoris_core::tools::MnemonicCheck> {
    Ok(lycoris_core::tools::check_mnemonic(&mnemonic)?)
}

/// 哈希计算:sha256 / sha512 / double-sha256 / keccak256 / hash160 / ripemd160。
#[tauri::command]
pub fn tool_hash(algorithm: String, input: String) -> ApiResult<lycoris_core::tools::DigestOutput> {
    Ok(lycoris_core::tools::hash_string(&algorithm, &input)?)
}

/// 编码转换:hex / base64 / base58 并排展示。
#[tauri::command]
pub fn tool_encode(input: String) -> ApiResult<lycoris_core::tools::EncodingOutput> {
    Ok(lycoris_core::tools::transform_encoding(&input)?)
}

/// 解析 BIP-21 / EIP-681 / Solana Pay 支付 URI。
#[tauri::command]
pub fn tool_parse_uri(uri: String) -> ApiResult<lycoris_core::tools::ParsedUri> {
    Ok(lycoris_core::tools::parse_payment_uri(&uri)?)
}

/// 构造支付 URI(BIP-21 / EIP-681 / Solana Pay)。
#[tauri::command]
pub fn tool_compose_uri(
    chain: ChainId,
    address: String,
    amount: Option<String>,
    label: Option<String>,
    message: Option<String>,
) -> ApiResult<String> {
    Ok(lycoris_core::tools::compose_payment_uri(
        chain,
        &address,
        amount.as_deref(),
        label.as_deref(),
        message.as_deref(),
    )?)
}

/// 用当前钱包的密钥对任意消息签名(EVM EIP-191 / Bitcoin BIP-137 /
/// Solana ed25519),签名在 Rust 侧完成,密钥不经过 JS。
#[tauri::command]
pub fn tool_sign_message(
    state: State<'_, crate::state::AppState>,
    chain: ChainId,
    message: String,
    password: String,
) -> ApiResult<lycoris_core::tx::message::MessageSignature> {
    if !state.verify_password(&password)? {
        return Err(ApiError::from(lycoris_core::Error::IncorrectPassword));
    }
    let secret = state.active_secret()?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
    match chain {
        ChainId::Ethereum | ChainId::EthereumClassic | ChainId::Bsc | ChainId::Polygon => Ok(
            lycoris_core::tx::message::evm_sign_message(material.secp()?, &message)?,
        ),
        ChainId::Bitcoin | ChainId::Litecoin => Ok(lycoris_core::tx::message::btc_sign_message(
            material.secp()?,
            &message,
        )?),
        ChainId::Solana => Ok(lycoris_core::tx::message::solana_sign_message(
            &material.ed25519_seed()?,
            &message,
        )?),
        _ => Err(ApiError::new(
            "UNSUPPORTED_CHAIN",
            format!("{} 暂不支持消息签名", chain.info().name),
        )),
    }
}

/// 验证消息签名:返回签名者地址,与预期不符即应视为无效。
#[tauri::command]
pub fn tool_verify_message(
    chain: ChainId,
    message: String,
    signature: String,
) -> ApiResult<lycoris_core::tx::message::VerifyResult> {
    match chain {
        ChainId::Ethereum | ChainId::EthereumClassic | ChainId::Bsc | ChainId::Polygon => {
            let address = lycoris_core::tx::message::evm_verify_message(&message, &signature)
                .map_err(ApiError::from)?;
            Ok(lycoris_core::tx::message::VerifyResult {
                scheme: "eip-191 personal_sign".into(),
                signer: address,
                verified: true,
            })
        }
        ChainId::Bitcoin | ChainId::Litecoin => {
            // BTC 侧接受 hex 或 base64 两种输入。
            let b64 = if signature.trim().len() == 130 || signature.trim().len() == 128 {
                let raw = hex::decode(signature.trim().trim_start_matches("0x")).map_err(|e| {
                    ApiError::new("INVALID_SIGNATURE", format!("hex 解码失败: {e}"))
                })?;
                // 130 = r‖s‖v,128 = r‖s(取不到恢复位则报错)
                if raw.len() != 65 {
                    return Err(ApiError::new(
                        "INVALID_SIGNATURE",
                        "hex 签名应为 65 字节(r‖s‖v)",
                    ));
                }
                use base64::Engine as _;
                base64::engine::general_purpose::STANDARD.encode(raw)
            } else {
                signature.trim().to_string()
            };
            let address = lycoris_core::tx::message::btc_verify_message(&message, &b64)
                .map_err(ApiError::from)?;
            Ok(lycoris_core::tx::message::VerifyResult {
                scheme: "bip-137 (compressed)".into(),
                signer: address,
                verified: true,
            })
        }
        ChainId::Solana => {
            // Solana 验签需要公钥:签名输入格式为 "地址:签名hex"。
            let (address, sig) = signature.trim().split_once(':').ok_or_else(|| {
                ApiError::new("INVALID_SIGNATURE", "Solana 验签格式:地址:签名hex")
            })?;
            let verified = lycoris_core::tx::message::solana_verify_message(address, &message, sig)
                .map_err(ApiError::from)?;
            Ok(lycoris_core::tx::message::VerifyResult {
                scheme: "ed25519 (solana sign_message)".into(),
                signer: address.to_string(),
                verified,
            })
        }
        _ => Err(ApiError::new(
            "UNSUPPORTED_CHAIN",
            format!("{} 暂不支持消息验签", chain.info().name),
        )),
    }
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
/// stray click cannot destroy funds. `reset_vault` is the password-less
/// variant used when the password itself is lost.
#[tauri::command]
pub fn delete_vault(state: State<'_, crate::state::AppState>, password: String) -> ApiResult<()> {
    if state.status().exists && !state.verify_password(&password)? {
        return Err(ApiError::from(lycoris_core::Error::IncorrectPassword));
    }
    Ok(state.reset_vault()?)
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

/// 钥匙环补加/替换(私钥钱包专属)。密码确认;成功后返回重派生的钱包视图。
#[tauri::command]
pub fn update_key_slot(
    state: State<'_, crate::state::AppState>,
    slot: KeySlotKind,
    key: String,
    password: String,
) -> ApiResult<WalletView> {
    state
        .update_key_slot(slot, key.trim(), &password)
        .map_err(ApiError::from)
}

/// 钥匙环摘除(私钥钱包专属)。至少保留一个槽。
#[tauri::command]
pub fn remove_key_slot(
    state: State<'_, crate::state::AppState>,
    slot: KeySlotKind,
    password: String,
) -> ApiResult<WalletView> {
    state
        .remove_key_slot(slot, &password)
        .map_err(ApiError::from)
}

/// 聚合钱包补加一把私钥(链 + 原始钥匙 + 可选备注)。
#[tauri::command]
pub fn add_aggregate_key(
    state: State<'_, crate::state::AppState>,
    chain: ChainId,
    key: String,
    label: Option<String>,
    password: String,
) -> ApiResult<WalletView> {
    state
        .add_aggregate_key(chain, key.trim(), label, &password)
        .map_err(ApiError::from)
}

/// 聚合钱包摘除一把私钥。
#[tauri::command]
pub fn remove_aggregate_key(
    state: State<'_, crate::state::AppState>,
    id: String,
    password: String,
) -> ApiResult<WalletView> {
    state
        .remove_aggregate_key(&id, &password)
        .map_err(ApiError::from)
}

/// 聚合钱包重命名一把私钥的备注。
#[tauri::command]
pub fn rename_aggregate_key(
    state: State<'_, crate::state::AppState>,
    id: String,
    label: String,
    password: String,
) -> ApiResult<WalletView> {
    state
        .rename_aggregate_key(&id, label.trim(), &password)
        .map_err(ApiError::from)
}

#[tauri::command]
pub fn window_toggle_maximize(window: tauri::Window) -> ApiResult<bool> {
    window::toggle_maximize(&window)
}

#[tauri::command]
pub fn window_close(window: tauri::Window) -> ApiResult<()> {
    window::close(&window)
}
