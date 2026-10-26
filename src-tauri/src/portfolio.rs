//! Balances, send quotes and transaction broadcast for every supported chain.
//!
//! All network traffic and all signing happen here; the frontend only ever
//! sees addresses, formatted amounts and transaction ids.

use crate::error::{ApiError, ApiResult};
use crate::monero;
use crate::state::AppState;
use lycoris_core::address;
use lycoris_core::chains::{ChainId, ChainInfo, ChainKind, UtxoBackend};
use lycoris_core::error::Error;
use lycoris_core::models::{
    ActivityItem, ActivityStatus, AssetBalance, Direction, FeeOption, FiatCurrency, Portfolio,
    SendQuote, SendResult,
};
use lycoris_core::network::{
    blockbook::BlockbookClient, blockchair::BlockchairClient, chainz::ChainzClient, evm::EvmClient,
    history, prices, solana::SolanaClient, utxo::UtxoClient, woc::WocClient,
};
use lycoris_core::tx::evm::{sign_transaction, EvmCall, EvmFee};
use lycoris_core::tx::solana::build_transfer;
use lycoris_core::tx::utxo::{
    build_signed_p2pkh, build_signed_p2wpkh, change_script, fee_rate_for_level,
    legacy_change_script, max_legacy_sendable, max_sendable, recipient_script, select_legacy_utxos,
    select_utxos, UtxoInput,
};
use lycoris_core::units::{format_units, parse_units, truncate_decimal};
use lycoris_core::vault::now;
use serde::Deserialize;
use std::collections::HashMap;

const GWEI: u128 = 1_000_000_000;
const NATIVE_TRANSFER_GAS: u64 = 21_000;
const SOLANA_BASE_FEE: u64 = 5_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuoteRequest {
    pub chain: ChainId,
    pub to: String,
    #[serde(default)]
    pub amount: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SendRequest {
    pub chain: ChainId,
    pub to: String,
    pub amount: String,
    #[serde(default = "default_level")]
    pub level: String,
}

fn default_level() -> String {
    "normal".to_string()
}

fn level_label(level: &str) -> &'static str {
    match level {
        "fast" => "快速",
        "economy" => "经济",
        _ => "标准",
    }
}

pub(crate) fn format_amount(value: u128, decimals: u8) -> String {
    truncate_decimal(&format_units(value, decimals), 8)
}

fn balance_text(chain: ChainId, amount: u128) -> String {
    format_amount(amount, chain.info().decimals)
}

fn price_of(
    table: &HashMap<String, HashMap<String, f64>>,
    info: &ChainInfo,
    key: &str,
) -> Option<f64> {
    table
        .get(info.price_id)
        .and_then(|entry| entry.get(key))
        .copied()
}

/// Best-effort spot prices; failures degrade to `None` rather than failing the
/// whole request.
async fn load_prices(
    client: &reqwest::Client,
    chain: Option<ChainId>,
) -> HashMap<String, HashMap<String, f64>> {
    let ids: Vec<&str> = match chain {
        Some(chain) => vec![chain.info().price_id],
        None => {
            let mut ids: Vec<&str> = ChainId::ALL
                .iter()
                .map(|chain| chain.info().price_id)
                .collect();
            ids.sort_unstable();
            ids.dedup();
            ids
        }
    };
    prices::fetch_prices(client, &ids, &["usd", "cny"])
        .await
        .unwrap_or_default()
}

// ------------------------------------------------------------------ portfolio

fn asset_from(
    chain: ChainId,
    address: String,
    amount: Option<u128>,
    error: Option<String>,
) -> AssetBalance {
    let info = chain.info();
    AssetBalance {
        chain,
        name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        decimals: info.decimals,
        accent: info.accent.to_string(),
        address: address.clone(),
        amount: amount
            .map(|value| balance_text(chain, value))
            .unwrap_or_default(),
        price_usd: None,
        price_cny: None,
        value_usd: None,
        value_cny: None,
        explorer_url: format!("{}{}", info.explorer_address, address),
        error,
    }
}

/// One UTXO chain, any index dialect. `Err` carries a friendly message for
/// chains without a public data source.
#[derive(Clone)]
enum UtxoAdapter {
    Esplora(UtxoClient),
    Blockbook(BlockbookClient),
    Blockchair(BlockchairClient),
    Woc(WocClient),
    Chainz(ChainzClient),
}

const NO_DATA_SOURCE: &str = "该链暂无公共数据源（浏览器链接仍可用）";

/// Blockchair's per-chain base URL, used by the fallback backend only.
fn blockchair_base(chain: ChainId) -> &'static str {
    match chain {
        ChainId::Dogecoin => "https://api.blockchair.com/dogecoin",
        ChainId::BitcoinCash => "https://api.blockchair.com/bitcoin-cash",
        ChainId::Dash => "https://api.blockchair.com/dash",
        ChainId::Groestlcoin => "https://api.blockchair.com/groestlcoin",
        ChainId::Feathercoin => "https://api.blockchair.com/feathercoin",
        ChainId::Viacoin => "https://api.blockchair.com/viacoin",
        _ => "https://api.blockchair.com/bitcoin",
    }
}

impl UtxoAdapter {
    fn from_backend(
        client: &reqwest::Client,
        chain: ChainId,
        backend: UtxoBackend,
        blockchair_key: Option<&str>,
    ) -> Option<Self> {
        let info = chain.info();
        match backend {
            UtxoBackend::Esplora => Some(Self::Esplora(UtxoClient::new(client.clone(), info.rpc))),
            UtxoBackend::Blockbook => Some(Self::Blockbook(BlockbookClient::new(
                client.clone(),
                info.rpc,
            ))),
            UtxoBackend::Blockchair => {
                // The fallback reads Blockchair directly (primary chains keep
                // their own rpc; blockbook chains get the per-chain base).
                let base = if chain.utxo_backend() == UtxoBackend::Blockchair {
                    info.rpc
                } else {
                    blockchair_base(chain)
                };
                Some(Self::Blockchair(BlockchairClient::with_settings(
                    client.clone(),
                    base,
                    blockchair_key,
                )))
            }
            UtxoBackend::WhatsOnChain => Some(Self::Woc(WocClient::new(client.clone(), info.rpc))),
            UtxoBackend::Chainz => Some(Self::Chainz(ChainzClient::new(
                client.clone(),
                chain.chainz_coin(),
                info.decimals,
            ))),
            UtxoBackend::None => None,
        }
    }

