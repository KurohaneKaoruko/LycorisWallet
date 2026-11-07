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
pub mod insight;
pub mod monero;
pub mod prices;
pub mod solana;
pub mod utxo;
pub mod woc;

use std::time::Duration;

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        // 死源(超时主机)不应拖死回退链:总 20s/连接 8s 意味着主源不可达时
        // 最多 ~20s 就轮到下一个源。Monero 用自己的长超时客户端,不受影响。
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(8))
        // UA 版本跟随 crate 版本，避免升版后 UA 仍停在旧号。
        .user_agent(concat!(
            "LycorisWallet/",
            env!("CARGO_PKG_VERSION"),
            " (+https://github.com/KurohaneKaoruko/LycorisWallet)"
        ))
        // 本机 IPv6 出口经常不可达或被干扰(表现为间歇性连接超时), 强制 IPv4。
        .local_address(Some(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED)))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}
