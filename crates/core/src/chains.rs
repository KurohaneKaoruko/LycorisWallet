use serde::{Deserialize, Serialize};
use std::fmt;

/// The supported chains. Every chain in this list has full receive support,
/// and all of them can sign native transfers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainId {
    Bitcoin,
    Litecoin,
    Dogecoin,
    BitcoinCash,
    Ethereum,
    #[serde(rename = "ethereum-classic")]
    EthereumClassic,
    Bsc,
    Polygon,
    Solana,
    Monero,
}

impl ChainId {
    pub const ALL: [ChainId; 10] = [
        ChainId::Bitcoin,
        ChainId::Litecoin,
        ChainId::Dogecoin,
        ChainId::BitcoinCash,
        ChainId::Ethereum,
        ChainId::EthereumClassic,
        ChainId::Bsc,
        ChainId::Polygon,
        ChainId::Solana,
        ChainId::Monero,
    ];

    pub fn info(self) -> ChainInfo {
        match self {
            ChainId::Bitcoin => ChainInfo {
                id: self,
                name: "Bitcoin",
                symbol: "BTC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Sha256,
                bip44_coin: 0,
                derivation_path: "m/84'/0'/0'/0/0",
                accent: "#F7931A",
                explorer: "https://mempool.space",
                explorer_tx: "https://mempool.space/tx/",
                explorer_address: "https://mempool.space/address/",
                rpc: "https://mempool.space/api",
                price_id: "bitcoin",
                send_note: "Native SegWit (Bech32)",
            },
            ChainId::Litecoin => ChainInfo {
                id: self,
                name: "Litecoin",
                symbol: "LTC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Scrypt,
                bip44_coin: 2,
                derivation_path: "m/84'/2'/0'/0/0",
                accent: "#345D9D",
                explorer: "https://litecoinspace.org",
                explorer_tx: "https://litecoinspace.org/tx/",
                explorer_address: "https://litecoinspace.org/address/",
                rpc: "https://litecoinspace.org/api",
                price_id: "litecoin",
                send_note: "Native SegWit (Bech32)",
            },
            ChainId::Dogecoin => ChainInfo {
                id: self,
                name: "Dogecoin",
                symbol: "DOGE",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Scrypt,
                bip44_coin: 3,
                derivation_path: "m/44'/3'/0'/0/0",
                accent: "#C2A633",
                explorer: "https://blockchair.com/dogecoin",
                explorer_tx: "https://blockchair.com/dogecoin/transaction/",
                explorer_address: "https://blockchair.com/dogecoin/address/",
                rpc: "https://api.blockchair.com/dogecoin",
                price_id: "dogecoin",
                send_note: "Legacy P2PKH (Base58)",
            },
            ChainId::BitcoinCash => ChainInfo {
                id: self,
                name: "Bitcoin Cash",
                symbol: "BCH",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Sha256,
                bip44_coin: 145,
                derivation_path: "m/44'/145'/0'/0/0",
                accent: "#8DC351",
                explorer: "https://blockchair.com/bitcoin-cash",
                explorer_tx: "https://blockchair.com/bitcoin-cash/transaction/",
                explorer_address: "https://blockchair.com/bitcoin-cash/address/",
                rpc: "https://api.blockchair.com/bitcoin-cash",
                price_id: "bitcoin-cash",
                send_note: "Legacy P2PKH / cashaddr",
            },
            ChainId::Ethereum => ChainInfo {
                id: self,
                name: "Ethereum",
                symbol: "ETH",
                decimals: 18,
                kind: ChainKind::Evm,
                hash: HashAlgorithm::Keccak256,
                bip44_coin: 60,
                derivation_path: "m/44'/60'/0'/0/0",
                accent: "#627EEA",
                explorer: "https://etherscan.io",
                explorer_tx: "https://etherscan.io/tx/",
                explorer_address: "https://etherscan.io/address/",
                rpc: "https://ethereum-rpc.publicnode.com",
                price_id: "ethereum",
                send_note: "EIP-1559",
            },
            ChainId::Bsc => ChainInfo {
                id: self,
                name: "BNB Chain",
                symbol: "BNB",
                decimals: 18,
                kind: ChainKind::Evm,
                hash: HashAlgorithm::Keccak256,
                bip44_coin: 60,
                derivation_path: "m/44'/60'/0'/0/0",
                accent: "#F3BA2F",
                explorer: "https://bscscan.com",
                explorer_tx: "https://bscscan.com/tx/",
                explorer_address: "https://bscscan.com/address/",
                rpc: "https://bsc-rpc.publicnode.com",
                price_id: "binancecoin",
                send_note: "Legacy gas · EIP-155",
            },
            ChainId::Polygon => ChainInfo {
                id: self,
                name: "Polygon",
                symbol: "POL",
                decimals: 18,
                kind: ChainKind::Evm,
                hash: HashAlgorithm::Keccak256,
                bip44_coin: 60,
                derivation_path: "m/44'/60'/0'/0/0",
                accent: "#8247E5",
                explorer: "https://polygonscan.com",
                explorer_tx: "https://polygonscan.com/tx/",
                explorer_address: "https://polygonscan.com/address/",
                rpc: "https://polygon-bor-rpc.publicnode.com",
                price_id: "polygon-ecosystem-token",
                send_note: "EIP-1559",
            },
            ChainId::EthereumClassic => ChainInfo {
                id: self,
                name: "Ethereum Classic",
                symbol: "ETC",
                decimals: 18,
                kind: ChainKind::Evm,
                hash: HashAlgorithm::Keccak256,
                bip44_coin: 61,
                derivation_path: "m/44'/61'/0'/0/0",
                accent: "#3AB83A",
                explorer: "https://blockscout.com/etc/mainnet",
                explorer_tx: "https://blockscout.com/etc/mainnet/tx/",
                explorer_address: "https://blockscout.com/etc/mainnet/address/",
                rpc: "https://etc.rubypass.workers.dev",
                price_id: "ethereum-classic",
                send_note: "Legacy gas · EIP-155 replay protection",
            },
            ChainId::Solana => ChainInfo {
                id: self,
                name: "Solana",
                symbol: "SOL",
                decimals: 9,
                kind: ChainKind::Ed25519,
                hash: HashAlgorithm::Sha256,
                bip44_coin: 501,
                derivation_path: "m/44'/501'/0'/0'",
                accent: "#14F195",
                explorer: "https://solscan.io",
                explorer_tx: "https://solscan.io/tx/",
                explorer_address: "https://solscan.io/account/",
                rpc: "https://api.mainnet-beta.solana.com",
                price_id: "solana",
                send_note: "Ed25519 · System transfer",
            },
            ChainId::Monero => ChainInfo {
                id: self,
                name: "Monero",
                symbol: "XMR",
                decimals: 12,
                kind: ChainKind::Cryptonote,
                hash: HashAlgorithm::Keccak256,
                bip44_coin: 128,
                derivation_path: "m/44'/128'/0'/0/0",
                accent: "#FF6600",
                explorer: "https://xmrchain.net",
                explorer_tx: "https://xmrchain.net/tx/",
                explorer_address: "https://xmrchain.net/search?value=",
                rpc: "https://xmrchain.net/api",
                price_id: "monero",
                send_note: "CryptoNote · 双密钥（RingCT 转账即将支持）",
            },
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|chain| chain.as_str().eq_ignore_ascii_case(id))
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ChainId::Bitcoin => "bitcoin",
            ChainId::Litecoin => "litecoin",
            ChainId::Dogecoin => "dogecoin",
            ChainId::BitcoinCash => "bitcoincash",
            ChainId::Ethereum => "ethereum",
            ChainId::EthereumClassic => "ethereum-classic",
            ChainId::Bsc => "bsc",
            ChainId::Polygon => "polygon",
            ChainId::Solana => "solana",
            ChainId::Monero => "monero",
        }
    }

    /// EIP-155 chain id used for signing and replay protection.
    pub fn evm_chain_id(self) -> Option<u64> {
        match self {
            ChainId::Ethereum => Some(1),
            ChainId::EthereumClassic => Some(61),
            ChainId::Bsc => Some(56),
            ChainId::Polygon => Some(137),
            _ => None,
        }
    }

    /// True when the chain supports EIP-1559 fee logic end to end.
    pub fn supports_eip1559(self) -> bool {
        matches!(self, ChainId::Ethereum | ChainId::Polygon)
    }

    pub fn utxo_params(self) -> Option<UtxoParams> {
        match self {
            ChainId::Bitcoin => Some(UtxoParams {
                hrp: "bc",
                p2pkh_version: 0x00,
                p2sh_version: 0x05,
                wif_version: 0x80,
            }),
            ChainId::Litecoin => Some(UtxoParams {
                hrp: "ltc",
                p2pkh_version: 0x30,
                p2sh_version: 0x32,
                wif_version: 0xb0,
            }),
            ChainId::Dogecoin => Some(UtxoParams {
                hrp: "doge",
                p2pkh_version: 0x1e,
                p2sh_version: 0x16,
                wif_version: 0x9e,
            }),
            ChainId::BitcoinCash => Some(UtxoParams {
                hrp: "bitcoincash",
                p2pkh_version: 0x00,
                p2sh_version: 0x05,
                wif_version: 0x80,
            }),
            _ => None,
        }
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChainKind {
    Utxo,
    Evm,
    Ed25519,
    Cryptonote,
}

impl ChainKind {
    pub fn address_format(self) -> &'static str {
        match self {
            ChainKind::Utxo => "bech32 / base58check",
            ChainKind::Evm => "EIP-55 hex",
            ChainKind::Ed25519 => "base58",
            ChainKind::Cryptonote => "Monero base58",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    Sha256,
    Keccak256,
    Scrypt,
}

impl HashAlgorithm {
    pub fn label(self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "SHA-256",
            HashAlgorithm::Keccak256 => "Keccak-256",
            HashAlgorithm::Scrypt => "scrypt",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "FIPS 180-4 · 32 byte digest",
            HashAlgorithm::Keccak256 => "Ethereum native hash · 32 byte digest",
            HashAlgorithm::Scrypt => "N=16384, r=8, p=1 · 32 byte digest",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainInfo {
    pub id: ChainId,
    pub name: &'static str,
    pub symbol: &'static str,
    pub decimals: u8,
    pub kind: ChainKind,
    pub hash: HashAlgorithm,
    pub bip44_coin: u32,
    pub derivation_path: &'static str,
    pub accent: &'static str,
    pub explorer: &'static str,
    pub explorer_tx: &'static str,
    pub explorer_address: &'static str,
    pub rpc: &'static str,
    pub price_id: &'static str,
    pub send_note: &'static str,
}

impl ChainInfo {
    pub fn send_supported(&self) -> bool {
        // Legacy-P2PKH signing for Dogecoin / Bitcoin Cash and RingCT signing
        // for Monero land in follow-up changes; receive and history already work.
        !matches!(
            self.id,
            ChainId::Dogecoin | ChainId::BitcoinCash | ChainId::Monero
        )
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UtxoParams {
    pub hrp: &'static str,
    pub p2pkh_version: u8,
    pub p2sh_version: u8,
    pub wif_version: u8,
}
