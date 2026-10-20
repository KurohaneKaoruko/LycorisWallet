//! Monero integration.
//!
//! Monero cannot be signed with a generic ed25519 library: every spend is an
//! MLSAG/CLSAG ring signature plus a Bulletproofs+ range proof, and a mistake
//! there is unrecoverable. The app therefore keeps custody of the derived
//! spend/view keys and hands them to the official `monero-wallet-rpc`, which
//! scans the chain, picks ring members and signs. This is the same architecture
//! Feather uses; the RPC endpoint defaults to localhost and the wallet file is
//! created from *our* keys, so no third party ever holds a seed.

use crate::error::{ApiError, ApiResult};
use crate::portfolio::{format_amount, QuoteFees};
use crate::state::AppState;
use lycoris_core::address;
use lycoris_core::chains::ChainId;
use lycoris_core::cryptonote::MoneroKeys;
use lycoris_core::error::Error;
use lycoris_core::models::{FeeOption, Settings};
use lycoris_core::network::monero::{MoneroRpc, WalletImport};
use serde::Serialize;
use std::time::Duration;

/// Piconero per XMR.
const DECIMALS: u8 = 12;
/// Weight of a typical 1-input / 2-output CLSAG transaction, in bytes.
const TX_WEIGHT: u64 = 1_600;
/// Monero's dynamic fee floor: 20,000 piconero per byte.
const FEE_PER_BYTE: u64 = 20_000;

/// Monero's wallet priorities are 1 (unimportant), 2 (normal) and 3
/// (elevated); their fee multipliers are 1×, 4× and 20×.
fn priority_for(level: &str) -> u8 {
    match level {
        "economy" => 1,
        "fast" => 3,
        _ => 2,
    }
}

fn multiplier_for(level: &str) -> u64 {
    match level {
        "economy" => 1,
        "fast" => 20,
        _ => 4,
    }
}

fn fee_for(level: &str) -> u128 {
    u128::from(TX_WEIGHT) * u128::from(FEE_PER_BYTE) * u128::from(multiplier_for(level))
}

/// The wallet RPC can scan for minutes on the first import, so the default
/// 20-second HTTP timeout is not enough.
fn http() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .user_agent("LycorisWallet/0.1 (+https://example.invalid)")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Accept both `http://host:18083` and the full JSON-RPC path; users paste
/// either into settings.
fn endpoint(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    if trimmed.ends_with("/json_rpc") {
        trimmed.to_string()
    } else {
        format!("{trimmed}/json_rpc")
    }
}

fn rpc(settings: &Settings) -> MoneroRpc {
    MoneroRpc::new(
        http(),
        endpoint(&settings.monero_rpc_url),
        settings.monero_rpc_user.trim(),
        settings.monero_rpc_password.trim(),
    )
}

/// Stable wallet-file name for an address. `monero-wallet-rpc` stores it in
/// its own `--wallet-dir`.
fn wallet_name(address: &str) -> String {
    let digest = address::keccak256(address.as_bytes());
    format!("lycoris-{}", &hex::encode(digest)[..16])
}

/// The wallet file is encrypted with a password derived from the spend key, so
/// the vault never has to store another secret — and the file cannot be opened
/// by someone who only has the (public) address.
fn wallet_password(keys: &MoneroKeys) -> String {
    hex::encode(address::keccak256(&keys.private_spend))
}

pub fn monero_error(error: Error) -> ApiError {
    match error {
        Error::RpcUnavailable(message) => ApiError::new(
            "MONERO_RPC",
            format!(
                "无法连接 monero-wallet-rpc（{message}）。\
                 请在本地启动它（monero-wallet-rpc --rpc-bind-port 18083 --wallet-dir <目录> \
                 --daemon-address <节点>），地址可在「设置 → 网络」修改。"
            ),
        ),
        other => ApiError::new(other.code(), format!("Monero：{other}")),
    }
}

