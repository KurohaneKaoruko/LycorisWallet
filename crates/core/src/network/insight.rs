//! Insight-style explorer API adapter (`/api/addr/…`, `/api/txs?address=…`,
//! `/api/tx/send`), served by insight.dash.org (DASH) and explorer.firo.org
//! (FIRO).
//!
//! Verified live 2026-11: both hosts answer `200` on balance/utxo/history;
//! FIRO's broadcast (`POST /api/tx/send`, form `rawtx=…`) rejects junk with
//! `400 TX decode failed`, i.e. the endpoint exists. Values are integers in
//! the chain's smallest unit; some builds answer `amount` in coins instead,
//! which is rescaled.
//!
//! The Insight dialect has no schema; hosts drift. Every entry is parsed
//! defensively: a malformed row is skipped, not fatal (same policy as the
//! Blockbook/Esplora parsers).

use crate::chains::ChainId;
use crate::error::{Error, Result};
use crate::network::history::{self, HistoryTx};
use crate::network::utxo::Utxo;
use serde_json::Value;

#[derive(Clone)]
pub struct InsightClient {
    client: reqwest::Client,
    base: String,
}

impl InsightClient {
    /// `base` includes the host's API prefix, e.g.
    /// `https://insight.dash.org/insight-api` or `https://explorer.firo.org/api`.
    pub fn new(client: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
        }
    }

    async fn get_json(&self, path: &str, what: &str) -> Result<Value> {
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
        serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("{what}: invalid JSON: {e}")))
    }

    /// Confirmed + mempool balance of an address, in smallest units. Hosts
    /// answer a bare integer (may arrive as a JSON number or string).
    pub async fn balance(&self, address: &str) -> Result<u64> {
        let value = self
            .get_json(&format!("/addr/{address}/balance"), "balance")
            .await?;
        let total = value
            .as_i64()
            .ok_or_else(|| Error::Network("balance: expected an integer".into()))?;
        u64::try_from(total).map_err(|_| Error::Network("balance: negative balance".into()))
    }

    /// Confirmed + mempool UTXOs (`/addr/{a}/utxo`).
    pub async fn utxos(&self, address: &str) -> Result<Vec<Utxo>> {
        let value = self
            .get_json(&format!("/addr/{address}/utxo"), "utxos")
            .await?;
        let items = value.as_array().cloned().unwrap_or_default();
        let mut out = Vec::with_capacity(items.len());
        for item in items {
            let txid = match item.get("txid").and_then(|v| v.as_str()) {
                Some(txid) if !txid.is_empty() => txid.to_string(),
                _ => continue,
            };
            let vout = match item.get("vout").and_then(|v| v.as_u64()) {
                Some(vout) => vout,
                _ => continue,
            };
            // Prefer integer satoshis; some builds only carry coins.
            let satoshis = match item.get("satoshis").and_then(|v| v.as_u64()) {
                Some(sats) => sats,
                None => match item.get("amount").and_then(|v| v.as_f64()) {
                    Some(coins) if coins >= 0.0 => (coins * 1e8).round() as u64,
                    _ => continue,
                },
            };
            // Missing confirmations = mempool entry in most builds.
            let confirmed = item
                .get("confirmations")
                .and_then(|v| v.as_u64())
                .map(|c| c > 0)
                .unwrap_or(false);
            out.push(Utxo {
                txid,
                vout: u32::try_from(vout)
                    .map_err(|_| Error::Network("utxos: vout overflow".into()))?,
                value: satoshis,
                confirmed,
            });
        }
        Ok(out)
    }

    /// Recent transactions of the address (`/txs?address=…`).
    pub async fn transactions(&self, chain: ChainId, address: &str) -> Result<Vec<HistoryTx>> {
        // The page-size param is the same "silently capped" game blockbook
        // plays; a short page still parses.
        let value = self
            .get_json(
                &format!(
                    "/txs?address={address}&limit={}",
                    history::BLOCKBOOK_PAGE_SIZE
                ),
                "history",
            )
            .await?;
        let items = value
            .get("txs")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        let decimals = chain.info().decimals;
        let scale = 10u128.pow(u32::from(decimals));
        let mut out = Vec::with_capacity(items.len());
        for tx in items {
            let txid = match tx.get("txid").and_then(|v| v.as_str()) {
                Some(txid) if !txid.is_empty() => txid.to_string(),
                _ => continue, // malformed row — keep the neighbours
            };
            // Outgoing when one of the inputs is our address.
            let outgoing = tx
                .get("vin")
                .and_then(|v| v.as_array())
                .map(|vins| {
                    vins.iter().any(|vin| {
                        vin.get("addr")
                            .and_then(|v| v.as_str())
                            .or_else(|| vin.get("address").and_then(|v| v.as_str()))
                            .is_some_and(|a| a == address)
                    })
                })
                .unwrap_or(false);
            let side_amount = if outgoing {
                sum_vouts(&tx, address, false, scale)
            } else {
                sum_vouts(&tx, address, true, scale)
            };
            if !outgoing && side_amount == 0 {
                continue; // neither side — same policy as the blockbook parser
            }
            let timestamp = tx
                .get("time")
                .and_then(|v| v.as_i64())
                .or_else(|| tx.get("blocktime").and_then(|v| v.as_i64()))
                .unwrap_or(0);
            let confirmations = tx
                .get("confirmations")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            out.push(HistoryTx {
                chain,
                txid,
                outgoing,
                amount: side_amount,
                fee: None,
                timestamp,
                confirmations,
            });
        }
        Ok(out)
    }

    /// Broadcast a raw transaction (`POST /tx/send`, form `rawtx=…`).
    pub async fn broadcast(&self, raw_hex: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/tx/send", self.base))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(format!("rawtx={raw_hex}"))
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
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("broadcast: invalid JSON: {e}")))?;
        value
            .get("txid")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .ok_or_else(|| Error::Network("broadcast: response missing txid".into()))
    }
}

