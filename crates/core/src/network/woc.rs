//! WhatsOnChain adapter for BSV — the de-facto public BSV index API
//! (docs.whatsonchain.com, keyless up to 3 req/s).

use crate::error::{Error, Result};
use serde::Deserialize;

#[derive(Clone)]
pub struct WocClient {
    client: reqwest::Client,
    base: String,
}

#[derive(Debug, Deserialize)]
struct BalanceResponse {
    #[serde(default)]
    confirmed: i64,
    #[serde(default)]
    unconfirmed: i64,
}

#[derive(Debug, Deserialize)]
struct UnspentResponse {
    #[serde(default)]
    result: Vec<UnspentItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UnspentItem {
    #[serde(rename = "tx_hash")]
    tx_hash: String,
    #[serde(rename = "tx_pos")]
    tx_pos: u32,
    #[serde(default)]
    value: u64,
    #[serde(default)]
    height: i64,
}

#[derive(Debug, Deserialize)]
struct BroadcastResponse {
    #[serde(default)]
    txid: String,
}

impl WocClient {
    pub fn new(client: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
        }
    }

    async fn get_json<T: for<'de> Deserialize<'de>>(&self, path: &str, what: &str) -> Result<T> {
        let response = self
            .client
            .get(format!("{}/address/{}", self.base, path))
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

    /// Confirmed plus unconfirmed balance in satoshis.
    pub async fn balance(&self, address: &str) -> Result<u64> {
        let page: BalanceResponse = self
            .get_json(&format!("{address}/balance"), "balance")
            .await?;
        Ok((page.confirmed + page.unconfirmed).max(0) as u64)
    }

    /// Combined confirmed + mempool UTXOs.
    pub async fn utxos(&self, address: &str) -> Result<Vec<crate::network::utxo::Utxo>> {
        let page: UnspentResponse = self
            .get_json(&format!("{address}/unspent/all"), "utxos")
            .await?;
        Ok(page
            .result
            .into_iter()
            .map(|item| crate::network::utxo::Utxo {
                txid: item.tx_hash,
                vout: item.tx_pos,
                value: item.value,
                confirmed: item.height > 0,
            })
            .collect())
    }

    pub async fn broadcast(&self, raw_hex: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/tx/raw", self.base))
            .json(&serde_json::json!({ "rawhex": raw_hex }))
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
        let page: BroadcastResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("broadcast: invalid JSON: {e}")))?;
        if page.txid.is_empty() {
            return Err(Error::Network("broadcast: empty txid".into()));
        }
        Ok(page.txid)
    }
}