/// Open (or create) the wallet that belongs to the active phrase and return a
/// ready RPC handle.
async fn connect(state: &AppState, chain: ChainId, address: &str) -> ApiResult<MoneroRpc> {
    let settings = state.settings()?;
    if settings.monero_rpc_url.trim().is_empty() {
        return Err(ApiError::new(
            "MONERO_RPC",
            "未配置 monero-wallet-rpc 地址（设置 → 网络）",
        ));
    }
    let secret = state.active_secret()?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
    let keys = material.monero_keys()?;
    let spend = hex::encode(keys.private_spend);
    let view = hex::encode(keys.private_view);
    let filename = wallet_name(address);
    let password = wallet_password(keys);
    let import = WalletImport {
        filename: &filename,
        password: &password,
        address,
        spend_key: &spend,
        view_key: &view,
        restore_height: settings.monero_restore_height,
    };
    let rpc = rpc(&settings);
    rpc.ensure_wallet(&import).await.map_err(monero_error)?;
    Ok(rpc)
}

/// Unlocked balance (piconero). `Err` is a user-facing message.
pub async fn balance(state: &AppState, chain: ChainId, address: &str) -> Result<u128, String> {
    let rpc = connect(state, chain, address)
        .await
        .map_err(|error| error.message)?;
    let balance = rpc
        .balance(0)
        .await
        .map_err(|error| monero_error(error).message)?;
    Ok(u128::from(balance.unlocked_balance))
}

pub async fn quote(
    state: &AppState,
    chain: ChainId,
    to: &str,
    _amount: Option<u128>,
) -> ApiResult<QuoteFees> {
    address::validate_address(chain, to)?;
    let account = state.account_for(chain)?;
    let rpc = connect(state, chain, &account.address).await?;
    let balance = rpc.balance(0).await.map_err(monero_error)?;
    let unlocked = u128::from(balance.unlocked_balance);
    let normal = fee_for("normal");

    let option = |level: &str, label: &str, eta: &str, note: &str| FeeOption {
        level: level.to_string(),
        label: label.to_string(),
        fee: format_amount(fee_for(level), DECIMALS),
        fee_fiat: None,
        eta: eta.to_string(),
        note: note.to_string(),
        selected: level == "normal",
    };

    Ok(QuoteFees {
        options: vec![
            option("fast", "快速", "约 2 分钟", "优先级 3 · 环签名 16"),
            option("normal", "标准", "约 2 分钟", "优先级 2 · 环签名 16"),
            option("economy", "经济", "约 20 分钟", "优先级 1 · 环签名 16"),
        ],
        normal_fee: normal,
        max_amount: Some(unlocked.saturating_sub(normal)),
        balance: unlocked,
    })
}