    /// Builds the primary client for `chain`; `None` when the chain has no
    /// index API at all.
    fn for_chain(
        client: &reqwest::Client,
        chain: ChainId,
        blockchair_key: Option<&str>,
    ) -> Option<Self> {
        Self::from_backend(client, chain, chain.utxo_backend(), blockchair_key)
    }

    async fn balance(&self, chain: ChainId, address: &str) -> ApiResult<u64> {
        match self {
            Self::Esplora(client) => client.balance(address).await.map_err(Into::into),
            Self::Blockbook(client) => client
                .balance(address, chain.info().decimals)
                .await
                .map_err(Into::into),
            Self::Blockchair(client) => client.balance(address).await.map_err(Into::into),
            Self::Woc(client) => client.balance(address).await.map_err(Into::into),
            Self::Chainz(client) => client.balance(address).await.map_err(Into::into),
        }
    }

    async fn utxos(
        &self,
        chain: ChainId,
        address: &str,
    ) -> ApiResult<Vec<lycoris_core::network::utxo::Utxo>> {
        match self {
            Self::Esplora(client) => client.utxos(address).await.map_err(Into::into),
            Self::Blockbook(client) => client
                .utxos(address, chain.info().decimals)
                .await
                .map_err(Into::into),
            Self::Blockchair(client) => client.utxos(address).await.map_err(Into::into),
            Self::Woc(client) => client.utxos(address).await.map_err(Into::into),
            Self::Chainz(_) => Err(ApiError::new(
                "UNSUPPORTED",
                "该链暂无公共 UTXO 数据源，无法转账（余额查询可用）",
            )),
        }
    }

    async fn fees(&self, chain: ChainId) -> ApiResult<lycoris_core::tx::utxo::RecommendedFees> {
        match self {
            Self::Esplora(client) => client.fees().await.map_err(Into::into),
            Self::Blockbook(client) => client.fees(chain.info().decimals).await.map_err(Into::into),
            // Blockchair/WoC/Chainz have no free recommended-fee endpoint:
            // fall back to the chain's documented default rate.
            Self::Blockchair(_) | Self::Woc(_) | Self::Chainz(_) => Ok(default_fees(chain)),
        }
    }

    async fn broadcast(&self, raw_hex: &str) -> ApiResult<String> {
        match self {
            Self::Esplora(client) => client.broadcast(raw_hex).await.map_err(Into::into),
            Self::Blockbook(client) => client.broadcast(raw_hex).await.map_err(Into::into),
            Self::Blockchair(client) => client.broadcast(raw_hex).await.map_err(Into::into),
            Self::Woc(client) => client.broadcast(raw_hex).await.map_err(Into::into),
            Self::Chainz(_) => Err(ApiError::new(
                "UNSUPPORTED",
                "该链暂无公共广播端点，无法转账（余额查询可用）",
            )),
        }
    }

    /// Recent transactions, when the dialect has a history endpoint.
    async fn transactions(
        &self,
        chain: ChainId,
        address: &str,
    ) -> ApiResult<Vec<lycoris_core::network::history::HistoryTx>> {
        match self {
            Self::Esplora(_) => history::fetch_esplora(
                &lycoris_core::network::http_client(),
                chain.info().rpc,
                chain,
                address,
            )
            .await
            .map_err(ApiError::from),
            Self::Blockbook(client) => client
                .transactions(chain, address)
                .await
                .map_err(ApiError::from),
            Self::Blockchair(_) | Self::Woc(_) | Self::Chainz(_) => Ok(Vec::new()),
        }
    }
}

fn default_fees(chain: ChainId) -> lycoris_core::tx::utxo::RecommendedFees {
    let rate = chain.default_fee_rate();
    lycoris_core::tx::utxo::RecommendedFees {
        fastest: rate,
        half_hour: rate,
        hour: rate,
        economy: rate,
        minimum: rate,
    }
}

/// Balance with automatic multi-source fallback: primary backend first,
/// then the chain's configured fallback — so out-of-the-box wallets never
/// need an API key to show a balance.
async fn fetch_balance(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    blockchair_key: Option<&str>,
) -> Result<u128, String> {
    match chain.info().kind {
        ChainKind::Utxo => {
            let primary = UtxoAdapter::for_chain(client, chain, blockchair_key);
            let mut last_error = None;
            for adapter in
                std::iter::once(primary).chain(chain.fallback_backend().map(|backend| {
                    UtxoAdapter::from_backend(client, chain, backend, blockchair_key)
                }))
            {
                let Some(adapter) = adapter else {
                    continue;
                };
                match adapter.balance(chain, address).await {
                    Ok(amount) => return Ok(u128::from(amount)),
                    Err(error) => last_error = Some(error.to_string()),
                }
            }
            Err(match last_error {
                Some(message) => message,
                None => NO_DATA_SOURCE.to_string(),
            })
        }
        ChainKind::Evm => EvmClient::new(client.clone(), chain.info().rpc)
            .balance(address)
            .await
            .map_err(|error| error.to_string()),
        ChainKind::Ed25519 => SolanaClient::new(client.clone(), chain.info().rpc)
            .balance(address)
            .await
            .map(u128::from)
            .map_err(|error| error.to_string()),
        // Monero balances come from monero-wallet-rpc in `refresh`, which
        // needs the derived keys; this fan-out is stateless by design.
        ChainKind::Cryptonote => {
            Err("Monero 余额由本地 monero-wallet-rpc 提供（设置 → 网络）".to_string())
        }
    }
}

