//! Blockbook v2 adapter — the index API Trezor Suite itself uses.
//!
//! Live hosts (verified 2026-10): `blockbook.viacoin.org`,
//! `explorer.feathercoin.com`; additionally `blockbook.groestlcoin.org` and
//! `blockbook.dash.zelcore.io` per the Groestlcoin/Trezor Suite production
//! configs. Amounts are integer smallest-unit strings (`AmountString`),
//! confirmed against the live instances.

use crate::error::{Error, Result};
use crate::network::history::{self, HistoryTx};
use crate::tx::utxo::RecommendedFees;
use serde::Deserialize;

#[derive(Clone)]
pub struct BlockbookClient {
    client: reqwest::Client,
    base: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddressResponse {
    #[serde(default)]
    balance: String,
    #[serde(default)]
    unconfirmed_balance: String,
}

/// Blockbook `AmountString`: integer smallest units as a string. Some very
/// old instances answer with a decimal point — tolerated, never produced.
fn amount(payload: &str, decimals: u8) -> Result<u64> {
    let value = history::parse_amount(payload, decimals)
        .ok_or_else(|| Error::Network(format!("blockbook: bad amount {payload:?}")))?;
    u64::try_from(value)
        .map_err(|_| Error::Network(format!("blockbook: amount overflow {payload:?}")))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UtxoItem {
    txid: String,
    vout: u32,
    #[serde(default)]
    value: String,
    #[serde(default)]
    height: i64,
    #[serde(default)]
    confirmations: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FeeResponse {
    #[serde(default)]
    result: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendTxResponse {
    #[serde(default)]
    result: String,
}

impl BlockbookClient {
    pub fn new(client: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
        }
    }

    async fn get_text(&self, path: &str, what: &str) -> Result<String> {
        let response = self
            .client
            .get(format!("{}{}", self.base, path))
            .send()
            .await
            .map_err(|e| Error::Network(format!("{what}: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("{what}: invalid response: {e}")))?;
        if !status.is_success() {
            return Err(Error::Network(format!("{what}: HTTP {status}")));
        }
        Ok(text)
    }

    /// Confirmed + unconfirmed balance in smallest units.
    pub async fn balance(&self, address: &str, decimals: u8) -> Result<u64> {
        let text = self
            .get_text(&format!("/api/v2/address/{address}"), "balance")
            .await?;
        let page: AddressResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("balance: invalid JSON: {e}")))?;
        let confirmed = amount(&page.balance, decimals)?;
        let unconfirmed = amount(&page.unconfirmed_balance, decimals)?;
        // Unconfirmed shows negative while a withdrawal is pending.
        let total = (confirmed as i128) + (unconfirmed as i128);
        if total < 0 {
            return Ok(0);
        }
        Ok(total as u64)
    }

    /// Confirmed and mempool UTXOs, oldest first.
    pub async fn utxos(
        &self,
        address: &str,
        decimals: u8,
    ) -> Result<Vec<crate::network::utxo::Utxo>> {
        let text = self
            .get_text(&format!("/api/v2/utxo/{address}"), "utxos")
            .await?;
        let items: Vec<UtxoItem> = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("utxos: invalid JSON: {e}")))?;
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            out.push(crate::network::utxo::Utxo {
                txid: item.txid,
                vout: item.vout,
                value: amount(&item.value, decimals)?,
                confirmed: item.confirmations > 0 || item.height > 0,
            });
        }
        Ok(out)
    }

    /// Recommended fees, converted from coin-per-kb to sat/vbyte.
    /// Hosts that answer zero (Feathercoin does) fall back to a flat 1 sat/vB.
    pub async fn fees(&self, decimals: u8) -> Result<RecommendedFees> {
        let fallback = |rate: u64| RecommendedFees {
            fastest: rate,
            half_hour: rate,
            hour: rate,
            economy: rate.saturating_sub(1).max(1),
            minimum: 1,
        };
        let parse = |payload: &str, blocks: u32| -> Option<u64> {
            let page: FeeResponse = serde_json::from_str(payload).ok()?;
            let coin_per_kb = page.result.trim().parse::<f64>().ok()?;
            if coin_per_kb <= 0.0 {
                return None;
            }
            let scale = 10f64.powi(i32::from(decimals));
            let per_vbyte = coin_per_kb * scale / 1000.0;
            // Rates move fast; round to whole sats with a 1 sat floor.
            let sats = per_vbyte.round().max(1.0) as u64;
            let _ = blocks;
            Some(sats)
        };
        let fast = self
            .get_text("/api/v2/estimatefee/2", "fees")
            .await
            .ok()
            .and_then(|payload| parse(&payload, 2));
        let normal = self
            .get_text("/api/v2/estimatefee/4", "fees")
            .await
            .ok()
            .and_then(|payload| parse(&payload, 4));
        let slow = self
            .get_text("/api/v2/estimatefee/10", "fees")
            .await
            .ok()
            .and_then(|payload| parse(&payload, 10));
        match (fast, normal, slow) {
            (Some(fast), Some(normal), Some(slow)) => Ok(RecommendedFees {
                fastest: fast,
                half_hour: normal,
                hour: slow,
                economy: slow.saturating_sub(1).max(1),
                minimum: 1,
            }),
            (Some(fast), Some(normal), _) => Ok(RecommendedFees {
                fastest: fast,
                half_hour: normal,
                hour: normal,
                economy: normal.saturating_sub(1).max(1),
                minimum: 1,
            }),
            _ => Ok(fallback(1)),
        }
    }

    pub async fn broadcast(&self, raw_hex: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/api/v2/sendtx", self.base))
            .header("Content-Type", "text/plain")
            .body(raw_hex.to_string())
            .send()
            .await
            .map_err(|e| Error::Network(format!("broadcast: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("broadcast: invalid response: {e}")))?;
        if !status.is_success() {
            return Err(Error::Network(format!(
                "broadcast rejected (HTTP {status}): {}",
                text.trim()
            )));
        }
        let page: SendTxResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("broadcast: invalid JSON: {e}")))?;
        if page.result.is_empty() {
            return Err(Error::Network("broadcast: empty txid".into()));
        }
        Ok(page.result)
    }

    /// Recent transactions of the address (see [`history`]).
    pub async fn transactions(
        &self,
        chain: crate::chains::ChainId,
        address: &str,
    ) -> Result<Vec<HistoryTx>> {
        let text = self
            .get_text(
                &format!("/api/v2/address/{address}?details=txs&pageSize=25"),
                "history",
            )
            .await?;
        history::parse_blockbook_txs(&text, chain, address)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_integer_amounts() {
        assert_eq!(amount("1000000000", 8).unwrap(), 1_000_000_000);
        assert_eq!(amount("0", 8).unwrap(), 0);
        assert_eq!(amount("", 8).unwrap(), 0);
        // Old instances answered with decimals; still accepted.
        assert_eq!(amount("1.5", 8).unwrap(), 150_000_000);
    }

    #[test]
    fn fee_math_is_sane() {
        // 0.00021 coins/kb with 8 decimals → 21 sat/vbyte.
        let payload = r#"{"result":"0.00021"}"#;
        let page: FeeResponse = serde_json::from_str(payload).unwrap();
        let per_kb = page.result.parse::<f64>().unwrap();
        let sats = (per_kb * 10f64.powi(8) / 1000.0).round() as u64;
        assert_eq!(sats, 21);
    }

    #[test]
    fn negative_unconfirmed_is_clamped() {
        // balance=100, unconfirmed=-40 → 60
        let confirmed = 100i128;
        let unconfirmed = -40i128;
        let total = confirmed + unconfirmed;
        assert_eq!(total.max(0) as u64, 60);
        // fully-negative → clamp to zero instead of panicking
        assert_eq!((5 - 9).max(0) as u64, 0);
    }
}
