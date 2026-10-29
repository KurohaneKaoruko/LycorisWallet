//! Read-only chain adapters and broadcasting helpers.
//!
//! All network access is isolated here so the wallet can be audited, mocked or
//! replaced with private endpoints without touching the crypto code.

pub mod blockbook;
pub mod blockchair;
pub mod blockcypher;
pub mod chainz;
pub mod evm;
pub mod history;
pub mod monero;
pub mod prices;
pub mod solana;
pub mod utxo;
pub mod woc;

use std::time::Duration;

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(35))
        .connect_timeout(Duration::from_secs(15))
        .user_agent("LycorisWallet/0.6 (+https://github.com/KurohaneKaoruko/LycorisWallet)")
        // 本机 IPv6 出口经常不可达或被干扰(表现为间歇性连接超时), 强制 IPv4。
        .local_address(Some(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}