fn asset_from_result(
    chain: ChainId,
    address: String,
    result: Result<u128, String>,
    table: &HashMap<String, HashMap<String, f64>>,
) -> (AssetBalance, Option<String>) {
    let info = chain.info();
    match result {
        Ok(amount) => {
            let mut asset = asset_from(chain, address, Some(amount), None);
            asset.price_usd = price_of(table, &info, "usd");
            asset.price_cny = price_of(table, &info, "cny");
            let human = amount as f64 / 10f64.powi(i32::from(info.decimals));
            asset.value_usd = asset.price_usd.map(|price| price * human);
            asset.value_cny = asset.price_cny.map(|price| price * human);
            (asset, None)
        }
        Err(message) => {
            let warning = format!("{} 余额获取失败：{message}", info.name);
            (
                asset_from(chain, address, None, Some(message)),
                Some(warning),
            )
        }
    }
}

/// Outcome of an activity sync, shown as a subtle status line in the UI.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySyncReport {
    pub new_items: usize,
    pub confirmed_items: usize,
    /// Chains that failed; their names, for a short status hint.
    pub failed_chains: Vec<String>,
}

/// Pulls on-chain history for every address of the active wallet and merges
/// it into the activity list:
/// - incoming transactions become `Receive` entries,
/// - pending local entries are confirmed/failed by their live status,
/// - already-known txids are never duplicated.
pub async fn refresh_activity(state: &AppState) -> ApiResult<ActivitySyncReport> {
    state.touch();
    let accounts = state.all_accounts()?;
    let existing = state.activity()?;
    let client = lycoris_core::network::http_client();
    let blockchair_key = state
        .settings()
        .ok()
        .map(|settings| settings.blockchair_api_key.clone());

    let mut tasks = tokio::task::JoinSet::new();
    for (_, account) in accounts {
        let chain = account.chain;
        if chain.info().kind != ChainKind::Utxo {
            continue; // EVM/Solana history needs indexer APIs; Monero needs rpc.
        }
        let adapter = match UtxoAdapter::for_chain(&client, chain, blockchair_key.as_deref()) {
            Some(adapter) => adapter,
            None => continue, // explorer-only chain
        };
        let address = account.address.clone();
        tasks.spawn(async move {
            let result = adapter.transactions(chain, &address).await;
            (chain, result)
        });
    }

    let mut incoming: Vec<lycoris_core::network::history::HistoryTx> = Vec::new();
    let mut failed_chains = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        let Ok((chain, result)) = joined else {
            continue;
        };
        match result {
            Ok(txs) => incoming.extend(txs),
            Err(_) => failed_chains.push(chain.info().name.to_string()),
        }
    }

    let mut merged = existing;
    let mut report = ActivitySyncReport {
        new_items: 0,
        confirmed_items: 0,
        failed_chains,
    };

    // Confirm pending local entries: the history pages carry live status.
    let live: HashMap<(ChainId, String), (bool, u64)> = incoming
        .iter()
        .map(|tx| ((tx.chain, tx.txid.clone()), (tx.outgoing, tx.confirmations)))
        .collect();
    for item in merged.iter_mut() {
        if item.status != ActivityStatus::Pending {
            continue;
        }
        if let Some((_, confirmations)) = live.get(&(item.chain, item.txid.clone())) {
            if *confirmations > 0 {
                item.status = ActivityStatus::Confirmed;
                report.confirmed_items += 1;
            }
        }
    }

    // Add on-chain receives (and chain-visible sends we did not record).
    let known: std::collections::HashSet<(ChainId, String, bool)> = merged
        .iter()
        .map(|item| {
            (
                item.chain,
                item.txid.clone(),
                item.direction == Direction::Send,
            )
        })
        .collect();
    let mut fresh: Vec<ActivityItem> = Vec::new();
    for tx in incoming {
        let direction = if tx.outgoing {
            Direction::Send
        } else {
            Direction::Receive
        };
        if known.contains(&(tx.chain, tx.txid.clone(), tx.outgoing)) {
            continue;
        }
        // Skip placeholder rows (amount unknown, e.g. Solana signature rows).
        if tx.amount == 0 && tx.fee.is_none() {
            continue;
        }
        if tx.amount == 0 {
            continue;
        }
        let info = tx.chain.info();
        fresh.push(ActivityItem {
            id: uuid::Uuid::new_v4().to_string(),
            chain: tx.chain,
            direction,
            amount: format_amount(tx.amount, info.decimals),
            symbol: info.symbol.to_string(),
            counterparty: "链上记录".to_string(),
            txid: tx.txid.clone(),
            status: if tx.confirmations > 0 {
                ActivityStatus::Confirmed
            } else {
                ActivityStatus::Pending
            },
            fee: tx
                .fee
                .map(|fee| format_amount(fee, info.decimals))
                .unwrap_or_default(),
            timestamp: if tx.timestamp > 0 {
                tx.timestamp
            } else {
                now()
            },
            explorer_url: format!("{}{}", info.explorer_tx, tx.txid),
        });
    }
    report.new_items = fresh.len();
    merged.extend(fresh);
    merged.sort_by_key(|entry| std::cmp::Reverse(entry.timestamp));
    merged.dedup_by(|a, b| a.txid == b.txid && a.chain == b.chain && a.direction == b.direction);
    merged.truncate(500);
    state.replace_activity(merged)?;
    Ok(report)
}