/// Returns `(txid, fee in piconero, amount sent in piconero)`.
pub async fn send(
    state: &AppState,
    chain: ChainId,
    to: &str,
    amount: Option<u128>,
    level: &str,
) -> ApiResult<(String, u64, u128)> {
    address::validate_address(chain, to)?;
    let account = state.account_for(chain)?;
    let rpc = connect(state, chain, &account.address).await?;
    let priority = priority_for(level);

    match amount {
        Some(value) => {
            let value = u64::try_from(value)
                .map_err(|_| ApiError::new("INVALID_AMOUNT", "金额超出 Monero 可表示范围"))?;
            if value == 0 {
                return Err(ApiError::new("INVALID_AMOUNT", "金额必须大于零"));
            }
            let outcome = rpc
                .transfer(to, value, priority)
                .await
                .map_err(monero_error)?;
            Ok((
                outcome.tx_hash,
                outcome.fee,
                u128::from(outcome.amount.max(value)),
            ))
        }
        None => {
            // Sending everything has to let the wallet compute the fee, so use
            // sweep_all instead of guessing an amount.
            let outcome = rpc.sweep_all(to, priority).await.map_err(monero_error)?;
            let txid = outcome
                .tx_hash_list
                .first()
                .cloned()
                .ok_or_else(|| ApiError::new("MONERO_RPC", "钱包没有返回交易哈希"))?;
            let fee = outcome.fee_list.iter().copied().sum();
            let sent: u64 = outcome.amount_list.iter().copied().sum();
            Ok((txid, fee, u128::from(sent)))
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoneroStatus {
    pub connected: bool,
    pub version: Option<u64>,
    pub height: Option<u64>,
    pub address: Option<String>,
    pub wallet_matches: bool,
    pub message: String,
}

/// Connection test used by the settings page. It never creates a wallet file;
/// the first balance query or send does that.
pub async fn status(state: &AppState) -> ApiResult<MoneroStatus> {
    let settings = state.settings()?;
    if settings.monero_rpc_url.trim().is_empty() {
        return Ok(MoneroStatus {
            connected: false,
            version: None,
            height: None,
            address: None,
            wallet_matches: false,
            message: "未配置 monero-wallet-rpc 地址（设置 → 网络）".to_string(),
        });
    }
    let rpc = rpc(&settings);
    let version = match rpc.version().await {
        Ok(version) => version,
        Err(error) => {
            return Ok(MoneroStatus {
                connected: false,
                version: None,
                height: None,
                address: None,
                wallet_matches: false,
                message: monero_error(error).message,
            })
        }
    };
    let height = rpc.height().await.ok();
    let account = state.account_for(ChainId::Monero)?;
    let address = rpc.address(0).await.ok();
    let wallet_matches = address.as_deref() == Some(account.address.as_str());
    let message = if wallet_matches {
        "已连接，钱包与当前助记词匹配".to_string()
    } else if address.is_some() {
        "已连接，但打开的是另一个钱包；首次查询余额时会切换".to_string()
    } else {
        "已连接，尚未打开钱包；首次查询余额时会自动创建".to_string()
    };
    Ok(MoneroStatus {
        connected: true,
        version: Some(version),
        height,
        address,
        wallet_matches,
        message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fee_levels_follow_monero_priorities() {
        assert_eq!(priority_for("economy"), 1);
        assert_eq!(priority_for("normal"), 2);
        assert_eq!(priority_for("fast"), 3);
        // 1,600 bytes × 20,000 piconero/byte × 1× / 4× / 20×.
        assert_eq!(fee_for("economy"), 32_000_000);
        assert_eq!(fee_for("normal"), 128_000_000);
        assert_eq!(fee_for("fast"), 640_000_000);
    }

    #[test]
    fn rpc_url_accepts_both_forms() {
        assert_eq!(
            endpoint("http://127.0.0.1:18083"),
            "http://127.0.0.1:18083/json_rpc"
        );
        assert_eq!(
            endpoint("http://127.0.0.1:18083/"),
            "http://127.0.0.1:18083/json_rpc"
        );
        assert_eq!(
            endpoint(" http://127.0.0.1:18083/json_rpc "),
            "http://127.0.0.1:18083/json_rpc"
        );
    }

    #[test]
    fn wallet_name_and_password_are_deterministic() {
        let address = "48zQvBdmMxj9ByEJWD3uPAVdSKJEnJEsMS8ppnVdJXuh5GjcU9wRcuBck7NTZidFd44bFJmpumZRRi5yufFhd3dP7mHe5Eb";
        let name = wallet_name(address);
        assert!(name.starts_with("lycoris-"));
        assert_eq!(name.len(), "lycoris-".len() + 16);
        assert_eq!(name, wallet_name(address));
        assert_ne!(name, wallet_name("46XrxrAa5aXcfkFrdSACS2EfmfxHdJkXLcGuDD2ezLNFfrfMgBJPoP5NAEz3N9WSJuhPox5S78AY3PGVeZQczueyDDT66tY"));

        let keys = MoneroKeys::from_spend_seed(&[7u8; 32]);
        let password = wallet_password(&keys);
        assert_eq!(password.len(), 64);
        assert!(password.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(password, wallet_password(&keys));
    }
}
