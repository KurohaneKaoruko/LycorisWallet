//! Solana JSON-RPC adapter.

use crate::error::{Error, Result};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct SolanaClient {
    client: reqwest::Client,
    rpc: String,
}

impl SolanaClient {
    pub fn new(client: reqwest::Client, rpc: impl Into<String>) -> Self {
        Self {
            client,
            rpc: rpc.into(),
        }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let response = self
            .client
            .post(&self.rpc)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Network(format!("{method}: {e}")))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|e| Error::Network(format!("{method}: invalid response: {e}")))?;
        let value: Value = serde_json::from_str(&text)
            .map_err(|e| Error::Network(format!("{method}: invalid JSON: {e}")))?;
        if let Some(error) = value.get("error") {
            return Err(Error::Network(format!(
                "{method}: {}",
                error
                    .get("message")
                    .and_then(|m| m.as_str())
                    .unwrap_or("rpc error")
            )));
        }
        if !status.is_success() {
            return Err(Error::Network(format!("{method}: HTTP {status}")));
        }
        value
            .get("result")
            .cloned()
            .ok_or_else(|| Error::Network(format!("{method}: missing result")))
    }

    pub async fn balance(&self, address: &str) -> Result<u64> {
        let value = self
            .call("getBalance", json!([address, {"commitment": "confirmed"}]))
            .await?;
        value
            .get("value")
            .and_then(|v| v.as_u64())
            .ok_or_else(|| Error::Network("getBalance: missing value".into()))
    }

    pub async fn latest_blockhash(&self) -> Result<String> {
        let value = self
            .call("getLatestBlockhash", json!([{"commitment": "confirmed"}]))
            .await?;
        value
            .get("value")
            .and_then(|v| v.get("blockhash"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Network("getLatestBlockhash: missing blockhash".into()))
    }

    pub async fn fee_for_message(&self, base64_message: &str) -> Result<Option<u64>> {
        let value = self
            .call(
                "getFeeForMessage",
                json!([base64_message, {"commitment": "confirmed"}]),
            )
            .await?;
        Ok(value.get("value").and_then(|v| v.as_u64()))
    }

    pub async fn send_transaction(&self, base64_transaction: &str) -> Result<String> {
        let value = self
            .call(
                "sendTransaction",
                json!([
                    base64_transaction,
                    {"encoding": "base64", "preflightCommitment": "confirmed"}
                ]),
            )
            .await?;
        value
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Network("sendTransaction returned no signature".into()))
    }
}