pub async fn refresh(state: &AppState) -> ApiResult<Portfolio> {
    state.touch();
    // 聚合钱包的全部派生账户：总览显示所有已添加地址的资产总和。
    let all_accounts = state.all_accounts()?;
    let current = state.accounts()?;
    let client = lycoris_core::network::http_client();
    let table = load_prices(&client, None).await;

    // Monero needs the active secret and a wallet RPC, so it cannot join the
    // stateless balance fan-out below.
    let (monero_entries, rest): (Vec<_>, Vec<_>) = all_accounts
        .into_iter()
        .partition(|(_, account)| account.chain.info().kind == ChainKind::Cryptonote);

    let mut tasks = tokio::task::JoinSet::new();
    let blockchair_key = state
        .settings()
        .ok()
        .and_then(|settings| {
            let key = settings.blockchair_api_key;
            (!key.is_empty()).then_some(key)
        })
        .map(std::sync::Arc::new);
    for (id, account) in rest {
        let client = client.clone();
        let chain = account.chain;
        let blockchair_key = blockchair_key.clone();
        tasks.spawn(async move {
            let result = fetch_balance(
                &client,
                chain,
                &account.address,
                blockchair_key.as_deref().map(String::as_str),
            )
            .await;
            (id, chain, account.address, result)
        });
    }

    // (chain -> amount, error, address of the currently selected account)
    let mut amounts: HashMap<ChainId, (u128, Option<String>)> = HashMap::new();
    let mut addresses: HashMap<ChainId, String> = current
        .iter()
        .map(|account| (account.chain, account.address.clone()))
        .collect();
    let mut warnings = Vec::new();

    // Monero 没有多派生账户约定(一个 spend/view 对就是一个钱包),只有
    // 当前选中账户参与统计,余额来自本地 monero-wallet-rpc。
    if let Some((_, account)) = monero_entries.first() {
        let result = monero::balance(state, account.chain, &account.address).await;
        match result {
            Ok(amount) => {
                amounts.insert(account.chain, (amount, None));
            }
            Err(message) => {
                warnings.push(format!(
                    "{} 余额获取失败：{message}",
                    account.chain.info().name
                ));
                amounts.insert(account.chain, (0, Some(message)));
            }
        }
    }

    while let Some(joined) = tasks.join_next().await {
        let Ok((id, chain, address, result)) = joined else {
            continue;
        };
        let _ = id;
        match result {
            Ok(amount) => {
                let entry = amounts.entry(chain).or_insert((0, None));
                entry.0 = entry.0.saturating_add(amount);
            }
            Err(message) => {
                let entry = amounts.entry(chain).or_insert((0, None));
                if entry.1.is_none() {
                    entry.1 = Some(message);
                }
            }
        }
        // 记录每个地址,资产页要展示所有派生地址。
        addresses.entry(chain).or_insert(address);
    }

    let mut assets = Vec::new();
    for chain in ChainId::ALL {
        let Some(address) = addresses.get(&chain) else {
            continue;
        };
        let (amount, error) = amounts
            .get(&chain)
            .cloned()
            .unwrap_or((0, Some("余额未查询".into())));
        let (asset, warning) = asset_from_result(
            chain,
            address.clone(),
            if error.is_some() {
                Err(error.unwrap_or_default())
            } else {
                Ok(amount)
            },
            &table,
        );
        warnings.extend(warning);
        assets.push(asset);
    }
    assets.sort_by_key(|asset| {
        ChainId::ALL
            .iter()
            .position(|chain| *chain == asset.chain)
            .unwrap_or(usize::MAX)
    });

    if table.is_empty() {
        warnings.push("价格服务暂时不可用，仅显示链上余额".to_string());
    }

    let total_usd = assets
        .iter()
        .filter_map(|asset| asset.value_usd)
        .reduce(|a, b| a + b);
    let total_cny = assets
        .iter()
        .filter_map(|asset| asset.value_cny)
        .reduce(|a, b| a + b);

    Ok(Portfolio {
        assets,
        total_usd,
        total_cny,
        fetched_at: now(),
        warnings,
    })
}

// -------------------------------------------------------------------- quoting

pub(crate) struct QuoteFees {
    pub(crate) options: Vec<FeeOption>,
    /// Cost of the default ("normal") level, in the chain's smallest unit.
    pub(crate) normal_fee: u128,
    pub(crate) max_amount: Option<u128>,
    pub(crate) balance: u128,
}

/// Parse a user-entered amount; `None` means "max".
fn parse_amount(input: &str, decimals: u8) -> Result<Option<u128>, ApiError> {
    match input.trim() {
        "" | "max" | "MAX" => Ok(None),
        value => Ok(Some(parse_units(value, decimals)?)),
    }
}

/// Fee levels for the legacy (pre-SegWit) chains. They do not publish a
/// recommended fee rate through the API, so the chain's own minimum relay fee
/// is the baseline and the levels scale from there.
fn legacy_fee_levels(chain: ChainId) -> [(&'static str, u64, &'static str); 3] {
    let base = chain.default_fee_rate();
    [
        ("fast", base * 3 / 2, "约 30 分钟"),
        ("normal", base, "约 1 小时"),
        ("economy", base * 3 / 4, "约 2 小时"),
    ]
}

/// UTXO feed with the same multi-source fallback as `fetch_balance`. Returns
/// the adapter that answered, so the caller broadcasts through the same source
/// it built the transaction from.
async fn utxos_with_fallback(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    blockchair_key: Option<&str>,
) -> ApiResult<(UtxoAdapter, Vec<lycoris_core::network::utxo::Utxo>)> {
    let primary = UtxoAdapter::for_chain(client, chain, blockchair_key);
    let fallback = chain
        .fallback_backend()
        .and_then(|backend| UtxoAdapter::from_backend(client, chain, backend, blockchair_key));
    let mut last_error = None;
    for adapter in [primary, fallback].into_iter().flatten() {
        match adapter.utxos(chain, address).await {
            Ok(utxos) => return Ok((adapter, utxos)),
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| ApiError::new("UNSUPPORTED", NO_DATA_SOURCE)))
}

