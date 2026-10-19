//! Spot prices from CoinGecko.

use crate::error::{Error, Result};
use std::collections::HashMap;

/// Fetch spot prices from CoinGecko. Returns `id -> currency -> price`.
pub async fn fetch_prices(
    client: &reqwest::Client,
    ids: &[&str],
    currencies: &[&str],
) -> Result<HashMap<String, HashMap<String, f64>>> {
    if ids.is_empty() || currencies.is_empty() {
        return Ok(HashMap::new());
    }
    let url = format!(
        "https://api.coingecko.com/api/v3/simple/price?ids={}&vs_currencies={}",
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
    let mut out = HashMap::new();
    if let Some(object) = value.as_object() {
        for (id, currencies_value) in object {
            let mut entry = HashMap::new();
            if let Some(map) = currencies_value.as_object() {
                for (currency, price) in map {
                    if let Some(price) = price.as_f64() {
                        entry.insert(currency.clone(), price);
                    }
                }
            }
            out.insert(id.clone(), entry);
        }
    }
    Ok(out)
}
