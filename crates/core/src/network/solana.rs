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
    async fn balance_falls_back_to_secondary() {
        let dead = "http://127.0.0.1:1";
        let (rpcs, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":42}}"#,
            Duration::ZERO,
        )
        .await;
        let client =
            SolanaClient::with_fallbacks(client(Duration::from_secs(5)), [dead, rpcs[0].as_str()]);
        assert_eq!(client.balance("Addr").await.unwrap(), 42);
        assert_eq!(server.await.unwrap(), 1);
    }

    #[tokio::test]
    async fn slow_source_times_out_and_falls_back() {
        // 主源 1.5s 后才应答,客户端 200ms 超时 → 切到立即应答的备源。
        let (ok, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":7}}"#,
            Duration::ZERO,
        )
        .await;
        let (slow, slow_server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":1}}"#,
            Duration::from_millis(1500),
        )
        .await;
        let client = SolanaClient::with_fallbacks(
            client(Duration::from_millis(200)),
            [slow[0].clone(), ok[0].clone()],
        );
        assert_eq!(client.balance("Addr").await.unwrap(), 7);
        assert_eq!(server.await.unwrap(), 1);
        let _ = slow_server.await.unwrap();
    }

    #[tokio::test]
    async fn explicit_null_error_is_success() {
        // JSON-RPC 2.0 允许成功响应显式携带 "error": null,不应误判为失败。
        let (rpcs, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":9},"error":null}"#,
            Duration::ZERO,
        )
        .await;
        let client =
            SolanaClient::with_fallbacks(client(Duration::from_secs(5)), [rpcs[0].as_str()]);
        assert_eq!(client.balance("Addr").await.unwrap(), 9);
        assert_eq!(server.await.unwrap(), 1, "不应换源重试");
    }

    #[tokio::test]
    async fn rpc_error_tries_next_source() {
        // 主源返回 RPC 层错误 → 换下一个源。
        let (ok, server) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"result":{"context":{"slot":1},"value":5}}"#,
            Duration::ZERO,
        )
        .await;
        let (failing, _) = serve(
            1,
            "200 OK",
            r#"{"jsonrpc":"2.0","id":1,"error":{"code":-32000,"message":"node is behind"}}"#,
            Duration::ZERO,
        )
        .await;
        let client = SolanaClient::with_fallbacks(
            client(Duration::from_secs(5)),
            [failing[0].clone(), ok[0].clone()],
        );
        assert_eq!(client.balance("Addr").await.unwrap(), 5);
        assert_eq!(server.await.unwrap(), 1);
    }
}