/// Quote for Dogecoin / Bitcoin Cash: legacy P2PKH spends priced in
/// satoshis per byte.
async fn legacy_utxo_quote(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    amount: Option<u128>,
    blockchair_key: Option<&str>,
) -> ApiResult<QuoteFees> {
    let info = chain.info();
    let (_adapter, utxos) = utxos_with_fallback(client, chain, address, blockchair_key).await?;
    let inputs: Vec<UtxoInput> = utxos
        .iter()
        .map(|utxo| UtxoInput::from_parts(&utxo.txid, utxo.vout, utxo.value, utxo.confirmed))
        .collect::<Result<_, _>>()?;
    let balance: u64 = inputs.iter().map(|input| input.value).sum();
    let target = amount.map(|value| u64::try_from(value).unwrap_or(u64::MAX));

    let mut options = Vec::new();
    let mut normal_fee = 0u128;
    for (level, rate, eta) in legacy_fee_levels(chain) {
        let fee = match target {
            Some(value) => select_legacy_utxos(inputs.clone(), value, rate, chain.dust_limit())
                .map(|selection| selection.fee)
                .unwrap_or(0),
            None => max_legacy_sendable(&inputs, rate, chain.dust_limit())
                .map(|(_, fee)| fee)
                .unwrap_or(0),
        };
        if level == "normal" {
            normal_fee = u128::from(fee);
        }
        options.push(FeeOption {
            level: level.to_string(),
            label: level_label(level).to_string(),
            fee: format_amount(u128::from(fee), info.decimals),
            fee_fiat: None,
            eta: eta.to_string(),
            note: format!("{rate} {}/字节", chain.base_unit()),
            selected: level == "normal",
        });
    }

    let max_amount = max_legacy_sendable(&inputs, chain.default_fee_rate(), chain.dust_limit())
        .ok()
        .map(|(value, _)| u128::from(value));
    Ok(QuoteFees {
        options,
        normal_fee,
        max_amount,
        balance: u128::from(balance),
    })
}

async fn utxo_quote(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
    blockchair_key: Option<&str>,
) -> ApiResult<QuoteFees> {
    let info = chain.info();
    recipient_script(chain, to)?;
    if chain.legacy_utxo() {
        return legacy_utxo_quote(client, chain, address, amount, blockchair_key).await;
    }
    let (adapter, utxos) = utxos_with_fallback(client, chain, address, blockchair_key).await?;
    let fees = adapter.fees(chain).await?;
    let inputs: Vec<UtxoInput> = utxos
        .iter()
        .map(|utxo| UtxoInput::from_parts(&utxo.txid, utxo.vout, utxo.value, utxo.confirmed))
        .collect::<Result<_, _>>()?;
    let balance: u64 = inputs.iter().map(|input| input.value).sum();
    let target = amount.map(|value| u64::try_from(value).unwrap_or(u64::MAX));

    let mut options = Vec::new();
    let mut normal_fee = 0u128;
    for (level, rate, eta) in [
        ("fast", fee_rate_for_level(&fees, "fast"), "约 10 分钟"),
        ("normal", fee_rate_for_level(&fees, "normal"), "约 30 分钟"),
        ("economy", fee_rate_for_level(&fees, "slow"), "约 1 小时"),
    ] {
        let fee = match target {
            Some(value) => select_utxos(inputs.clone(), value, rate, true, chain.dust_limit())
                .map(|selection| selection.fee)
                .unwrap_or(0),
            None => max_sendable(&inputs, rate, chain.dust_limit())
                .map(|(_, fee)| fee)
                .unwrap_or(0),
        };
        if level == "normal" {
            normal_fee = u128::from(fee);
        }
        options.push(FeeOption {
            level: level.to_string(),
            label: level_label(level).to_string(),
            fee: format_amount(u128::from(fee), info.decimals),
            fee_fiat: None,
            eta: eta.to_string(),
            note: format!("{rate} {}/vB", chain.base_unit()),
            selected: level == "normal",
        });
    }

    let max_amount = max_sendable(
        &inputs,
        fee_rate_for_level(&fees, "normal"),
        chain.dust_limit(),
    )
    .ok()
    .map(|(value, _)| u128::from(value));
    Ok(QuoteFees {
        options,
        normal_fee,
        max_amount,
        balance: u128::from(balance),
    })
}

fn evm_fee_levels(
    chain: ChainId,
    base_fee: Option<u128>,
    priority: Option<u128>,
    gas_price: Option<u128>,
) -> [(&'static str, EvmFee, &'static str, String); 3] {
    let info = chain.info();
    let decimals = info.decimals;
    if matches!(chain, ChainId::Ethereum | ChainId::Polygon) {
        let base = base_fee.unwrap_or(0).max(GWEI);
        let priority = priority.unwrap_or(GWEI).max(1);
        let tip_fast = (priority * 2).max(GWEI);
        let tip_economy = (priority / 2).max(1);
        [
            (
                "fast",
                EvmFee::Eip1559 {
                    max_fee_per_gas: base * 2 + tip_fast,
                    max_priority_fee_per_gas: tip_fast,
                },
                "约 15 秒",
                format!(
                    "上限 {:.2} gwei · 小费 {:.2} gwei",
                    (base * 2 + tip_fast) as f64 / GWEI as f64,
                    tip_fast as f64 / GWEI as f64
                ),
            ),
            (
                "normal",
                EvmFee::Eip1559 {
                    max_fee_per_gas: base * 12 / 10 + priority,
                    max_priority_fee_per_gas: priority,
                },
                "约 30 秒",
                format!(
                    "上限 {:.2} gwei · 小费 {:.2} gwei",
                    (base * 12 / 10 + priority) as f64 / GWEI as f64,
                    priority as f64 / GWEI as f64
                ),
            ),
            (
                "economy",
                EvmFee::Eip1559 {
                    max_fee_per_gas: base + tip_economy,
                    max_priority_fee_per_gas: tip_economy,
                },
                "约 1 分钟",
                format!(
                    "上限 {:.2} gwei · 小费 {:.2} gwei",
                    (base + tip_economy) as f64 / GWEI as f64,
                    tip_economy as f64 / GWEI as f64
                ),
            ),
        ]
    } else {
        let price = gas_price.unwrap_or(GWEI / 2).max(GWEI / 10);
        let _ = decimals;
        [
            (
                "fast",
                EvmFee::Legacy {
                    gas_price: price * 12 / 10,
                },
                "约 5 秒",
                format!(
                    "{:.2} gwei · Legacy",
                    (price * 12 / 10) as f64 / GWEI as f64
                ),
            ),
            (
                "normal",
                EvmFee::Legacy { gas_price: price },
                "约 15 秒",
                format!("{:.2} gwei · Legacy", price as f64 / GWEI as f64),
            ),
            (
                "economy",
                EvmFee::Legacy {
                    gas_price: price * 9 / 10,
                },
                "约 1 分钟",
                format!("{:.2} gwei · Legacy", (price * 9 / 10) as f64 / GWEI as f64),
            ),
        ]
    }
}

