//! Spot prices from CoinGecko.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Production CoinGecko base. Tests point [`fetch_prices_from`] at a local
/// mock instead (same pattern as the other network adapters).
const COINGECKO_BASE: &str = "https://api.coingecko.com/api/v3/simple/price";

/// Fetch spot prices from CoinGecko. Returns `id -> currency -> price`.
pub async fn fetch_prices(
    client: &reqwest::Client,
    ids: &[&str],
    currencies: &[&str],
) -> Result<HashMap<String, HashMap<String, f64>>> {
    fetch_prices_from(client, COINGECKO_BASE, ids, currencies).await
}

/// Same as [`fetch_prices`] against an explicit base URL — the seam the
/// offline tests use.
pub async fn fetch_prices_from(
    client: &reqwest::Client,
    base: &str,
    ids: &[&str],
    currencies: &[&str],
) -> Result<HashMap<String, HashMap<String, f64>>> {
    if ids.is_empty() || currencies.is_empty() {
        return Ok(HashMap::new());
    }
    let url = format!(
        "{}?ids={}&vs_currencies={}",
        base,
        ids.join(","),
        currencies.join(",")
    );
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| Error::Network(format!("prices: {e}")))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| Error::Network(format!("prices: invalid response: {e}")))?;
    if !status.is_success() {
        return Err(Error::Network(format!(
            "prices: HTTP {status}: {}",
            text.chars().take(160).collect::<String>()
        )));
    }
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| Error::Network(format!("prices: invalid JSON: {e}")))?;
    Ok(parse_prices(&value))
}

/// Shape check shared with the tests: only object-valued entries with finite
/// numeric currency values survive. `null` prices (CoinGecko emits them for
/// delisted ids) and non-finite floats are dropped instead of poisoning the
/// fiat math downstream.
fn parse_prices(value: &serde_json::Value) -> HashMap<String, HashMap<String, f64>> {
    let mut out = HashMap::new();
    if let Some(object) = value.as_object() {
        for (id, currencies_value) in object {
            let Some(map) = currencies_value.as_object() else {
                continue;
            };
            let mut entry = HashMap::new();
            for (currency, price) in map {
                let Some(price) = price.as_f64() else {
                    continue;
                };
                if !price.is_finite() {
                    continue;
                }
                entry.insert(currency.clone(), price);
            }
            if !entry.is_empty() {
                out.insert(id.clone(), entry);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serve exactly one GET with a canned body and return the request line
    /// plus headers the client actually sent (monero.rs mock pattern).
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
        (format!("http://{addr}/simple/price"), handle)
    }

    fn client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn parses_spot_price_shape() {
        let (url, server) =
            serve_once(r#"{"bitcoin":{"usd":65000.5,"cny":470000},"monero":{"usd":180}}"#).await;
        let table = fetch_prices_from(&client(), &url, &["bitcoin", "monero"], &["usd", "cny"])
            .await
            .unwrap();
        assert_eq!(table["bitcoin"]["usd"], 65000.5);
        assert_eq!(table["bitcoin"]["cny"], 470000.0);
        assert_eq!(table["monero"]["usd"], 180.0);
        assert!(!table["monero"].contains_key("cny"));

        let request = server.await.unwrap();
        assert!(request.starts_with("GET /simple/price"));
        assert!(request.contains("ids=bitcoin,monero"));
        assert!(request.contains("vs_currencies=usd,cny"));
    }

    #[tokio::test]
    async fn drops_null_prices() {
        // CoinGecko answers with null for delisted ids; the fiat math must
        // not receive them (a null price would surface as "—" or worse).
        let (url, server) =
            serve_once(r#"{"bitcoin":{"usd":65000},"deadcoin":{"usd":null}}"#).await;
        let table = fetch_prices_from(&client(), &url, &["bitcoin", "deadcoin"], &["usd"])
            .await
            .unwrap();
        assert_eq!(table["bitcoin"]["usd"], 65000.0);
        assert!(!table.contains_key("deadcoin"));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn http_error_carries_status() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 16 * 1024];
            let read = socket.read(&mut buf).await.unwrap();
            let response = format!(
                "HTTP/1.1 429 Too Many Requests\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                6,
                "throttle"
            );
            socket.write_all(response.as_bytes()).await.unwrap();
            let _ = read;
        });
        let url = format!("http://{addr}/simple/price");
        let error = fetch_prices_from(&client(), &url, &["bitcoin"], &["usd"])
            .await
            .unwrap_err();
        assert!(error.to_string().contains("429"), "got: {error}");
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn malformed_json_is_an_error() {
        let (url, server) = serve_once(r#"{"bitcoin": {"usd": tru}}"#).await;
        let error = fetch_prices_from(&client(), &url, &["bitcoin"], &["usd"])
            .await
            .unwrap_err();
        assert!(error.to_string().contains("invalid JSON"), "got: {error}");
        server.await.unwrap();
    }

    #[tokio::test]
    async fn string_typed_price_yields_empty_table_not_an_error() {
        // 校验过的 CoinGecko 响应里价格永远是数字;若源返回字符串(代理
        // 篡改/接口变更),宽松解析成空表——上游用"价格服务不可用"警告
        // 与缓存兜底,不在这里硬失败。
        let (url, server) = serve_once(r#"{"bitcoin":{"usd":"sixty-five-thousand"}}"#).await;
        let table = fetch_prices_from(&client(), &url, &["bitcoin"], &["usd"])
            .await
            .unwrap();
        assert!(table.is_empty());
        server.await.unwrap();
    }

    #[tokio::test]
    async fn empty_inputs_short_circuit_without_network() {
        // No server to connect to — must return before any request is made.
        let table = fetch_prices_from(&client(), "http://127.0.0.1:1/simple/price", &[], &["usd"])
            .await
            .unwrap();
        assert!(table.is_empty());
        let table = fetch_prices_from(
            &client(),
            "http://127.0.0.1:1/simple/price",
            &["bitcoin"],
            &[],
        )
        .await
        .unwrap();
        assert!(table.is_empty());
    }
}
