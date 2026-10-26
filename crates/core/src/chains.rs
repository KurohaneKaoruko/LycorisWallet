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
    BitcoinSv,
    Namecoin,
    LitecoinCash,
    Feathercoin,
    Viacoin,
    Potcoin,
    EGulden,
    Dash,
    Groestlcoin,
    Vertcoin,
}

impl ChainId {
    pub const ALL: [ChainId; 20] = [
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
        ChainId::BitcoinSv,
        ChainId::Namecoin,
        ChainId::LitecoinCash,
        ChainId::Feathercoin,
        ChainId::Viacoin,
        ChainId::Potcoin,
        ChainId::EGulden,
        ChainId::Dash,
        ChainId::Groestlcoin,
        ChainId::Vertcoin,
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
                send_note: "CryptoNote · RingCT（经 monero-wallet-rpc 签名）",
            },
            ChainId::BitcoinSv => ChainInfo {
                id: self,
                name: "Bitcoin SV",
                symbol: "BSV",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Sha256,
                bip44_coin: 236,
                derivation_path: "m/44'/236'/0'/0/0",
                accent: "#EAB300",
                explorer: "https://whatsonchain.com",
                explorer_tx: "https://whatsonchain.com/tx/",
                explorer_address: "https://whatsonchain.com/address/",
                rpc: "https://api.whatsonchain.com/v1/bsv/main",
                price_id: "bitcoin-sv",
                send_note: "Legacy P2PKH（经典 sighash）",
            },
            ChainId::Namecoin => ChainInfo {
                id: self,
                name: "Namecoin",
                symbol: "NMC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Sha256,
                bip44_coin: 7,
                derivation_path: "m/44'/7'/0'/0/0",
                accent: "#186C9D",
                explorer: "https://nmc.blockbook.trezor.io",
                explorer_tx: "https://nmc.blockbook.trezor.io/tx/",
                explorer_address: "https://nmc.blockbook.trezor.io/address/",
                rpc: "https://nmc.blockbook.trezor.io/api",
                price_id: "namecoin",
                send_note: "Legacy P2PKH（经典 sighash）",
            },
            ChainId::LitecoinCash => ChainInfo {
                id: self,
                name: "Litecoin Cash",
                symbol: "LCC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Sha256,
                bip44_coin: 192,
                derivation_path: "m/44'/192'/0'/0/0",
                accent: "#BEBEBE",
                explorer: "https://explorer.litecoinca.sh",
                explorer_tx: "https://explorer.litecoinca.sh/tx/",
                explorer_address: "https://explorer.litecoinca.sh/address/",
                rpc: "https://explorer.litecoinca.sh/api",
                price_id: "litecoin-cash",
                send_note: "Legacy P2PKH（经典 sighash）",
            },
            ChainId::Feathercoin => ChainInfo {
                id: self,
                name: "Feathercoin",
                symbol: "FTC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Scrypt,
                bip44_coin: 8,
                derivation_path: "m/44'/8'/0'/0/0",
                accent: "#C6E4EE",
                explorer: "https://ftc.blockbook.trezor.io",
                explorer_tx: "https://ftc.blockbook.trezor.io/tx/",
                explorer_address: "https://ftc.blockbook.trezor.io/address/",
                rpc: "https://ftc.blockbook.trezor.io/api",
                price_id: "feathercoin",
                send_note: "Legacy P2PKH（经典 sighash）",
            },
            ChainId::Viacoin => ChainInfo {
                id: self,
                name: "Viacoin",
                symbol: "VIA",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Scrypt,
                bip44_coin: 14,
                derivation_path: "m/44'/14'/0'/0/0",
                accent: "#404E82",
                explorer: "https://via.blockbook.trezor.io",
                explorer_tx: "https://via.blockbook.trezor.io/tx/",
                explorer_address: "https://via.blockbook.trezor.io/address/",
                rpc: "https://via.blockbook.trezor.io/api",
                price_id: "viacoin",
                send_note: "Native SegWit (Bech32)",
            },
            ChainId::Potcoin => ChainInfo {
                id: self,
                name: "Potcoin",
                symbol: "POT",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Scrypt,
                bip44_coin: 81,
                derivation_path: "m/44'/81'/0'/0/0",
                accent: "#3F8C48",
                explorer: "https://chainz.cryptoid.info/pot",
                explorer_tx: "https://chainz.cryptoid.info/pot/tx.dws?",
                explorer_address: "https://chainz.cryptoid.info/pot/address.dws?",
                rpc: "https://potcoin1.blockbook.trezor.io",
                price_id: "potcoin",
                send_note: "Legacy P2PKH（经典 sighash）",
            },
            ChainId::EGulden => ChainInfo {
                id: self,
                name: "e-Gulden",
                symbol: "EFL",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Scrypt,
                bip44_coin: 78,
                derivation_path: "m/44'/78'/0'/0/0",
                accent: "#F4812F",
                explorer: "https://efl.blockbook.trezor.io",
                explorer_tx: "https://efl.blockbook.trezor.io/tx/",
                explorer_address: "https://efl.blockbook.trezor.io/address/",
                rpc: "https://efl.blockbook.trezor.io/api",
                price_id: "e-gulden",
                send_note: "Legacy P2PKH（经典 sighash）",
            },
            ChainId::Dash => ChainInfo {
                id: self,
                name: "Dash",
                symbol: "DASH",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::X11,
                bip44_coin: 5,
                derivation_path: "m/44'/5'/0'/0/0",
                accent: "#008DE4",
                explorer: "https://blockbook.dash.zelcore.io",
                explorer_tx: "https://blockbook.dash.zelcore.io/tx/",
                explorer_address: "https://blockbook.dash.zelcore.io/address/",
                rpc: "https://blockbook.dash.zelcore.io/api",
                price_id: "dash",
                send_note: "Legacy P2PKH（经典 sighash）",
            },
            ChainId::Groestlcoin => ChainInfo {
                id: self,
                name: "Groestlcoin",
                symbol: "GRS",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Groestl,
                bip44_coin: 17,
                derivation_path: "m/44'/17'/0'/0/0",
                accent: "#36855A",
                explorer: "https://grs.blockbook.trezor.io",
                explorer_tx: "https://grs.blockbook.trezor.io/tx/",
                explorer_address: "https://grs.blockbook.trezor.io/address/",
                rpc: "https://grs.blockbook.trezor.io/api",
                price_id: "groestlcoin",
                send_note: "Native SegWit (Bech32)",
            },
            ChainId::Vertcoin => ChainInfo {
                id: self,
                name: "Vertcoin",
                symbol: "VTC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Lyra2re,
                bip44_coin: 28,
                derivation_path: "m/44'/28'/0'/0/0",
                accent: "#5BB5A0",
                explorer: "https://vtc.blockbook.trezor.io",
                explorer_tx: "https://vtc.blockbook.trezor.io/tx/",
                explorer_address: "https://vtc.blockbook.trezor.io/address/",
                rpc: "https://vtc.blockbook.trezor.io/api",
                price_id: "vertcoin",
                send_note: "Native SegWit (Bech32)",
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
            ChainId::BitcoinSv => "bitcoinsv",
            ChainId::Namecoin => "namecoin",
            ChainId::LitecoinCash => "litecoincash",
            ChainId::Feathercoin => "feathercoin",
            ChainId::Viacoin => "viacoin",
            ChainId::Potcoin => "potcoin",
            ChainId::EGulden => "egulden",
            ChainId::Dash => "dash",
            ChainId::Groestlcoin => "groestlcoin",
            ChainId::Vertcoin => "vertcoin",
        }
    }

    /// True when the chain spends with legacy (pre-SegWit) P2PKH inputs.
    ///
    /// Dogecoin and Bitcoin Cash sign a different digest than the SegWit
    /// chains: Dogecoin uses the classic pre-BIP143 sighash, Bitcoin Cash the
    /// BIP143 digest with `SIGHASH_FORKID`.
    pub fn legacy_utxo(self) -> bool {
        matches!(
            self,
            ChainId::Dogecoin
                | ChainId::BitcoinCash
                | ChainId::BitcoinSv
                | ChainId::Namecoin
                | ChainId::LitecoinCash
                | ChainId::Feathercoin
                | ChainId::Potcoin
                | ChainId::EGulden
                | ChainId::Dash
        )
    }

    /// Which digest a legacy input is signed with.
    pub fn legacy_sighash(self) -> crate::tx::utxo::LegacySighash {
        match self {
            ChainId::BitcoinCash => crate::tx::utxo::LegacySighash::ForkId,
            _ => crate::tx::utxo::LegacySighash::Classic,
        }
    }

    /// The name of the chain's smallest unit, as its community calls it.
    ///
    /// Bitcoin and Bitcoin Cash count in satoshis, Litecoin in litoshis and
    /// Dogecoin in shibas.
    pub fn base_unit(self) -> &'static str {
        match self {
            ChainId::Bitcoin | ChainId::BitcoinCash | ChainId::BitcoinSv => "聪",
            ChainId::Litecoin => "里特",
            ChainId::LitecoinCash => "里特现金",
            ChainId::Dogecoin => "柴",
            ChainId::Namecoin => "聪", // 与比特币同构,社区无专名
            ChainId::Feathercoin => "羽",
            ChainId::Viacoin => "维亚",
            ChainId::Potcoin => "锅",
            ChainId::EGulden => "格罗申",
            ChainId::Dash => "duff",
            ChainId::Groestlcoin => "gros",
            ChainId::Vertcoin => "弗特",
            ChainId::Ethereum | ChainId::EthereumClassic | ChainId::Bsc | ChainId::Polygon => {
                "gwei"
            }
            ChainId::Solana => "lamports",
            ChainId::Monero => "piconero",
        }
    }

    /// Fallback fee rate in satoshis per virtual byte, used when the chain's
    /// API does not publish one.
    ///
    /// Dogecoin relays at 0.01 DOGE per kilobyte, Bitcoin Cash accepts a
    /// single satoshi per byte.
    pub fn default_fee_rate(self) -> u64 {
        match self {
            ChainId::Dogecoin => 1_000,
            ChainId::BitcoinCash => 2,
            _ => 1,
        }
    }

    /// The smallest output the network will relay, in the chain's smallest
    /// unit. Anything below it can never be spent again.
    pub fn dust_limit(self) -> u64 {
        match self {
            // Dogecoin dust is one whole DOGE.
            ChainId::Dogecoin => 100_000_000,
            _ => 546,
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
            // Same address version bytes as Bitcoin (BSV kept the legacy
            // format when it split from BCH).
            ChainId::BitcoinSv => Some(UtxoParams {
                hrp: "bsv",
                p2pkh_version: 0x00,
                p2sh_version: 0x05,
                wif_version: 0x80,
            }),
            ChainId::Namecoin => Some(UtxoParams {
                hrp: "nmc",
                p2pkh_version: 0x34,
                p2sh_version: 0x0d,
                wif_version: 0xb4,
            }),
            ChainId::LitecoinCash => Some(UtxoParams {
                hrp: "lcc",
                p2pkh_version: 0x1c,
                p2sh_version: 0x05,
                wif_version: 0xb0,
            }),
            ChainId::Feathercoin => Some(UtxoParams {
                hrp: "fc",
                p2pkh_version: 0x0e,
                p2sh_version: 0x05,
                wif_version: 0x8e,
            }),
            ChainId::Viacoin => Some(UtxoParams {
                hrp: "via",
                p2pkh_version: 0x47,
                p2sh_version: 0x21,
                wif_version: 0xc7,
            }),
            ChainId::Potcoin => Some(UtxoParams {
                hrp: "pot",
                p2pkh_version: 0x37,
                p2sh_version: 0x05,
                wif_version: 0xb7,
            }),
            ChainId::EGulden => Some(UtxoParams {
                hrp: "efl",
                p2pkh_version: 0x30,
                p2sh_version: 0x05,
                wif_version: 0xb0,
            }),
            ChainId::Dash => Some(UtxoParams {
                hrp: "dash",
                p2pkh_version: 0x4c,
                p2sh_version: 0x10,
                wif_version: 0xcc,
            }),
            ChainId::Groestlcoin => Some(UtxoParams {
                hrp: "grs",
                p2pkh_version: 0x24,
                p2sh_version: 0x05,
                wif_version: 0x80,
            }),
            ChainId::Vertcoin => Some(UtxoParams {
                hrp: "vtc",
                p2pkh_version: 0x47,
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
    X11,
    Groestl,
    Lyra2re,
}

impl HashAlgorithm {
    pub fn label(self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "SHA-256",
            HashAlgorithm::Keccak256 => "Keccak-256",
            HashAlgorithm::Scrypt => "scrypt",
            HashAlgorithm::X11 => "X11",
            HashAlgorithm::Groestl => "Groestl",
            HashAlgorithm::Lyra2re => "Lyra2RE",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "FIPS 180-4 · 32 byte digest",
            HashAlgorithm::Keccak256 => "Ethereum native hash · 32 byte digest",
            HashAlgorithm::Scrypt => "N=16384, r=8, p=1 · 32 byte digest",
            HashAlgorithm::X11 => "Dash PoW 哈希 · 32 byte digest",
            HashAlgorithm::Groestl => "Groestl-512 → 前 32 字节",
            HashAlgorithm::Lyra2re => "Vertcoin PoW 哈希链",
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
        // Monero spends are signed by a local `monero-wallet-rpc`; when it is
        // not running the quote explains how to start it.
        true
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UtxoParams {
    pub hrp: &'static str,
    pub p2pkh_version: u8,
    pub p2sh_version: u8,
    pub wif_version: u8,
}
