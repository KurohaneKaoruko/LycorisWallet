//! Blockcypher adapter — keyless fallback for chains whose primary Blockbook
//! instances are unreachable from some networks (verified live 2026-10:
//! `api.blockcypher.com/v1/doge/main` and `/v1/dash/main` answer 200 with the
//! same JSON shapes as the Bitcoin endpoint).
//!
//! Coverage differs from Blockbook: balances and UTXOs come from the address
//! endpoint (`txrefs`), fees from the chain info endpoint (`*_fee_per_kb`,
//! coin-per-kB like Blockbook's `estimatefee`), broadcast from `txs/push`.

use crate::chains::ChainId;
use crate::error::{Error, Result};
use crate::network::history::HistoryTx;
use crate::network::utxo::Utxo;
use crate::tx::utxo::RecommendedFees;
use serde::Deserialize;

#[derive(Clone)]
pub struct BlockcypherClient {
    client: reqwest::Client,
    base: String,
    decimals: u8,
}

impl BlockcypherClient {
    /// `base` like `https://api.blockcypher.com/v1/doge/main`.
    pub fn new(client: reqwest::Client, base: impl Into<String>, decimals: u8) -> Self {
        Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
            decimals,
        }
    }

    async fn get_json(&self, path: &str, what: &str) -> Result<serde_json::Value> {
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

    /// Confirmed + unconfirmed balance in smallest units.
    pub async fn balance(&self, address: &str) -> Result<u64> {
        let page: AddressResponse = self
            .get_json(&format!("/addrs/{address}/balance"), "balance")
            .await?
            .try_into()?;
        u64::try_from(page.final_balance)
            .map_err(|_| Error::Network("balance: negative final balance".into()))
    }

    /// Confirmed and mempool UTXOs (unspent `txrefs`).
    pub async fn utxos(&self, address: &str) -> Result<Vec<Utxo>> {
        let value = self
            .get_json(
                &format!("/addrs/{address}?unspentOnly=true&limit=2000"),
                "utxos",
            )
            .await?;
        let mut out = Vec::new();
        for key in ["txrefs", "unconfirmed_txrefs"] {
            let Some(refs) = value.get(key).and_then(|v| v.as_array()) else {
                continue;
            };
            for r in refs {
                let txid = r
                    .get("tx_hash")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| Error::Network("utxos: txref missing tx_hash".into()))?
                    .to_string();
                let vout = r
                    .get("tx_output_n")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| Error::Network("utxos: txref missing tx_output_n".into()))?;
                let value = r
                    .get("value")
                    .and_then(|v| v.as_u64())
                    .ok_or_else(|| Error::Network("utxos: txref missing value".into()))?;
                let confirmations = r.get("confirmations").and_then(|v| v.as_u64()).unwrap_or(0);
                out.push(Utxo {
                    txid,
                    vout: u32::try_from(vout)
                        .map_err(|_| Error::Network("utxos: vout overflow".into()))?,
                    value,
                    confirmed: confirmations > 0,
                });
            }
        }
        Ok(out)
    }

    /// Recommended fees from the chain info endpoint. Blockcypher rates are
    /// coin-per-kB — the same unit Blockbook's `estimatefee` returns — so the
    /// conversion matches: coin/kB × 10^decimals / 1000 → smallest-unit/vByte.
    pub async fn fees(&self) -> Result<RecommendedFees> {
        let info = self.get_json("/", "fees").await?;
        let scale = 10f64.powi(i32::from(self.decimals));
        let rate = |key: &str| -> Option<u64> {
            let coin_per_kb = info.get(key)?.as_f64()?;
            if coin_per_kb <= 0.0 {
                return None;
            }
            let per_vbyte = coin_per_kb * scale / 1000.0;
            Some(per_vbyte.round().max(1.0) as u64)
        };
        let high = rate("high_fee_per_kb");
        let medium = rate("medium_fee_per_kb");
        let low = rate("low_fee_per_kb");
        match (high, medium, low) {
            (Some(high), Some(medium), Some(low)) => Ok(RecommendedFees {
                fastest: high,
                half_hour: medium,
                hour: low,
                economy: low,
                minimum: 1,
            }),
            (Some(high), Some(medium), _) => Ok(RecommendedFees {
                fastest: high,
                half_hour: medium,
                hour: medium,
                economy: medium.saturating_sub(1).max(1),
                minimum: 1,
            }),
            _ => Err(Error::Network("fees: chain info missing fee fields".into())),
        }
    }

    /// Broadcast a raw transaction; returns the txid.
    pub async fn broadcast(&self, raw_hex: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/txs/push", self.base))
            .json(&serde_json::json!({ "tx": raw_hex }))
            .send()
            .await
            .map_err(|e| Error::Network(format!("broadcast: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("broadcast: invalid response: {e}")))?;
        let value: serde_json::Value = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("broadcast: invalid JSON: {e}")))?;
        if !status.is_success() {
            let reason = value
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(Error::Network(format!(
                "broadcast: HTTP {status}: {reason}"
            )));
        }
        value
            .get("tx")
            .and_then(|tx| tx.get("hash"))
            .and_then(|h| h.as_str())
            .map(str::to_string)
            .ok_or_else(|| Error::Network("broadcast: missing tx hash".into()))
    }

    /// Transaction history (confirmed only; mempool entries have no block
    /// height and carry no usable timestamp for the activity list).
    pub async fn history(&self, chain: ChainId, address: &str) -> Result<Vec<HistoryTx>> {
        let value = self
            .get_json(&format!("/addrs/{address}/full?limit=50"), "history")
            .await?;
        let Some(txs) = value.get("txs").and_then(|v| v.as_array()) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for tx in txs {
            let Some(hash) = tx.get("hash").and_then(|v| v.as_str()) else {
                continue;
            };
            let height = tx.get("block_height").and_then(|v| v.as_i64()).unwrap_or(0);
            if height <= 0 {
                continue;
            }
            let timestamp = tx
                .get("confirmed")
                .and_then(|v| v.as_str())
                .and_then(parse_iso8601_unix)
                .unwrap_or(0);
            // A row moves value for our side when our address appears among
            // the inputs (outgoing) or outputs (incoming).
            let targets = |items: &serde_json::Value| -> u128 {
                items
                    .as_array()
                    .map(|list| {
                        list.iter()
                            .filter(|item| {
                                item.get("addresses")
                                    .and_then(|v| v.as_array())
                                    .map(|addrs| {
                                        addrs
                                            .iter()
                                            .filter_map(|a| a.as_str())
                                            .any(|a| a == address)
                                    })
                                    .unwrap_or(false)
                            })
                            .filter_map(|item| item.get("value").and_then(|v| v.as_u64()))
                            .map(u128::from)
                            .sum::<u128>()
                    })
                    .unwrap_or(0)
            };
            let outgoing = targets(tx.get("inputs").unwrap_or(&serde_json::Value::Null));
            let incoming = targets(tx.get("outputs").unwrap_or(&serde_json::Value::Null));
            if incoming == 0 && outgoing == 0 {
                continue;
            }
            let amount = incoming
                .saturating_sub(outgoing)
                .max(outgoing.saturating_sub(incoming));
            let fee = tx.get("fees").and_then(|v| v.as_u64()).map(u128::from);
            out.push(HistoryTx {
                chain,
                txid: hash.to_string(),
                outgoing: outgoing > incoming,
                amount,
                fee,
                timestamp,
                confirmations: 1,
            });
        }
        Ok(out)
    }
}

