//! Solana JSON-RPC adapter.

use crate::error::{Error, Result};
use serde_json::{json, Value};

#[derive(Clone)]
pub struct SolanaClient {
    client: reqwest::Client,
    /// RPC 候选列表(主源在前)。任一调用失败自动切换下一个, 全部失败才报错。
    rpcs: Vec<String>,
}

impl SolanaClient {
    pub fn new(client: reqwest::Client, rpc: impl Into<String>) -> Self {
        Self {
            client,
            rpcs: vec![rpc.into()],
        }
    }

    /// 多 RPC 回退:主源失败(网络不可达/限速/宕机)时依次尝试备用源。
    pub fn with_fallbacks(
        client: reqwest::Client,
        rpcs: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        Self {
            client,
            rpcs: rpcs.into_iter().map(Into::into).collect(),
        }
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });
        let mut last_error = None;
        for rpc in &self.rpcs {
            let response = match self.client.post(rpc).json(&body).send().await {
                Ok(response) => response,
                Err(e) => {
                    last_error =
                        Some(Error::Network(format!("{method}: {rpc}: {e}")));
                    continue;
                }
            };
            let status = response.status();
            let text = match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    last_error =
                        Some(Error::Network(format!("{method}: invalid response: {e}")));
                    continue;
                }
            };
            let value: Value = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(e) => {
                    last_error =
                        Some(Error::Network(format!("{method}: invalid JSON: {e}")));
                    continue;
                }
            };
            if let Some(error) = value.get("error") {
                last_error = Some(Error::Network(format!(
                    "{method}: {}",
                    error
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("rpc error")
                )));
                continue;
            }
            if !status.is_success() {
                last_error = Some(Error::Network(format!("{method}: HTTP {status}")));
                continue;
            }
            return value
                .get("result")
                .cloned()
                .ok_or_else(|| Error::Network(format!("{method}: missing result")));
        }
        Err(last_error
            .unwrap_or_else(|| Error::Network(format!("{method}: no RPC configured"))))
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
