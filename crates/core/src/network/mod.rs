//! Read-only chain adapters and broadcasting helpers.
//!
//! All network access is isolated here so the wallet can be audited, mocked or
//! replaced with private endpoints without touching the crypto code.

pub mod blockchair;
pub mod evm;
pub mod prices;
pub mod solana;
pub mod utxo;

use std::time::Duration;

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(10))
        .user_agent("LycorisWallet/0.1 (+https://example.invalid)")
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}
