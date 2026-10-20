//! `monero-wallet-rpc` adapter.
//!
//! A Monero spend is a CLSAG ring signature plus a Bulletproofs+ range proof
//! over Pedersen commitments. Re-implementing that here would be both large and
//! dangerous — a single nonce or range-proof mistake can leak the spend key or
//! burn the output. Lycoris therefore drives the official
//! `monero-wallet-rpc` (the same architecture Feather uses) and keeps custody
//! of the keys: the wallet file is created from this app's derived spend/view
//! keys through `generate_from_keys`, and the RPC endpoint defaults to
//! localhost.
//!
//! One wallet can be open per RPC process, so [`MoneroRpc::ensure_wallet`]
//! closes whatever is open and opens (or generates) the wallet that belongs to
//! the active phrase.

use crate::error::{Error, Result};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Clone)]
pub struct MoneroRpc {
    client: reqwest::Client,
    url: String,
    auth: Option<(String, String)>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Balance {
    pub balance: u64,
    pub unlocked_balance: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransferOutcome {
    #[serde(default)]
    pub tx_hash: String,
    #[serde(default)]
    pub fee: u64,
    #[serde(default)]
    pub amount: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SweepOutcome {
    #[serde(default)]
    pub tx_hash_list: Vec<String>,
    #[serde(default)]
    pub fee_list: Vec<u64>,
    #[serde(default)]
    pub amount_list: Vec<u64>,
}

#[derive(Debug, Deserialize)]
struct VersionResponse {
    #[serde(default)]
    version: u64,
}

#[derive(Debug, Deserialize)]
struct HeightResponse {
    #[serde(default)]
    height: u64,
}

#[derive(Debug, Deserialize)]
struct AddressResponse {
    #[serde(default)]
    address: String,
}

#[derive(Debug, Deserialize)]
struct RpcError {
    #[serde(default)]
    code: i64,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

/// Everything `monero-wallet-rpc` needs to open or create the wallet that
/// belongs to one derived phrase.
pub struct WalletImport<'a> {
    pub filename: &'a str,
    pub password: &'a str,
    pub address: &'a str,
    pub spend_key: &'a str,
    pub view_key: &'a str,
    pub restore_height: u64,
}

impl MoneroRpc {
    /// `url` must point at the JSON-RPC endpoint, e.g.
    /// `http://127.0.0.1:18083/json_rpc`.
    pub fn new(
        client: reqwest::Client,
        url: impl Into<String>,
        user: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        let user = user.into();
        let password = password.into();
        Self {
            client,
            url: url.into(),
            auth: if user.trim().is_empty() {
                None
            } else {
                Some((user, password))
            },
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": "0",
            "method": method,
            "params": params,
        });
        let mut request = self.client.post(&self.url).json(&body);
        if let Some((user, password)) = &self.auth {
            request = request.basic_auth(user, Some(password));
        }
        let response = request.send().await.map_err(|error| {
            if error.is_connect() || error.is_timeout() {
                Error::RpcUnavailable(format!("{}: {error}", self.url))
            } else {
                Error::Network(format!("{method}: {error}"))
            }
        })?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|error| Error::Network(format!("{method}: {error}")))?;
        if status == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Error::RpcUnavailable(format!(
                "{}: authentication failed (401)",
                self.url
            )));
        }
        if !status.is_success() {
            let detail = text.trim();
            let detail = if detail.len() > 200 {
                &detail[..200]
            } else {
                detail
            };
            return Err(Error::Network(format!(
                "{method}: HTTP {status}{}",
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(" ({detail})")
                }
            )));
        }
        let envelope: Envelope = serde_json::from_str(&text)
            .map_err(|error| Error::Network(format!("{method}: invalid JSON: {error}")))?;
        if let Some(error) = envelope.error {
            return Err(Error::Transaction(format!(
                "{method}: {} (code {})",
                error.message, error.code
            )));
        }
        envelope
            .result
            .ok_or_else(|| Error::Network(format!("{method}: empty result")))
    }

    /// Round-trip check: the RPC answers and reports its version.
    pub async fn version(&self) -> Result<u64> {
        let value = self.call("get_version", json!({})).await?;
        let parsed: VersionResponse =
            serde_json::from_value(value).map_err(|e| Error::Network(e.to_string()))?;
        Ok(parsed.version)
    }

    pub async fn height(&self) -> Result<u64> {
        let value = self.call("get_height", json!({})).await?;
        let parsed: HeightResponse =
            serde_json::from_value(value).map_err(|e| Error::Network(e.to_string()))?;
        Ok(parsed.height)
    }

    /// Address of the currently open wallet.
    pub async fn address(&self, account_index: u32) -> Result<String> {
        let value = self
            .call("get_address", json!({ "account_index": account_index }))
            .await?;
        let parsed: AddressResponse =
            serde_json::from_value(value).map_err(|e| Error::Network(e.to_string()))?;
        Ok(parsed.address)
    }

    pub async fn balance(&self, account_index: u32) -> Result<Balance> {
        let value = self
            .call("get_balance", json!({ "account_index": account_index }))
            .await?;
        serde_json::from_value(value).map_err(|e| Error::Network(e.to_string()))
    }

    pub async fn close_wallet(&self) -> Result<()> {
        self.call("close_wallet", json!({})).await.map(|_| ())
    }

    pub async fn open_wallet(&self, filename: &str, password: &str) -> Result<()> {
        self.call(
            "open_wallet",
            json!({ "filename": filename, "password": password }),
        )
        .await
        .map(|_| ())
    }

    /// Create the wallet file from keys this app derived. `restore_height`
    /// controls how far back the wallet scans; 0 means genesis.
    pub async fn generate_from_keys(&self, import: &WalletImport<'_>) -> Result<String> {
        let value = self
            .call(
                "generate_from_keys",
                json!({
                    "restore_height": import.restore_height,
                    "filename": import.filename,
                    "address": import.address,
                    "spendkey": import.spend_key,
                    "viewkey": import.view_key,
                    "password": import.password,
                }),
            )
            .await?;
        let parsed: AddressResponse =
            serde_json::from_value(value).map_err(|e| Error::Network(e.to_string()))?;
        Ok(parsed.address)
    }

    /// Make sure the wallet open in `monero-wallet-rpc` is the one that belongs
    /// to `import.address`, creating it if necessary.
    pub async fn ensure_wallet(&self, import: &WalletImport<'_>) -> Result<()> {
        // Ours already? Nothing to do.
        if let Ok(address) = self.address(0).await {
            if address == import.address {
                return Ok(());
            }
        }
        // `open_wallet` rejects a second wallet, so close the current one
        // first. Failing to close usually means "no wallet open", which is
        // exactly the state we want.
        let _ = self.close_wallet().await;
        if self
            .open_wallet(import.filename, import.password)
            .await
            .is_ok()
        {
            let address = self.address(0).await?;
            return if address == import.address {
                Ok(())
            } else {
                Err(Error::Transaction(format!(
                    "monero-wallet-rpc opened a different wallet ({address}); \
                     set another wallet directory"
                )))
            };
        }
        let address = self.generate_from_keys(import).await?;
        if address != import.address {
            return Err(Error::Transaction(format!(
                "monero-wallet-rpc generated a different address ({address})"
            )));
        }
        Ok(())
    }

    /// Send `amount` piconero to `address` with the given priority
    /// (1 = unimportant, 2 = normal, 3 = elevated).
    pub async fn transfer(
        &self,
        address: &str,
        amount: u64,
        priority: u8,
    ) -> Result<TransferOutcome> {
        let value = self
            .call(
                "transfer",
                json!({
                    "destinations": [{ "amount": amount, "address": address }],
                    "account_index": 0,
                    "priority": priority,
                    "get_tx_key": true,
                }),
            )
            .await?;
        serde_json::from_value(value).map_err(|e| Error::Network(e.to_string()))
    }

    /// Send the whole unlocked balance (fee deducted). Monero may split this
    /// into several transactions when the wallet holds many outputs.
    pub async fn sweep_all(&self, address: &str, priority: u8) -> Result<SweepOutcome> {
        let value = self
            .call(
                "sweep_all",
                json!({
                    "address": address,
                    "account_index": 0,
                    "priority": priority,
                    "get_tx_keys": true,
                }),
            )
            .await?;
        serde_json::from_value(value).map_err(|e| Error::Network(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serve exactly one request with a canned body and return what the client
    /// actually sent.
    async fn serve_once(body: &'static str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 16 * 1024];
            let read = socket.read(&mut buf).await.unwrap();
            let request = String::from_utf8_lossy(&buf[..read]).to_string();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            request
        });
        (format!("http://{addr}/json_rpc"), handle)
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn balance_uses_json_rpc_shape() {
        let (url, server) = serve_once(
            r#"{"id":"0","jsonrpc":"2.0","result":{"balance":1200,"unlocked_balance":900}}"#,
        )
        .await;
        let rpc = MoneroRpc::new(client(), url, "", "");
        let balance = rpc.balance(0).await.unwrap();
        assert_eq!(balance.balance, 1200);
        assert_eq!(balance.unlocked_balance, 900);

        let request = server.await.unwrap();
        assert!(request.starts_with("POST /json_rpc"));
        assert!(request.contains("\"method\":\"get_balance\""));
        assert!(request.contains("\"account_index\":0"));
    }

    #[tokio::test]
    async fn rpc_errors_keep_the_wallet_message() {
        let (url, server) = serve_once(
            r#"{"id":"0","jsonrpc":"2.0","error":{"code":-17,"message":"not enough unlocked money"}}"#,
        )
        .await;
        let rpc = MoneroRpc::new(client(), url, "", "");
        let error = rpc.transfer("4address", 1, 2).await.unwrap_err();
        assert!(error.to_string().contains("not enough unlocked money"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn generate_from_keys_sends_both_keys() {
        let (url, server) =
            serve_once(r#"{"id":"0","jsonrpc":"2.0","result":{"address":"4abc","info":"ok"}}"#)
                .await;
        let rpc = MoneroRpc::new(client(), url, "", "");
        let address = rpc
            .generate_from_keys(&WalletImport {
                filename: "lycoris-1",
                password: "pw",
                address: "4abc",
                spend_key: "aa",
                view_key: "bb",
                restore_height: 3_000_000,
            })
            .await
            .unwrap();
        assert_eq!(address, "4abc");

        let request = server.await.unwrap();
        assert!(request.contains("\"method\":\"generate_from_keys\""));
        assert!(request.contains("\"spendkey\":\"aa\""));
        assert!(request.contains("\"viewkey\":\"bb\""));
        assert!(request.contains("\"restore_height\":3000000"));
        assert!(request.contains("\"password\":\"pw\""));
    }

    #[tokio::test]
    async fn basic_auth_is_sent_when_configured() {
        let (url, server) =
            serve_once(r#"{"id":"0","jsonrpc":"2.0","result":{"height":42}}"#).await;
        let rpc = MoneroRpc::new(client(), url, "lycoris", "secret");
        assert_eq!(rpc.height().await.unwrap(), 42);

        let request = server.await.unwrap();
        // base64("lycoris:secret")
        assert!(request.contains("authorization: Basic bHljb3JpczpzZWNyZXQ="));
    }

    #[tokio::test]
    async fn ensure_wallet_creates_a_missing_wallet() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut methods = Vec::new();
            // No wallet open → close fails → open fails → create from keys.
            for body in [
                r#"{"id":"0","jsonrpc":"2.0","error":{"code":-13,"message":"No wallet file"}}"#,
                r#"{"id":"0","jsonrpc":"2.0","error":{"code":-1,"message":"No wallet open"}}"#,
                r#"{"id":"0","jsonrpc":"2.0","error":{"code":-1,"message":"Wallet file not found"}}"#,
                r#"{"id":"0","jsonrpc":"2.0","result":{"address":"4abc","info":"ok"}}"#,
            ] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = vec![0u8; 16 * 1024];
                let read = socket.read(&mut buf).await.unwrap();
                let request = String::from_utf8_lossy(&buf[..read]).to_string();
                let method = request
                    .split("\"method\":\"")
                    .nth(1)
                    .and_then(|rest| rest.split('"').next())
                    .unwrap_or_default()
                    .to_string();
                methods.push(method);
                let response = format!(
                    "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            methods
        });

        let rpc = MoneroRpc::new(client(), format!("http://{addr}/json_rpc"), "", "");
        rpc.ensure_wallet(&WalletImport {
            filename: "lycoris-1",
            password: "pw",
            address: "4abc",
            spend_key: "aa",
            view_key: "bb",
            restore_height: 0,
        })
        .await
        .unwrap();

        assert_eq!(
            server.await.unwrap(),
            [
                "get_address",
                "close_wallet",
                "open_wallet",
                "generate_from_keys"
            ]
        );
    }

    #[tokio::test]
    async fn connection_errors_are_marked_unavailable() {
        // Bind then drop to get a port that refuses connections.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        let rpc = MoneroRpc::new(client(), format!("http://{addr}/json_rpc"), "", "");
        match rpc.version().await {
            Err(Error::RpcUnavailable(message)) => assert!(message.contains(&addr.to_string())),
            other => panic!("expected RpcUnavailable, got {other:?}"),
        }
    }
}
