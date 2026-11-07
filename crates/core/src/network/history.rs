//! Transaction history for every chain, in one shape.
//!
//! Each adapter pulls the most recent transactions of an address and
//! normalises them into [`HistoryTx`]: direction is decided by whether the
//! address funds the inputs (send) or only receives (receive), amounts are
//! the value that actually moved for *this* address, and confirmations come
//! straight from the source so the UI can promote pending entries.
//!
//! Three wire protocols are spoken here:
//! - Blockbook v2 (`/api/v2/address/…?details=txs`) — the UTXO chains added
//!   in 2026 plus Bitcoin/Litecoin when they are served by a Blockbook host.
//! - Esplora (`/address/…/txs`) — mempool.space style hosts.
//! - Solana JSON-RPC (`getSignaturesForAddress` + `getTransaction`).

use crate::chains::ChainId;
use crate::error::{Error, Result};
use serde::Deserialize;

/// One normalised transaction, as shown in the activity list.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryTx {
    pub chain: ChainId,
    pub txid: String,
    /// `true` when our address is among the inputs.
    pub outgoing: bool,
    /// Value that moved for our side, in the chain's smallest unit:
    /// received by us, or sent to others (change excluded).
    pub amount: u128,
    /// Total network fee in smallest units, when the source reports it.
    pub fee: Option<u128>,
    /// Block time (unix seconds); mempool entries carry the seen time.
    pub timestamp: i64,
    /// Confirmations; `0` means still in the mempool.
    pub confirmations: u64,
}

impl HistoryTx {
    pub fn explorer_url(&self) -> String {
        format!("{}{}", self.chain.info().explorer_tx, self.txid)
    }
}

/* ------------------------------------------------------------------ blockbook */

/// Blockbook v2 shapes we care about (see trezor/blockbook openapi.yaml).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockbookAddress {
    #[serde(default)]
    transactions: Vec<BlockbookTx>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockbookTx {
    txid: String,
    #[serde(default)]
    vin: Vec<BlockbookVin>,
    #[serde(default)]
    vout: Vec<BlockbookVout>,
    #[serde(default)]
    confirmations: u64,
    #[serde(default)]
    block_time: i64,
    /// Fees as a decimal string.
    #[serde(default)]
    fees: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockbookVin {
    #[serde(default)]
    addresses: Vec<String>,
    #[serde(default)]
    is_address: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BlockbookVout {
    #[serde(default)]
    addresses: Option<Vec<String>>,
    #[serde(default)]
    is_address: bool,
    #[serde(default)]
    value: String,
}

/// Parses a Blockbook amount. Canonical instances return integer smallest
/// units as strings (`"1000000000"`); some old ones answer with a decimal
/// point (`"1.5"`), which is tolerated.
pub fn parse_amount(value: &str, decimals: u8) -> Option<u128> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0);
    }
    let (whole, frac) = match trimmed.split_once('.') {
        // Canonical AmountString: integer smallest units, no decimal point.
        None => return trimmed.parse::<u128>().ok(),
        Some(parts) => parts,
    };
    if frac.len() > usize::from(decimals) {
        return None; // more precision than the chain has: reject
    }
    let whole: u128 = if whole.is_empty() {
        0
    } else {
        whole.parse().ok()?
    };
    let scale = 10u128.checked_pow(u32::from(decimals))?;
    if frac.is_empty() {
        return whole.checked_mul(scale);
    }
    let frac_value: u128 = frac.parse().ok()?;
    let frac_scaled =
        frac_value.checked_mul(10u128.checked_pow(u32::from(decimals) - frac.len() as u32)?)?;
    whole.checked_mul(scale)?.checked_add(frac_scaled)
}

/// Own-address matching. Blockbook echoes the address exactly as requested
/// for base58 chains; cashaddr hosts may answer with the prefixed form.
fn owns(address: &str, candidate: &str) -> bool {
    address == candidate
        || candidate
            .rsplit(':')
            .next()
            .is_some_and(|tail| tail == address.rsplit(':').next().unwrap_or(address))
}

