use crate::error::{Error, Result};
use serde::Deserialize;

#[derive(Clone)]
pub struct UtxoClient {
    client: reqwest::Client,
    base: String,
}

#[derive(Debug, Clone)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
    pub confirmed: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct RecommendedFees {
    pub fastest: u64,
    pub half_hour: u64,
    pub hour: u64,
    pub economy: u64,
    pub minimum: u64,
}

#[derive(Deserialize)]
struct AddressResponse {
    chain_stats: AddressStats,
    mempool_stats: AddressStats,
}

#[derive(Deserialize)]
struct AddressStats {
    funded_txo_sum: u64,
    spent_txo_sum: u64,
}

#[derive(Deserialize)]
struct UtxoResponse {
    txid: String,
    vout: u32,
    value: u64,
    status: UtxoStatus,
}

#[derive(Deserialize)]
struct UtxoStatus {
    confirmed: bool,
}

#[derive(Deserialize)]
struct FeesResponse {
    #[serde(rename = "fastestFee")]
    fastest_fee: u64,
    #[serde(rename = "halfHourFee")]
    half_hour_fee: u64,
    #[serde(rename = "hourFee")]
    hour_fee: u64,
    #[serde(rename = "economyFee")]
    economy_fee: u64,
    #[serde(rename = "minimumFee")]
    minimum_fee: u64,
}

impl UtxoClient {
    pub fn new(client: reqwest::Client, base: impl Into<String>) -> Self {
        Self {
            client,
            base: base.into().trim_end_matches('/').to_string(),
        }
    }

    pub async fn balance(&self, address: &str) -> Result<u64> {
        let response = self
            .client
            .get(format!("{}/address/{}", self.base, address))
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
        let response: AddressResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("balance: invalid JSON: {e}")))?;
        let confirmed = response
            .chain_stats
            .funded_txo_sum
            .saturating_sub(response.chain_stats.spent_txo_sum);
        let pending = response
            .mempool_stats
            .funded_txo_sum
            .saturating_sub(response.mempool_stats.spent_txo_sum);
        Ok(confirmed.saturating_add(pending))
    }

    pub async fn utxos(&self, address: &str) -> Result<Vec<Utxo>> {
        let response = self
            .client
            .get(format!("{}/address/{}/utxo", self.base, address))
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
        let response: Vec<UtxoResponse> = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("utxos: invalid JSON: {e}")))?;
        Ok(response
            .into_iter()
            .map(|utxo| Utxo {
                txid: utxo.txid,
                vout: utxo.vout,
                value: utxo.value,
                confirmed: utxo.status.confirmed,
            })
            .collect())
    }

    pub async fn fees(&self) -> Result<RecommendedFees> {
        let response = self
            .client
            .get(format!("{}/v1/fees/recommended", self.base))
            .send()
            .await
            .map_err(|e| Error::Network(format!("fees: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("fees: invalid response: {e}")))?;
        if !status.is_success() {
            return Err(Error::Network(format!("fees: HTTP {status}")));
        }
        let response: FeesResponse = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("fees: invalid JSON: {e}")))?;
        Ok(RecommendedFees {
            fastest: response.fastest_fee,
            half_hour: response.half_hour_fee,
            hour: response.hour_fee,
            economy: response.economy_fee,
            minimum: response.minimum_fee,
        })
    }

    pub async fn broadcast(&self, raw_hex: &str) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/tx", self.base))
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
        Ok(text.trim().to_string())
    }
}
