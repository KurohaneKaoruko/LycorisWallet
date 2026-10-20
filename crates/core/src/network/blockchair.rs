//! Blockchair adapter for the UTXO chains that do not speak mempool.space's
//! API: Dogecoin and Bitcoin Cash.
//!
//! Blockchair exposes one base URL per chain (`…/dogecoin`, `…/bitcoin-cash`)
//! and answers with a `data` object. An API key is optional: without one the
//! public rate limit applies, so the key can be supplied through the
//! `LYCORIS_BLOCKCHAIR_KEY` environment variable.

use crate::error::{Error, Result};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Clone)]
pub struct BlockchairClient {
    client: reqwest::Client,
    base: String,
    api_key: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DashboardResponse {
    data: HashMap<String, Dashboard>,
}

#[derive(Debug, Deserialize)]
struct Dashboard {
    address: AddressInfo,
}

#[derive(Debug, Deserialize)]
struct AddressInfo {
    #[serde(default)]
    balance: u64,
}

#[derive(Debug, Deserialize)]
struct UtxoResponse {
    #[serde(default)]
    data: Vec<UtxoItem>,
}

#[derive(Debug, Deserialize)]
struct UtxoItem {
    txid: String,
    vout: u32,
    value: u64,
    #[serde(default)]
    block: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct BroadcastResponse {
    data: BroadcastData,
}

#[derive(Debug, Deserialize)]
struct BroadcastData {
    #[serde(default)]
    txid: String,
}

impl BlockchairClient {
    pub fn new(client: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
            api_key: std::env::var("LYCORIS_BLOCKCHAIR_KEY")
                .ok()
                .filter(|key| !key.trim().is_empty()),
        }
    }

    /// The same client with an explicit key, used by the tests.
    pub fn with_key(mut self, key: Option<String>) -> Self {
        self.api_key = key.filter(|value| !value.trim().is_empty());
        self
    }

    /// A client that prefers the key from the wallet settings and falls back
    /// to the environment variable.
    pub fn with_settings(
        client: reqwest::Client,
        base: impl Into<String>,
        settings_key: Option<&str>,
    ) -> Self {
        let from_settings = settings_key
            .map(str::trim)
            .filter(|key| !key.is_empty())
            .map(str::to_string);
        match from_settings {
            Some(key) => Self::new(client, base).with_key(Some(key)),
            None => Self::new(client, base),
        }
    }

    fn url(&self, path: &str) -> String {
        let mut url = format!("{}{}", self.base, path);
        if let Some(key) = &self.api_key {
            url.push_str(&format!("?key={key}"));
        }
        url
    }

    /// Confirmed plus unconfirmed balance of an address, in satoshis.
    pub async fn balance(&self, address: &str) -> Result<u64> {
        let response = self
            .client
            .get(self.url(&format!("/dashboards/address/{address}")))
            .send()
            .await
            .map_err(|e| Error::Network(format!("balance: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("balance: invalid response: {e}")))?;
        if !status.is_success() {
            return Err(Error::Network(format!("balance: HTTP {status}")));
        }
        let response: DashboardResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("balance: invalid JSON: {e}")))?;
        Ok(response
            .data
            .values()
            .map(|dashboard| dashboard.address.balance)
            .max()
            .unwrap_or(0))
    }

    /// The unspent outputs of an address, in satoshis.
    pub async fn utxos(&self, address: &str) -> Result<Vec<crate::network::utxo::Utxo>> {
        let response = self
            .client
            .get(self.url(&format!("/utxo/{address}")))
            .send()
            .await
            .map_err(|e| Error::Network(format!("utxos: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("utxos: invalid response: {e}")))?;
        if !status.is_success() {
            return Err(Error::Network(format!("utxos: HTTP {status}")));
        }
        let response: UtxoResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("utxos: invalid JSON: {e}")))?;
        Ok(response
            .data
            .into_iter()
            .map(|item| crate::network::utxo::Utxo {
                txid: item.txid,
                vout: item.vout,
                value: item.value,
                // Blockchair reports the block height; unconfirmed outputs come
                // back without one.
                confirmed: item.block.is_some_and(|height| height > 0),
            })
            .collect())
    }

    /// Broadcast a raw transaction and return its id.
    pub async fn broadcast(&self, raw_hex: &str) -> Result<String> {
        let response = self
            .client
            .post(self.url("/raw/transaction"))
            .form(&[("data", raw_hex)])
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
        let response: BroadcastResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("broadcast: invalid JSON: {e}")))?;
        if response.data.txid.is_empty() {
            return Err(Error::Network(format!(
                "broadcast: the node did not return a transaction id: {}",
                text.trim()
            )));
        }
        Ok(response.data.txid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_dashboard_response() {
        let json = r#"{"data":{"DMM7...":{"address":{"address":"DMM7...","balance":88400000000,
            "transaction_count":12,"output_count":9,"spent_count":3,"unspent_output_count":4}}},
            "context":{"address":"DMM7..."}}"#;
        let response: DashboardResponse = serde_json::from_str(json).unwrap();
        let balance = response
            .data
            .values()
            .map(|dashboard| dashboard.address.balance)
            .max()
            .unwrap();
        assert_eq!(balance, 88_400_000_000);
    }

    #[test]
    fn parses_an_utxo_response() {
        let json = r#"{"data":[{"txid":"ab","vout":1,"value":1000,"block":12345},
            {"txid":"cd","vout":0,"value":2000}],"context":{}}"#;
        let response: UtxoResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.len(), 2);
        assert_eq!(response.data[0].vout, 1);
        assert_eq!(response.data[0].value, 1000);
        assert!(response.data[0].block.is_some());
        assert!(response.data[1].block.is_none());
    }

    #[test]
    fn parses_a_broadcast_response() {
        let json = r#"{"data":{"txid":"9f1c2d"},"context":{}}"#;
        let response: BroadcastResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.data.txid, "9f1c2d");
    }
}