pub fn parse_blockbook_txs(payload: &str, chain: ChainId, address: &str) -> Result<Vec<HistoryTx>> {
    let page: BlockbookAddress =
        serde_json::from_str(payload).map_err(|e| Error::Network(format!("history: {e}")))?;
    let decimals = chain.info().decimals;
    let mut out = Vec::with_capacity(page.transactions.len());
    for tx in page.transactions {
        // One malformed entry (indexer hiccup, partial prune) must not throw
        // away the whole page — skip it and keep the neighbours.
        if tx.txid.is_empty() {
            continue;
        }
        let funded_by_us = tx
            .vin
            .iter()
            .any(|vin| vin.is_address && vin.addresses.iter().any(|a| owns(address, a)));
        let received: u128 = tx
            .vout
            .iter()
            .filter(|v| {
                v.is_address
                    && v.addresses
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .any(|a| owns(address, a))
            })
            .filter_map(|v| parse_amount(&v.value, decimals))
            .sum();
        let spent_to_others: u128 = tx
            .vout
            .iter()
            .filter(|v| {
                v.is_address
                    && !v
                        .addresses
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .any(|a| owns(address, a))
            })
            .filter_map(|v| parse_amount(&v.value, decimals))
            .sum();
        let (outgoing, amount) = if funded_by_us {
            (true, spent_to_others)
        } else if received > 0 {
            (false, received)
        } else {
            continue; // neither side — skip
        };
        let fee = parse_amount(&tx.fees, decimals);
        out.push(HistoryTx {
            chain,
            txid: tx.txid,
            outgoing,
            amount,
            fee,
            timestamp: if tx.block_time > 0 { tx.block_time } else { 0 },
            confirmations: tx.confirmations,
        });
    }
    Ok(out)
}

/// Same page, several times: the adapter asks for `pageSize=100` but older
/// Blockbook instances cap at 25 and answer `200` with a short page. The
/// parsing side does not care (it normalises whatever comes back), so there is
/// nothing to branch on here — the size lives in the request URL only.
pub const BLOCKBOOK_PAGE_SIZE: u32 = 100;

/* -------------------------------------------------------------------- esplora */

#[derive(Debug, Deserialize)]
struct EsploraTx {
    txid: String,
    #[serde(default)]
    status: EsploraStatus,
    #[serde(default)]
    vin: Vec<EsploraVin>,
    #[serde(default)]
    vout: Vec<EsploraVout>,
    #[serde(default)]
    fee: i64,
}

/// Esplora 的 wire 字段是 snake_case(`block_time`/`scriptpubkey_address`),
/// 不是 Blockbook 那套 camelCase——此前误加 `rename_all = "camelCase"`,
/// 字段全部落回 default,地址永远匹配不上,BTC/LTC 的链上历史整页被跳过。
#[derive(Debug, Deserialize, Default)]
struct EsploraStatus {
    #[serde(default)]
    confirmed: bool,
    #[serde(default)]
    block_time: i64,
}

#[derive(Debug, Deserialize)]
struct EsploraVin {
    #[serde(default)]
    prevout: Option<EsploraPrevout>,
}

#[derive(Debug, Deserialize)]
struct EsploraPrevout {
    #[serde(default)]
    scriptpubkey_address: Option<String>,
}

#[derive(Debug, Deserialize)]
struct EsploraVout {
    #[serde(default)]
    scriptpubkey_address: Option<String>,
    #[serde(default)]
    value: u64,
}

pub async fn fetch_esplora(
    client: &reqwest::Client,
    base: &str,
    chain: ChainId,
    address: &str,
) -> Result<Vec<HistoryTx>> {
    let url = format!("{}/address/{}/txs", base.trim_end_matches('/'), address);
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| Error::Network(format!("history: {e}")))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| Error::Network(format!("history: invalid response: {e}")))?;
    if !status.is_success() {
        return Err(Error::Network(format!("history: HTTP {status}")));
    }
    parse_esplora_txs(&text, chain, address)
}