fn evm_fee_cost(fee: &EvmFee) -> u128 {
    match fee {
        EvmFee::Eip1559 {
            max_fee_per_gas, ..
        } => *max_fee_per_gas * u128::from(NATIVE_TRANSFER_GAS),
        EvmFee::Legacy { gas_price } => *gas_price * u128::from(NATIVE_TRANSFER_GAS),
    }
}

async fn evm_quote(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
) -> ApiResult<QuoteFees> {
    let info = chain.info();
    address::parse_evm_address(to)?;
    let adapter = EvmClient::new(client.clone(), info.rpc);
    let balance = adapter.balance(address).await?;
    let base_fee = if chain.supports_eip1559() {
        adapter.latest_base_fee().await?
    } else {
        None
    };
    let priority = if chain.supports_eip1559() {
        adapter.max_priority_fee().await?
    } else {
        None
    };
    let gas_price = if chain.supports_eip1559() {
        None
    } else {
        Some(adapter.gas_price().await?)
    };

    let levels = evm_fee_levels(chain, base_fee, priority, gas_price);
    let mut normal_fee = 0u128;
    let options: Vec<FeeOption> = levels
        .iter()
        .map(|(level, fee, eta, note)| {
            let cost = evm_fee_cost(fee);
            if *level == "normal" {
                normal_fee = cost;
            }
            FeeOption {
                level: (*level).to_string(),
                label: level_label(level).to_string(),
                fee: format_amount(cost, info.decimals),
                fee_fiat: None,
                eta: (*eta).to_string(),
                note: note.clone(),
                selected: *level == "normal",
            }
        })
        .collect();
    let _ = amount;

    let max_amount = balance.checked_sub(normal_fee).filter(|value| *value > 0);
    Ok(QuoteFees {
        options,
        normal_fee,
        max_amount,
        balance,
    })
}

async fn solana_quote(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
) -> ApiResult<QuoteFees> {
    let info = chain.info();
    address::parse_solana_address(to)?;
    let adapter = SolanaClient::new(client.clone(), info.rpc);
    let balance = u128::from(adapter.balance(address).await?);
    let fee = u128::from(SOLANA_BASE_FEE);
    let _ = amount;
    let options = ["fast", "normal", "economy"]
        .iter()
        .map(|level| FeeOption {
            level: (*level).to_string(),
            label: level_label(level).to_string(),
            fee: format_amount(fee, info.decimals),
            fee_fiat: None,
            eta: "约 1 秒".to_string(),
            note: "固定基础费".to_string(),
            selected: *level == "normal",
        })
        .collect();
    Ok(QuoteFees {
        options,
        normal_fee: fee,
        max_amount: balance.checked_sub(fee),
        balance,
    })
}

async fn quote_fees(
    state: &AppState,
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
) -> ApiResult<QuoteFees> {
    match chain.info().kind {
        ChainKind::Utxo => {
            let settings = state.settings()?;
            utxo_quote(
                client,
                chain,
                address,
                to,
                amount,
                Some(settings.blockchair_api_key.as_str()),
            )
            .await
        }
        ChainKind::Evm => evm_quote(client, chain, address, to, amount).await,
        ChainKind::Ed25519 => solana_quote(client, chain, address, to, amount).await,
        ChainKind::Cryptonote => monero::quote(state, chain, to, amount).await,
    }
}

fn chain_note(chain: ChainId) -> String {
    match chain.info().kind {
        ChainKind::Utxo => "原生 SegWit（Bech32）· 手续费按虚拟字节计价".to_string(),
        ChainKind::Evm => {
            if matches!(chain, ChainId::Ethereum | ChainId::Polygon) {
                "EIP-1559 · 21,000 gas".to_string()
            } else {
                "Legacy 交易（EIP-155）· 21,000 gas".to_string()
            }
        }
        ChainKind::Ed25519 => "Ed25519 系统转账".to_string(),
        ChainKind::Cryptonote => {
            "CryptoNote · RingCT（CLSAG + Bulletproofs+，本地 monero-wallet-rpc 签名）".to_string()
        }
    }
}