/// Sum the matching (or non-matching) vout values of one tx, in smallest
/// units. `ours == true` sums outputs addressed to `address`, `false` the
/// rest (change excluded when summing a send).
fn sum_vouts(tx: &Value, address: &str, ours: bool, scale: u128) -> u128 {
    tx.get("vout")
        .and_then(|v| v.as_array())
        .map(|vouts| {
            vouts
                .iter()
                .filter(|vout| {
                    let matches = vout
                        .get("addresses")
                        .and_then(|v| v.as_array())
                        .map(|addrs| {
                            addrs
                                .iter()
                                .filter_map(|a| a.as_str())
                                .any(|a| a == address)
                        })
                        .unwrap_or(false);
                    matches == ours
                })
                .filter_map(|vout| {
                    vout.get("satoshis")
                        .and_then(|v| v.as_u64())
                        .map(u128::from)
                        .or_else(|| {
                            vout.get("value")
                                .and_then(|v| v.as_f64())
                                .map(|coins| (coins * scale as f64).round() as u128)
                        })
                })
                .sum()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_accepts_integer_and_string_payloads() {
        let bare: Value = serde_json::from_str("123456789").unwrap();
        assert_eq!(bare.as_i64(), Some(123456789));
        let text: Value = serde_json::from_str("\"123456789\"").unwrap();
        // Strings are rejected: hosts answer bare integers per the spec; a
        // string here means an unfamiliar dialect and must fail loudly.
        assert_eq!(text.as_i64(), None);
    }

    #[test]
    fn sum_vouts_prefers_satoshis_and_scales_coins() {
        let tx: Value = serde_json::from_str(
            r#"{"vout":[
                {"satoshis":5000,"addresses":["Xy"]},
                {"value":0.00000123,"addresses":["Xy"]},
                {"value":9.9,"addresses":["Other"]}
            ]}"#,
        )
        .unwrap();
        assert_eq!(sum_vouts(&tx, "Xy", true, 100_000_000), 5000 + 123);
        assert_eq!(sum_vouts(&tx, "Xy", false, 100_000_000), 990_000_000);
    }

    #[test]
    fn outgoing_detection_reads_addr_or_address() {
        let tx: Value = serde_json::from_str(
            r#"{"vin":[{"addr":"Me"}],"vout":[{"satoshis":1,"addresses":["You"]}]}"#,
        )
        .unwrap();
        let hits = tx
            .get("vin")
            .and_then(|v| v.as_array())
            .map(|vins| {
                vins.iter().any(|vin| {
                    vin.get("addr")
                        .and_then(|v| v.as_str())
                        .or_else(|| vin.get("address").and_then(|v| v.as_str()))
                        .is_some_and(|a| a == "Me")
                })
            })
            .unwrap();
        assert!(hits);
    }
}
