//! Balances, send quotes and transaction broadcast for every supported chain.
//!
//! All network traffic and all signing happen here; the frontend only ever
//! sees addresses, formatted amounts and transaction ids.

use crate::error::{ApiError, ApiResult};
use crate::state::AppState;
use lycoris_core::address;
use lycoris_core::chains::{ChainId, ChainInfo, ChainKind};
use lycoris_core::error::Error;
use lycoris_core::models::{
    ActivityItem, ActivityStatus, AssetBalance, Direction, FeeOption, FiatCurrency, Portfolio,
    SendQuote, SendResult,
};
use lycoris_core::network::{evm::EvmClient, prices, solana::SolanaClient, utxo::UtxoClient};
use lycoris_core::tx::evm::{sign_transaction, EvmCall, EvmFee};
use lycoris_core::tx::solana::build_transfer;
use lycoris_core::tx::utxo::{
    build_signed_p2wpkh, change_script, fee_rate_for_level, max_sendable, recipient_script,
    select_utxos, UtxoInput, DUST_LIMIT,
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

fn format_amount(value: u128, decimals: u8) -> String {
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
        None => ChainId::ALL
            .iter()
            .map(|chain| chain.info().price_id)
            .collect(),
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

async fn fetch_balance(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
) -> Result<u128, String> {
    match chain.info().kind {
        ChainKind::Utxo => UtxoClient::new(client.clone(), chain.info().rpc)
            .balance(address)
            .await
            .map(u128::from)
            .map_err(|error| error.to_string()),
        ChainKind::Evm => EvmClient::new(client.clone(), chain.info().rpc)
            .balance(address)
            .await
            .map_err(|error| error.to_string()),
        ChainKind::Ed25519 => SolanaClient::new(client.clone(), chain.info().rpc)
            .balance(address)
            .await
            .map(u128::from)
            .map_err(|error| error.to_string()),
    }
}

pub async fn refresh(state: &AppState) -> ApiResult<Portfolio> {
    state.touch();
    let accounts = state.accounts()?;
    let client = lycoris_core::network::http_client();
    let table = load_prices(&client, None).await;

    let mut tasks = tokio::task::JoinSet::new();
    for account in accounts {
        let client = client.clone();
        let chain = account.chain;
        tasks.spawn(async move {
            let result = fetch_balance(&client, chain, &account.address).await;
            (chain, account.address, result)
        });
    }

    let mut assets = Vec::new();
    let mut warnings = Vec::new();
    while let Some(joined) = tasks.join_next().await {
        let Ok((chain, address, result)) = joined else {
            continue;
        };
        let info = chain.info();
        let asset = match result {
            Ok(amount) => {
                let mut asset = asset_from(chain, address, Some(amount), None);
                asset.price_usd = price_of(&table, &info, "usd");
                asset.price_cny = price_of(&table, &info, "cny");
                let human = amount as f64 / 10f64.powi(i32::from(info.decimals));
                asset.value_usd = asset.price_usd.map(|price| price * human);
                asset.value_cny = asset.price_cny.map(|price| price * human);
                asset
            }
            Err(message) => {
                warnings.push(format!("{} 余额获取失败：{message}", info.name));
                asset_from(chain, address, None, Some(message))
            }
        };
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

    let total_usd = assets.iter().filter_map(|asset| asset.value_usd).reduce(|a, b| a + b);
    let total_cny = assets.iter().filter_map(|asset| asset.value_cny).reduce(|a, b| a + b);

    Ok(Portfolio {
        assets,
        total_usd,
        total_cny,
        fetched_at: now(),
        warnings,
    })
}

// -------------------------------------------------------------------- quoting

struct QuoteFees {
    options: Vec<FeeOption>,
    /// Cost of the default ("normal") level, in the chain's smallest unit.
    normal_fee: u128,
    max_amount: Option<u128>,
    balance: u128,
}

/// Parse a user-entered amount; `None` means "max".
fn parse_amount(input: &str, decimals: u8) -> Result<Option<u128>, ApiError> {
    match input.trim() {
        "" | "max" | "MAX" => Ok(None),
        value => Ok(Some(parse_units(value, decimals)?)),
    }
}

async fn utxo_quote(
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
) -> ApiResult<QuoteFees> {
    let info = chain.info();
    recipient_script(chain, to)?;
    let adapter = UtxoClient::new(client.clone(), info.rpc);
    let fees = adapter.fees().await?;
    let utxos = adapter.utxos(address).await?;
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
            Some(value) => select_utxos(inputs.clone(), value, rate, true)
                .map(|selection| selection.fee)
                .unwrap_or(0),
            None => max_sendable(&inputs, rate).map(|(_, fee)| fee).unwrap_or(0),
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
            note: format!("{rate} 聪/vB"),
            selected: level == "normal",
        });
    }

    let max_amount = max_sendable(&inputs, fee_rate_for_level(&fees, "normal"))
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
                format!("{:.2} gwei · Legacy", (price * 12 / 10) as f64 / GWEI as f64),
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
    let base_fee = if matches!(chain, ChainId::Ethereum | ChainId::Polygon) {
        adapter.latest_base_fee().await?
    } else {
        None
    };
    let priority = if matches!(chain, ChainId::Ethereum | ChainId::Polygon) {
        adapter.max_priority_fee().await?
    } else {
        None
    };
    let gas_price = if matches!(chain, ChainId::Ethereum | ChainId::Polygon) {
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
    client: &reqwest::Client,
    chain: ChainId,
    address: &str,
    to: &str,
    amount: Option<u128>,
) -> ApiResult<QuoteFees> {
    match chain.info().kind {
        ChainKind::Utxo => utxo_quote(client, chain, address, to, amount).await,
        ChainKind::Evm => evm_quote(client, chain, address, to, amount).await,
        ChainKind::Ed25519 => solana_quote(client, chain, address, to, amount).await,
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
    }
}

pub async fn quote(state: &AppState, request: &QuoteRequest) -> ApiResult<SendQuote> {
    state.touch();
    let account = state.account_for(request.chain)?;
    // Fail fast (and loudly) if the vault locked itself mid-session.
    state.active_secret()?;
    let info = request.chain.info();
    let client = lycoris_core::network::http_client();
    let amount = parse_amount(&request.amount, info.decimals)?;

    let quote = quote_fees(
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
                let value =
                    units as f64 / 10f64.powi(i32::from(info.decimals)) * price;
                option.fee_fiat = Some(format!("≈ ${value:.2}"));
            }
        }
    }

    let requested = amount.unwrap_or_else(|| quote.max_amount.unwrap_or(0));
    let mut can_send = quote.balance >= requested + quote.normal_fee && requested > 0;
    let mut warning = None;

    if info.kind == ChainKind::Utxo && requested > 0 && requested < u128::from(DUST_LIMIT) {
        can_send = false;
        warning = Some(format!("低于灰尘限额（{DUST_LIMIT} 聪）"));
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
    let info = chain.info();
    let secret = state.active_secret()?;
    let client = lycoris_core::network::http_client();
    let adapter = UtxoClient::new(client.clone(), info.rpc);
    let fees = adapter.fees().await?;
    let fee_rate = fee_rate_for_level(&fees, level);
    let utxos = adapter.utxos(address).await?;
    let inputs: Vec<UtxoInput> = utxos
        .iter()
        .map(|utxo| UtxoInput::from_parts(&utxo.txid, utxo.vout, utxo.value, utxo.confirmed))
        .collect::<Result<_, _>>()?;
    let target = match amount {
        Some(value) => u64::try_from(value).map_err(|_| Error::InvalidAmount("金额过大".into()))?,
        None => max_sendable(&inputs, fee_rate)?.0,
    };
    if target < DUST_LIMIT {
        return Err(Error::InvalidAmount(format!("低于灰尘限额（{DUST_LIMIT} 聪）")).into());
    }
    let selection = select_utxos(inputs, target, fee_rate, true)?;
    let material = lycoris_core::derive::derive_private_key(&secret, chain)?;
    let key = material.secp()?;
    let pubkey = address::compressed_pubkey(key);
    let params = chain
        .utxo_params()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let (_, change) = change_script(&params, &pubkey);
    let recipient = recipient_script(chain, to)?;
    let change_output = if selection.change >= DUST_LIMIT {
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
    let lamports =
        u64::try_from(target).map_err(|_| Error::InvalidAmount("金额过大".into()))?;
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