pub async fn quote(state: &AppState, request: &QuoteRequest) -> ApiResult<SendQuote> {
    state.touch();
    let account = state.account_for(request.chain)?;
    // Fail fast (and loudly) if the vault locked itself mid-session.
    state.active_secret()?;
    let info = request.chain.info();
    if !info.send_supported() {
        return Err(ApiError::new(
            "SEND_UNSUPPORTED",
            format!("{} 的转账功能即将支持，收款地址已可用", info.name),
        ));
    }
    let client = lycoris_core::network::http_client();
    let amount = parse_amount(&request.amount, info.decimals)?;

    let quote = quote_fees(
        state,
        &client,
        request.chain,
        &account.address,
        &request.to,
        amount,
    )
    .await?;

    let table = load_prices(&client, Some(request.chain)).await;
    let mut options = quote.options;
    for option in options.iter_mut() {
        if let Some(price) = price_of(&table, &info, "usd") {
            if let Ok(units) = parse_units(&option.fee, info.decimals) {
                let value = units as f64 / 10f64.powi(i32::from(info.decimals)) * price;
                option.fee_fiat = Some(format!("≈ ${value:.2}"));
            }
        }
    }

    let requested = amount.unwrap_or_else(|| quote.max_amount.unwrap_or(0));
    let mut can_send = quote.balance >= requested + quote.normal_fee && requested > 0;
    let mut warning = None;

    if info.kind == ChainKind::Utxo
        && requested > 0
        && requested < u128::from(request.chain.dust_limit())
    {
        can_send = false;
        warning = Some(format!(
            "低于灰尘限额（{} {}）",
            request.chain.dust_limit(),
            request.chain.base_unit()
        ));
    }
    if !can_send && warning.is_none() {
        warning = Some(format!(
            "可用余额 {} {}，不足以支付金额与手续费",
            balance_text(request.chain, quote.balance),
            info.symbol
        ));
    }

    Ok(SendQuote {
        chain: request.chain,
        symbol: info.symbol.to_string(),
        decimals: info.decimals,
        balance: balance_text(request.chain, quote.balance),
        max_amount: quote
            .max_amount
            .filter(|value| *value > 0)
            .map(|value| format_amount(value, info.decimals))
            .unwrap_or_else(|| "0".to_string()),
        amount: format_amount(requested, info.decimals),
        to: request.to.clone(),
        fee_options: options,
        can_send,
        warning,
        note: chain_note(request.chain),
    })
}

// -------------------------------------------------------------------- sending

fn activity_item(
    chain: ChainId,
    amount: String,
    fee: String,
    counterparty: String,
    txid: String,
) -> ActivityItem {
    let info = chain.info();
    ActivityItem {
        id: uuid::Uuid::new_v4().to_string(),
        chain,
        direction: Direction::Send,
        amount,
        symbol: info.symbol.to_string(),
        counterparty,
        txid: txid.clone(),
        status: ActivityStatus::Pending,
        fee,
        timestamp: now(),
        explorer_url: format!("{}{}", info.explorer_tx, txid),
    }
}

/// `(txid, fee in smallest units, amount sent in smallest units)`
async fn send_utxo(
    state: &AppState,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
    level: &str,
) -> ApiResult<(String, u64, u128)> {
    let _info = chain.info();
    let secret = state.active_secret()?;
    let client = lycoris_core::network::http_client();
    if chain.legacy_utxo() {
        return send_legacy_utxo(state, chain, address, to, amount, level).await;
    }
    let (adapter, utxos) = utxos_with_fallback(&client, chain, address, None).await?;
    let fees = adapter.fees(chain).await?;
    let fee_rate = fee_rate_for_level(&fees, level);
    let inputs: Vec<UtxoInput> = utxos
        .iter()
        .map(|utxo| UtxoInput::from_parts(&utxo.txid, utxo.vout, utxo.value, utxo.confirmed))
        .collect::<Result<_, _>>()?;
    let target = match amount {
        Some(value) => u64::try_from(value).map_err(|_| Error::InvalidAmount("金额过大".into()))?,
        None => max_sendable(&inputs, fee_rate, chain.dust_limit())?.0,
    };
    if target < chain.dust_limit() {
        return Err(Error::InvalidAmount(format!(
            "低于灰尘限额（{} {}）",
            chain.dust_limit(),
            chain.base_unit()
        ))
        .into());
    }
    let selection = select_utxos(inputs, target, fee_rate, true, chain.dust_limit())?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
    let key = material.secp()?;
    let pubkey = address::compressed_pubkey(key);
    let params = chain
        .utxo_params()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let (_, change) = change_script(&params, &pubkey);
    let recipient = recipient_script(chain, to)?;
    let change_output = if selection.change >= chain.dust_limit() {
        Some((change.as_slice(), selection.change))
    } else {
        None
    };
    let (raw, fee) = build_signed_p2wpkh(
        key,
        &selection.inputs,
        &recipient,
        target,
        change_output,
        fee_rate,
        chain,
    )?;
    let txid = adapter.broadcast(&raw).await?;
    Ok((txid, fee, u128::from(target)))
}

/// Send with a legacy (pre-SegWit) P2PKH input: Dogecoin, Bitcoin Cash and
/// the 2026 P2PKH chains (DASH/FTC via Blockbook, BSV via WhatsOnChain).
async fn send_legacy_utxo(
    state: &AppState,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
    level: &str,
) -> ApiResult<(String, u64, u128)> {
    let _info = chain.info();
    let secret = state.active_secret()?;
    let client = lycoris_core::network::http_client();
    let (adapter, utxos) = utxos_with_fallback(
        &client,
        chain,
        address,
        Some(state.settings()?.blockchair_api_key.as_str()),
    )
    .await?;
    let fee_rate = legacy_fee_levels(chain)
        .iter()
        .find(|(name, _, _)| *name == level)
        .map(|(_, rate, _)| *rate)
        .unwrap_or_else(|| chain.default_fee_rate());

    let inputs: Vec<UtxoInput> = utxos
        .iter()
        .map(|utxo| UtxoInput::from_parts(&utxo.txid, utxo.vout, utxo.value, utxo.confirmed))
        .collect::<Result<_, _>>()?;
    let target = match amount {
        Some(value) => u64::try_from(value).map_err(|_| Error::InvalidAmount("金额过大".into()))?,
        None => max_legacy_sendable(&inputs, fee_rate, chain.dust_limit())?.0,
    };
    if target < chain.dust_limit() {
        return Err(Error::InvalidAmount(format!(
            "低于灰尘限额（{} {}）",
            chain.dust_limit(),
            chain.base_unit()
        ))
        .into());
    }
    let selection = select_legacy_utxos(inputs, target, fee_rate, chain.dust_limit())?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
    let key = material.secp()?;
    let pubkey = address::compressed_pubkey(key);
    let params = chain
        .utxo_params()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let (_, change) = legacy_change_script(&params, &pubkey);
    let recipient = recipient_script(chain, to)?;
    let change_output = if selection.change >= chain.dust_limit() {
        Some((change.as_slice(), selection.change))
    } else {
        None
    };
    let (raw, fee) = build_signed_p2pkh(
        key,
        &selection.inputs,
        &recipient,
        target,
        change_output,
        fee_rate,
        chain,
    )?;
    let txid = adapter.broadcast(&raw).await?;
    Ok((txid, fee, u128::from(target)))
}

