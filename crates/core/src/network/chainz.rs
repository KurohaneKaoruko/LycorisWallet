//! Chainz (cryptoid.info) read-only balance adapter for the small UTXO
//! chains that have no public Blockbook/Esplora host: Namecoin, Litecoin
//! Cash, Potcoin, e-Gulden, Vertcoin.
//!
//! The API is free of charge and keyless (`chainz.cryptoid.info/api.dws`);
//! it answers a plain decimal coin-unit number, so this adapter is
//! balance-only — spending needs a UTXO source those chains do not expose.

use crate::error::{Error, Result};

#[derive(Clone)]
pub struct ChainzClient {
    client: reqwest::Client,
    /// e.g. `https://chainz.cryptoid.info/pot`
    base: String,
    decimals: u8,
}

impl ChainzClient {
    pub fn new(client: reqwest::Client, coin: &str, decimals: u8) -> Self {
        Self {
            client,
            base: format!("https://chainz.cryptoid.info/{coin}"),
            decimals,
        }
    }

    /// Confirmed balance of an address, in smallest units.
    pub async fn balance(&self, address: &str) -> Result<u64> {
        let url = format!("{}/api.dws?q=getbalance&a={address}", self.base);
        let response = self
            .client
            .get(&url)
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
        // Plain decimal coin units, e.g. "1234.5678" (no quotes, no object).
        parse_decimal(text.trim(), self.decimals)
            .ok_or_else(|| Error::Network(format!("balance: unexpected payload {text:?}")))
    }
}

/// "1234.5678" with 8 decimals → 123456780000, rounding halves up.
pub fn parse_decimal(payload: &str, decimals: u8) -> Option<u64> {
    let (whole, frac) = match payload.split_once('.') {
        // Plain integers ("0", "42") are valid balances too.
        None => (payload, ""),
        Some(parts) => parts,
    };
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: u64 = whole.parse().ok()?;
    let scale = u64::checked_pow(10, u32::from(decimals))?;
    let value = whole.checked_mul(scale)?;
    if frac.is_empty() {
        return Some(value);
    }
    // Keep the leading decimals digits; round on the first dropped digit.
    let mut frac_digits: Vec<u8> = frac.bytes().map(|b| b - b'0').collect();
    let round_up = frac_digits
        .get(usize::from(decimals))
        .is_some_and(|digit| *digit >= 5);
    frac_digits.truncate(usize::from(decimals));
    while frac_digits.len() < usize::from(decimals) {
        frac_digits.push(0);
    }
    let mut frac_value = 0u64;
    for digit in frac_digits {
        frac_value = frac_value * 10 + u64::from(digit);
    }
    if round_up {
        frac_value = frac_value.checked_add(1)?;
    }
    value.checked_add(frac_value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_balances() {
        assert_eq!(parse_decimal("0", 8), Some(0));
        assert_eq!(parse_decimal("1234.5678", 8), Some(123_456_780_000));
        assert_eq!(parse_decimal("1.5", 8), Some(150_000_000));
        // Fraction shorter than the scale pads with zeros.
        assert_eq!(parse_decimal("2.25", 8), Some(225_000_000));
        // Longer fractions round half-up at the chain's precision.
        assert_eq!(parse_decimal("1.000000005", 8), Some(100_000_001));
        assert_eq!(parse_decimal("1.000000004", 8), Some(100_000_000));
        // Garbage is rejected, never guessed.
        assert_eq!(parse_decimal("", 8), None);
        assert_eq!(parse_decimal("abc", 8), None);
        assert_eq!(parse_decimal("1.2.3", 8), None);
        assert_eq!(parse_decimal("-1.0", 8), None);
    }
}