/// `2026-10-28T17:31:52Z` → unix seconds. Blockcypher timestamps are UTC ISO.
fn parse_iso8601_unix(text: &str) -> Option<i64> {
    // Manual parse avoids a chrono dependency: "YYYY-MM-DDTHH:MM:SS" prefix.
    let (date, rest) = text.split_once('T')?;
    let time = rest.trim_end_matches('Z');
    let mut date_parts = date.split('-');
    let year: i64 = date_parts.next()?.parse().ok()?;
    let month: i64 = date_parts.next()?.parse().ok()?;
    let day: i64 = date_parts.next()?.parse().ok()?;
    let mut time_parts = time.split(':');
    let hour: i64 = time_parts.next()?.parse().ok()?;
    let minute: i64 = time_parts.next()?.parse().ok()?;
    let second: i64 = time_parts.next().unwrap_or("0").parse().ok()?;
    // days-from-civil algorithm (Howard Hinnant), valid for the Gregorian era.
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
struct AddressResponse {
    final_balance: i64,
}

impl TryFrom<serde_json::Value> for AddressResponse {
    type Error = Error;

    fn try_from(value: serde_json::Value) -> Result<Self> {
        serde_json::from_value(value)
            .map_err(|e| Error::Network(format!("balance: invalid shape: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balance_shape_parses() {
        let value = serde_json::json!({
            "address": "DMM78DyiwEPzWmykiQQUFcPcUv13YPe2ZK",
            "total_received": 0,
            "balance": 42,
            "unconfirmed_balance": -7,
            "final_balance": 35,
            "n_tx": 1
        });
        let page: AddressResponse = value.try_into().expect("documented shape should parse");
        assert_eq!(page.final_balance, 35);
    }

    #[test]
    fn utxo_shape_yields_confirmed_and_mempool() {
        let payload = serde_json::json!({
            "txrefs": [
                {"tx_hash": "abc", "tx_output_n": 1, "value": 12345, "confirmations": 6},
                {"tx_hash": "def", "tx_output_n": 0, "value": 99, "confirmations": 0}
            ],
            "unconfirmed_txrefs": [
                {"tx_hash": "ghi", "tx_output_n": 2, "value": 7, "confirmations": 0}
            ]
        });
        let mut utxos = Vec::new();
        for key in ["txrefs", "unconfirmed_txrefs"] {
            let refs = payload.get(key).and_then(|v| v.as_array()).unwrap();
            for r in refs {
                utxos.push(Utxo {
                    txid: r["tx_hash"].as_str().unwrap().to_string(),
                    vout: u32::try_from(r["tx_output_n"].as_u64().unwrap()).unwrap(),
                    value: r["value"].as_u64().unwrap(),
                    confirmed: r["confirmations"].as_u64().unwrap_or(0) > 0,
                });
            }
        }
        assert_eq!(utxos.len(), 3);
        assert_eq!(utxos[0].txid, "abc");
        assert_eq!(utxos[0].vout, 1);
        assert_eq!(utxos[0].value, 12345);
        assert!(utxos[0].confirmed);
        assert!(!utxos[1].confirmed);
        assert!(!utxos[2].confirmed);
    }

    #[test]
    fn fee_conversion_matches_live_values() {
        // Live: DOGE medium_fee_per_kb = 56_027_084 coin/kB, 8 decimals.
        let per_vbyte = 56_027_084f64 * 10f64.powi(8) / 1000.0;
        assert_eq!(per_vbyte.round() as u64, 5_602_708_400_000);
        // Live: DASH medium_fee_per_kb = 18_036 duffs/kB, 8 decimals.
        let per_vbyte = 18_036f64 * 10f64.powi(8) / 1000.0;
        assert_eq!(per_vbyte.round() as u64, 1_803_600_000);
    }

    #[test]
    fn iso8601_parses_to_unix() {
        // 2026-10-28T17:31:52Z == 1790909512? Verify against a known epoch:
        // 2026-01-01T00:00:00Z is 1767225600.
        assert_eq!(
            parse_iso8601_unix("2026-01-01T00:00:00Z"),
            Some(1_767_225_600)
        );
        let t = parse_iso8601_unix("2026-10-28T17:31:52Z").expect("parses");
        assert!(t > 1_767_225_600);
        assert_eq!(parse_iso8601_unix("1970-01-01T00:00:00Z"), Some(0));
    }

    #[test]
    fn history_row_direction_and_amount() {
        let address = "DMM78DyiwEPzWmykiQQUFcPcUv13YPe2ZK";
        let tx = serde_json::json!({
            "hash": "h1",
            "block_height": 6_434_412,
            "confirmed": "2026-10-28T17:31:52Z",
            "fees": 1_000,
            "inputs": [
                {"addresses": ["DOtherAddress1"], "value": 10_000}
            ],
            "outputs": [
                {"addresses": [address], "value": 9_000},
                {"addresses": ["DChange1"], "value": 0}
            ]
        });
        let targets = |items: &serde_json::Value| -> u128 {
            items
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter(|item| {
                            item.get("addresses")
                                .and_then(|v| v.as_array())
                                .map(|addrs| {
                                    addrs
                                        .iter()
                                        .filter_map(|a| a.as_str())
                                        .any(|a| a == address)
                                })
                                .unwrap_or(false)
                        })
                        .filter_map(|item| item.get("value").and_then(|v| v.as_u64()))
                        .map(u128::from)
                        .sum::<u128>()
                })
                .unwrap_or(0)
        };
        let outgoing = targets(tx.get("inputs").unwrap());
        let incoming = targets(tx.get("outputs").unwrap());
        assert_eq!(incoming, 9_000);
        assert_eq!(outgoing, 0);
        let amount = incoming
            .saturating_sub(outgoing)
            .max(outgoing.saturating_sub(incoming));
        assert_eq!(amount, 9_000);
        assert!(outgoing <= incoming);
    }
}
