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
            // JSON-RPC 2.0 规范允许成功响应显式携带 `"error": null`,
            // 此时按成功处理而不是换源。
            if let Some(error) = value.get("error").filter(|e| !e.is_null()) {
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
        // ETHW answers eth_gasPrice with 0; its real
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serve `count` requests, each answered with `status` and `body`, and
    /// record how many arrived. Slow responses delay before answering so the
    /// client timeout can be observed without touching the real network.
    async fn serve(
        count: usize,
        status: &'static str,
        body: &'static str,
        delay: Duration,
    ) -> (Vec<String>, tokio::task::JoinHandle<usize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let mut served = 0usize;
            for _ in 0..count {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let mut buf = vec![0u8; 16 * 1024];
                let Ok(read) = socket.read(&mut buf).await else {
                    break;
                };
                let _request = String::from_utf8_lossy(&buf[..read]);
                let response = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                tokio::time::sleep(delay).await;
                if socket.write_all(response.as_bytes()).await.is_err() {
                    break;
                }
                served += 1;
                let _ = socket.flush().await;
            }
            served
        });
        (vec![format!("http://{addr}")], handle)
    }

    fn client(timeout: Duration) -> reqwest::Client {
        reqwest::Client::builder().timeout(timeout).build().unwrap()
    }

    #[tokio::test]
    async fn falls_back_when_primary_is_down() {
        // 主源端口没人听(连接被拒) → 自动切到备源成功。
        let dead = "http://127.0.0.1:1";
        let (rpcs, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x10d4f"}"#,
            Duration::ZERO,
        )
        .await;
        let client =
            EvmClient::with_fallbacks(client(Duration::from_secs(5)), [dead, rpcs[0].as_str()]);
        assert_eq!(client.chain_id().await.unwrap(), 0x10d4f);
        assert_eq!(server.await.unwrap(), 1, "备源应收到恰好一次请求");
    }

    #[tokio::test]
    async fn falls_back_on_http_error_and_bad_json() {
        // 备源一:HTTP 500;备源二:非 JSON;备源三:正常。
        let (ok, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x2710"}"#,
            Duration::ZERO,
        )
        .await;
        let (bad_status, _) = serve(0, "500 Internal Server Error", "nope", Duration::ZERO).await;
        let (garbage, _) = serve(1, "200 OK", "<html>502</html>", Duration::ZERO).await;
        let client = EvmClient::with_fallbacks(
            client(Duration::from_secs(5)),
            [bad_status[0].clone(), garbage[0].clone(), ok[0].clone()],
        );
        assert_eq!(client.chain_id().await.unwrap(), 10_000);
        assert_eq!(server.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn slow_source_times_out_and_falls_back() {
        // 主源 1.5s 后才应答,客户端 200ms 超时 → 切到立即应答的备源。
        let (ok, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x64"}"#,
            Duration::ZERO,
        )
        .await;
        let (slow, slow_server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x1"}"#,
            Duration::from_millis(1500),
        )
        .await;
        let client = EvmClient::with_fallbacks(
            client(Duration::from_millis(200)),
            [slow[0].clone(), ok[0].clone()],
        );
        assert_eq!(client.chain_id().await.unwrap(), 100);
        assert_eq!(server.await.unwrap(), 1, "快速备源应被使用");
        // 慢源可以收到请求,但超时后其迟到的应答不会被采纳。
        let _ = slow_server.await.unwrap();
    }

    #[tokio::test]
    async fn explicit_null_error_is_success() {
        // JSON-RPC 2.0 允许成功响应显式携带 "error": null,不应误判为失败。
        let (rpcs, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":"0x3e8","error":null}"#,
            Duration::ZERO,
        )
        .await;
        let client = EvmClient::with_fallbacks(client(Duration::from_secs(5)), [rpcs[0].as_str()]);
        assert_eq!(client.chain_id().await.unwrap(), 1000);
        assert_eq!(server.await.unwrap(), 1, "不应换源重试");
    }

    #[tokio::test]
    async fn all_sources_failing_reports_last_error() {
        let (a, _) = serve(1, "500 Internal Server Error", "down", Duration::ZERO).await;
        let (b, _) = serve(1, "200 OK", "not json", Duration::ZERO).await;
        let client =
            EvmClient::with_fallbacks(client(Duration::from_secs(5)), [a[0].clone(), b[0].clone()]);
        let error = client.chain_id().await.unwrap_err();
        assert!(error.to_string().contains("eth_chainId"), "{error}");
    }
}