/// Parsing half of [`fetch_esplora`], exposed for offline tests.
pub fn parse_esplora_txs(payload: &str, chain: ChainId, address: &str) -> Result<Vec<HistoryTx>> {
    let txs: Vec<EsploraTx> =
        serde_json::from_str(payload).map_err(|e| Error::Network(format!("history: {e}")))?;
    let mut out = Vec::with_capacity(txs.len());
    for tx in txs {
        // Same rule as the Blockbook side: one malformed entry skips alone.
        if tx.txid.is_empty() {
            continue;
        }
        let funded_by_us = tx.vin.iter().any(|vin| {
            vin.prevout
                .as_ref()
                .and_then(|p| p.scriptpubkey_address.as_deref())
                .is_some_and(|a| owns(address, a))
        });
        let received: u128 = tx
            .vout
            .iter()
            .filter(|v| v.scriptpubkey_address.as_deref() == Some(address))
            .map(|v| u128::from(v.value))
            .sum();
        let spent_to_others: u128 = tx
            .vout
            .iter()
            .filter(|v| {
                v.scriptpubkey_address
                    .as_deref()
                    .is_some_and(|a| a != address)
            })
            .map(|v| u128::from(v.value))
            .sum();
        let (outgoing, amount) = if funded_by_us {
            (true, spent_to_others)
        } else if received > 0 {
            (false, received)
        } else {
            continue;
        };
        out.push(HistoryTx {
            chain,
            txid: tx.txid,
            outgoing,
            amount,
            fee: (tx.fee > 0).then(|| u128::try_from(tx.fee).unwrap_or(0)),
            timestamp: tx.status.block_time,
            confirmations: u64::from(tx.status.confirmed),
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_parse_amounts() {
        assert_eq!(parse_amount("0.001", 8), Some(100_000));
        // Canonical wire format: integer smallest units, no decimal point.
        assert_eq!(parse_amount("1", 8), Some(1));
        assert_eq!(parse_amount("1000000000", 8), Some(1_000_000_000));
        assert_eq!(parse_amount("1.5", 8), Some(150_000_000));
        assert_eq!(parse_amount("", 8), Some(0));
        assert_eq!(parse_amount("0.000000001", 8), None);
    }

    #[test]
    fn parses_blockbook_page() {
        let payload = r#"{
          "transactions": [
            {
              "txid": "aa",
              "vin": [{"addresses": ["XOTHER"], "isAddress": true, "value": "1.0"}],
              "vout": [
                {"addresses": ["XOWN"], "isAddress": true, "value": "0.9"},
                {"addresses": ["XBACK"], "isAddress": true, "value": "0.1"}
              ],
              "blockHeight": 100,
              "confirmations": 3,
              "blockTime": 1700000000,
              "fees": "0.001"
            },
            {
              "txid": "bb",
              "vin": [{"addresses": ["XOWN"], "isAddress": true, "value": "2.0"}],
              "vout": [
                {"addresses": ["XOTHER"], "isAddress": true, "value": "1.99"}
              ],
              "blockHeight": 200,
              "confirmations": 1,
              "blockTime": 1700000100,
              "fees": "0.01"
            }
          ]
        }"#;
        let txs = parse_blockbook_txs(payload, ChainId::Dash, "XOWN").unwrap();
        assert_eq!(txs.len(), 2);
        assert_eq!(txs[0].txid, "aa");
        assert!(!txs[0].outgoing);
        assert_eq!(txs[0].amount, 90_000_000);
        assert_eq!(txs[0].confirmations, 3);
        assert_eq!(txs[0].fee, Some(100_000));
        assert!(txs[1].outgoing);
        assert_eq!(txs[1].amount, 199_000_000);
        assert_eq!(
            txs[1].explorer_url(),
            "https://insight.dash.org/tx/bb",
            "DASH explorer links follow the new insight.dash.org host"
        );
    }

    #[test]
    fn skips_unrelated_transactions() {
        let payload = r#"{"transactions": [{
          "txid": "cc",
          "vin": [{"addresses": ["A"], "isAddress": true, "value": "1"}],
          "vout": [{"addresses": ["B"], "isAddress": true, "value": "1"}]
        }]}"#;
        let txs = parse_blockbook_txs(payload, ChainId::Dash, "XOWN").unwrap();
        assert!(txs.is_empty());
    }

    #[test]
    fn blockbook_page_without_transactions_key_is_empty_not_an_error() {
        // Fresh addresses answer `{"address": "…", "txs": 0}` with no
        // transactions array at all — the serde default handles it, but this
        // pins the "empty page is success" contract.
        let txs =
            parse_blockbook_txs(r#"{"address":"XOWN","txs":0}"#, ChainId::Dash, "XOWN").unwrap();
        assert!(txs.is_empty());
    }

    #[test]
    fn skips_single_malformed_blockbook_entry() {
        // Indexer hiccup: an entry with no txid sits between two good ones.
        // The page must survive with the good neighbours intact.
        let payload = r#"{"transactions": [
            {"txid": "good1", "vin": [{"addresses": ["XOWN"], "isAddress": true}],
             "vout": [{"addresses": ["XOTHER"], "isAddress": true, "value": "5"}],
             "confirmations": 2, "blockTime": 1700000000},
            {"txid": "", "vin": [], "vout": []},
            {"txid": "good2", "vin": [], "vout": [{"addresses": ["XOWN"], "isAddress": true, "value": "1"}],
             "confirmations": 9, "blockTime": 1700000500}
        ]}"#;
        let txs = parse_blockbook_txs(payload, ChainId::Dash, "XOWN").unwrap();
        assert_eq!(txs.len(), 2);
        assert_eq!(txs[0].txid, "good1");
        // Canonical AmountString "5" = 5 smallest units, not 5 coins.
        assert_eq!(txs[0].amount, 5);
        assert_eq!(txs[1].txid, "good2");
        assert_eq!(txs[1].amount, 1);
    }

    #[test]
    fn blockbook_dust_outputs_with_undecodable_amount_are_skipped_not_fatal() {
        // `parse_amount` returns None on out-of-range precision; that output
        // drops out of the sum instead of failing the whole tx row.
        let payload = r#"{"transactions": [{
          "txid": "dd",
          "vin": [{"addresses": ["XOWN"], "isAddress": true}],
          "vout": [
            {"addresses": ["XOWN"], "isAddress": true, "value": "0.000000001"},
            {"addresses": ["XOTHER"], "isAddress": true, "value": "2.5"}
          ],
          "confirmations": 1, "blockTime": 1700000200
        }]}"#;
        let txs = parse_blockbook_txs(payload, ChainId::Dash, "XOWN").unwrap();
        assert_eq!(txs.len(), 1);
        // The 1-ct dust output was undecodable, so only the 2.5 side counts.
        assert!(txs[0].outgoing);
        assert_eq!(txs[0].amount, 250_000_000);
    }

    #[test]
    fn parses_esplora_page() {
        // mempool.space shape: prevout addresses, status.confirmed flag.
        let payload = r#"[
          {
            "txid": "esplora1",
            "fee": 500,
            "status": {"confirmed": true, "block_time": 1700000000},
            "vin": [
              {"prevout": {"scriptpubkey_address": "XOTHER", "value": 3000}},
              {"prevout": {"scriptpubkey_address": "XOWN", "value": 7000}}
            ],
            "vout": [
              {"scriptpubkey_address": "XOWN", "value": 2500},
              {"scriptpubkey_address": "XOTHER", "value": 4400}
            ]
          },
          {
            "txid": "esplora2",
            "fee": 300,
            "status": {"confirmed": false, "block_time": 0},
            "vin": [],
            "vout": [{"scriptpubkey_address": "XOWN", "value": 1234}]
          }
        ]"#;
        let txs = super::parse_esplora_txs(payload, ChainId::Bitcoin, "XOWN").unwrap();
        assert_eq!(txs.len(), 2);
        // Self-send with change: outgoing = spent_to_others only.
        assert!(txs[0].outgoing);
        assert_eq!(txs[0].amount, 4400);
        assert_eq!(txs[0].fee, Some(500));
        assert_eq!(txs[0].confirmations, 1);
        // Mempool receive: confirmed=false → confirmations 0.
        assert!(!txs[1].outgoing);
        assert_eq!(txs[1].amount, 1234);
        assert_eq!(txs[1].confirmations, 0);
    }

    #[test]
    fn esplora_page_skips_malformed_entry() {
        let payload = r#"[
          {"txid": "", "vout": [{"scriptpubkey_address": "XOWN", "value": 1}]},
          {"txid": "ok", "vout": [{"scriptpubkey_address": "XOWN", "value": 42}]}
        ]"#;
        let txs = super::parse_esplora_txs(payload, ChainId::Bitcoin, "XOWN").unwrap();
        assert_eq!(txs.len(), 1);
        assert_eq!(txs[0].txid, "ok");
    }

    #[test]
    fn esplora_page_neither_side_is_skipped() {
        let payload = r#"[
          {"txid": "unrelated", "vin": [], "vout": [{"scriptpubkey_address": "XOTHER", "value": 9}]}
        ]"#;
        let txs = super::parse_esplora_txs(payload, ChainId::Bitcoin, "XOWN").unwrap();
        assert!(txs.is_empty());
    }

    #[test]
    fn esplora_rejects_object_root() {
        // The endpoint answers an array; an object means a wrong/hostile host.
        assert!(super::parse_esplora_txs(r#"{"txs":[]}"#, ChainId::Bitcoin, "XOWN").is_err());
    }
}