async fn send_evm(
    state: &AppState,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
    level: &str,
) -> ApiResult<(String, u64, u128)> {
    let info = chain.info();
    let secret = state.active_secret()?;
    let client = lycoris_core::network::http_client();
    let adapter = EvmClient::new(client.clone(), info.rpc);
    let chain_id = chain
        .evm_chain_id()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let to_bytes = address::parse_evm_address(to)?;
    let balance = adapter.balance(address).await?;

    let (evm_fee, fee_cost) = if matches!(chain, ChainId::Ethereum | ChainId::Polygon) {
        let base = adapter.latest_base_fee().await?.unwrap_or(0).max(GWEI);
        let priority = adapter.max_priority_fee().await?.unwrap_or(GWEI).max(1);
        let tip_fast = (priority * 2).max(GWEI);
        let tip_economy = (priority / 2).max(1);
        let fee = match level {
            "fast" => EvmFee::Eip1559 {
                max_fee_per_gas: base * 2 + tip_fast,
                max_priority_fee_per_gas: tip_fast,
            },
            "economy" => EvmFee::Eip1559 {
                max_fee_per_gas: base + tip_economy,
                max_priority_fee_per_gas: tip_economy,
            },
            _ => EvmFee::Eip1559 {
                max_fee_per_gas: base * 12 / 10 + priority,
                max_priority_fee_per_gas: priority,
            },
        };
        let cost = evm_fee_cost(&fee);
        (fee, cost)
    } else {
        let gas_price = adapter.gas_price().await?.max(GWEI / 10);
        let price = match level {
            "fast" => gas_price * 12 / 10,
            "economy" => gas_price * 9 / 10,
            _ => gas_price,
        };
        let fee = EvmFee::Legacy { gas_price: price };
        let cost = evm_fee_cost(&fee);
        (fee, cost)
    };

    let target = amount.unwrap_or_else(|| balance.saturating_sub(fee_cost));
    if target == 0 {
        return Err(Error::InvalidAmount("金额必须大于零".into()).into());
    }
    if balance < target + fee_cost {
        return Err(Error::InsufficientBalance.into());
    }

    let nonce = adapter.nonce(address).await?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
    let key = material.secp()?;
    let signed = sign_transaction(
        key,
        &EvmCall {
            chain_id,
            nonce,
            gas_limit: NATIVE_TRANSFER_GAS,
            to: to_bytes,
            value: target,
            data: &[],
            fee: evm_fee,
        },
    )?;
    let txid = adapter.send_raw_transaction(&signed.raw_hex).await?;
    Ok((txid, fee_cost as u64, target))
}

async fn send_solana(
    state: &AppState,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
    _level: &str,
) -> ApiResult<(String, u64, u128)> {
    let info = chain.info();
    let secret = state.active_secret()?;
    let client = lycoris_core::network::http_client();
    let adapter = SolanaClient::new(client.clone(), info.rpc);
    let balance = u128::from(adapter.balance(address).await?);
    let to_bytes = address::parse_solana_address(to)?;
    let blockhash = adapter.latest_blockhash().await?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
    let seed = material.ed25519_seed()?;

    let target = amount.unwrap_or_else(|| balance.saturating_sub(u128::from(SOLANA_BASE_FEE)));
    if target == 0 {
        return Err(Error::InvalidAmount("金额必须大于零".into()).into());
    }
    let lamports = u64::try_from(target).map_err(|_| Error::InvalidAmount("金额过大".into()))?;
    let built = build_transfer(&seed, &to_bytes, lamports, &blockhash)?;
    let fee = adapter
        .fee_for_message(&built.message_base64)
        .await?
        .unwrap_or(SOLANA_BASE_FEE);
    if balance < target + u128::from(fee) {
        return Err(Error::InsufficientBalance.into());
    }
    let txid = adapter.send_transaction(&built.transaction_base64).await?;
    Ok((txid, fee, target))
}

pub async fn send(state: &AppState, request: &SendRequest) -> ApiResult<SendResult> {
    state.touch();
    let account = state.account_for(request.chain)?;
    let info = request.chain.info();
    let amount = parse_amount(&request.amount, info.decimals)?;
    let level = request.level.to_lowercase();

    let (txid, fee, sent) = match info.kind {
        ChainKind::Utxo => {
            send_utxo(
                state,
                request.chain,
                &account.address,
                &request.to,
                amount,
                &level,
            )
            .await?
        }
        ChainKind::Evm => {
            send_evm(
                state,
                request.chain,
                &account.address,
                &request.to,
                amount,
                &level,
            )
            .await?
        }
        ChainKind::Ed25519 => {
            send_solana(
                state,
                request.chain,
                &account.address,
                &request.to,
                amount,
                &level,
            )
            .await?
        }
        ChainKind::Cryptonote => {
            monero::send(state, request.chain, &request.to, amount, &level).await?
        }
    };

    let result = SendResult {
        chain: request.chain,
        symbol: info.symbol.to_string(),
        txid: txid.clone(),
        explorer_url: format!("{}{}", info.explorer_tx, txid),
        amount: format_amount(sent, info.decimals),
        fee: format_amount(u128::from(fee), info.decimals),
        to: request.to.clone(),
        timestamp: now(),
    };

    state.add_activity(activity_item(
        request.chain,
        result.amount.clone(),
        result.fee.clone(),
        request.to.clone(),
        txid,
    ))?;

    Ok(result)
}

/// Unused today but kept so currency formatting lives in one place.
#[allow(dead_code)]
fn fiat_string(value: f64, currency: FiatCurrency) -> String {
    match currency {
        FiatCurrency::Usd => format!("${value:.2}"),
        FiatCurrency::Cny => format!("¥{value:.2}"),
    }
}
