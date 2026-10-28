use crate::error::{Error, Result};
use serde_json::{json, Value};
use sha3::{Digest, Keccak256};

#[derive(Clone)]
pub struct EvmClient {
    client: reqwest::Client,
    /// RPC 候选列表(主源在前)。任一调用失败自动切换下一个, 全部失败才报错。
    rpcs: Vec<String>,
}

impl EvmClient {
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
                    last_error = Some(Error::Network(format!("{method}: {rpc}: {e}")));
                    continue;
                }
            };
            let status = response.status();
            let text = match response.text().await {
                Ok(text) => text,
                Err(e) => {
                    last_error = Some(Error::Network(format!("{method}: invalid response: {e}")));
                    continue;
                }
            };
            let value: Value = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(e) => {
                    last_error = Some(Error::Network(format!("{method}: invalid JSON: {e}")));
                    continue;
                }
            };
            if let Some(error) = value.get("error") {
                // RPC 层错误(参数/状态问题)换源也一样的概率低, 但仍继续试下一源。
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
        Err(last_error.unwrap_or_else(|| Error::Network(format!("{method}: no RPC configured"))))
    }

    pub async fn chain_id(&self) -> Result<u64> {
        let value = self.call("eth_chainId", json!([])).await?;
        hex_u64(value.as_str().unwrap_or("0x0"))
    }

    pub async fn balance(&self, address: &str) -> Result<u128> {
        let value = self
            .call("eth_getBalance", json!([address, "latest"]))
            .await?;
        hex_u128(value.as_str().unwrap_or("0x0"))
    }

    pub async fn nonce(&self, address: &str) -> Result<u64> {
        let value = self
            .call("eth_getTransactionCount", json!([address, "pending"]))
            .await?;
        hex_u64(value.as_str().unwrap_or("0x0"))
    }

    pub async fn gas_price(&self) -> Result<u128> {
        let value = self.call("eth_gasPrice", json!([])).await?;
        let price = hex_u128(value.as_str().unwrap_or("0x0"))?;
        if price > 0 {
            return Ok(price);
        }
        // Some PoW chains (ETHW) answer eth_gasPrice with 0; their real
        // floor is the block base fee.
        Ok(self.latest_base_fee().await?.unwrap_or(0))
    }

    pub async fn max_priority_fee(&self) -> Result<Option<u128>> {
        match self.call("eth_maxPriorityFeePerGas", json!([])).await {
            Ok(value) => Ok(value.as_str().and_then(|s| hex_u128(s).ok())),
            Err(_) => Ok(None),
        }
    }

    pub async fn latest_base_fee(&self) -> Result<Option<u128>> {
        let value = self
            .call("eth_getBlockByNumber", json!(["latest", false]))
            .await?;
        Ok(value
            .get("baseFeePerGas")
            .and_then(|v| v.as_str())
            .and_then(|s| hex_u128(s).ok()))
    }

    pub async fn estimate_gas(&self, from: &str, to: &str, value_wei: u128) -> Result<u64> {
        let value = self
            .call(
                "eth_estimateGas",
                json!([{
                    "from": from,
                    "to": to,
                    "value": format!("0x{value_wei:x}"),
                }]),
            )
            .await?;
        hex_u64(value.as_str().unwrap_or("0x5208"))
    }

    pub async fn send_raw_transaction(&self, raw_hex: &str) -> Result<String> {
        let value = self
            .call("eth_sendRawTransaction", json!([raw_hex]))
            .await?;
        value
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Network("broadcast returned no transaction hash".into()))
    }
}

pub fn hex_u64(value: &str) -> Result<u64> {
    let trimmed = value.trim_start_matches("0x");
    if trimmed.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(trimmed, 16)
        .map_err(|_| Error::Network(format!("invalid hex quantity: {value}")))
}

pub fn hex_u128(value: &str) -> Result<u128> {
    let trimmed = value.trim_start_matches("0x");
    if trimmed.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(trimmed, 16)
        .map_err(|_| Error::Network(format!("invalid hex quantity: {value}")))
}

pub fn keccak(data: &[u8]) -> [u8; 32] {
    let digest = Keccak256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}
