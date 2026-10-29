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
    #[serde(rename = "ethw")]
    Ethw,
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
    Ravencoin,
    Monacoin,
    Fujicoin,
    Zcash,
    Firo,
    Neurai,
    Clore,
    Zephyr,
    OctaSpace,
}

impl ChainId {
    pub const ALL: [ChainId; 30] = [
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
        ChainId::Ravencoin,
        ChainId::Monacoin,
        ChainId::Fujicoin,
        ChainId::Ethw,
        ChainId::Zcash,
        ChainId::Firo,
        ChainId::Neurai,
        ChainId::Clore,
        ChainId::Zephyr,
        ChainId::OctaSpace,
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
                explorer: "https://doge.trezor.io",
                explorer_tx: "https://doge.trezor.io/tx/",
                explorer_address: "https://doge.trezor.io/address/",
                // Trezor Suite's own public Blockbook — keyless, no rate plan.
                rpc: "https://doge.trezor.io",
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
                explorer: "https://bch.trezor.io",
                explorer_tx: "https://bch.trezor.io/tx/",
                explorer_address: "https://bch.trezor.io/address/",
                // Trezor Suite's own public Blockbook — keyless, no rate plan.
                rpc: "https://bch.trezor.io",
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
                rpc: "https://etc.etcdesktop.com",
                price_id: "ethereum-classic",
                send_note: "Legacy gas · EIP-155 replay protection",
            },
            ChainId::Ethw => ChainInfo {
                id: self,
                name: "EthereumPoW",
                symbol: "ETHW",
                decimals: 18,
                kind: ChainKind::Evm,
                hash: HashAlgorithm::Keccak256,
                // SLIP-44 registered 10001 for ETHW; the EVM chain id is the
                // same number (the merge fork chose 10001).
                bip44_coin: 10001,
                derivation_path: "m/44'/10001'/0'/0/0",
                accent: "#9C6CE8",
                explorer: "https://www.oklink.com/ethw",
                explorer_tx: "https://www.oklink.com/ethw/tx/",
                explorer_address: "https://www.oklink.com/ethw/address/",
                // Official public RPC (ethereumpow.org), verified live:
                // eth_chainId → 0x2711 (10001), balance/block/gas answer.
                rpc: "https://mainnet.ethereumpow.org",
                price_id: "ethereum-pow",
                send_note: "Legacy gas · Etchash PoW 链",
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
                rpc: "https://solana-rpc.publicnode.com",
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
                explorer: "https://namecha.in/",
                explorer_tx: "https://namecha.in/tx/",
                explorer_address: "https://namecha.in/address/",
                // No public index API: balance/history gracefully report
                // "no data source"; the explorer link above still works.
                rpc: "",
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
                explorer: "https://chainz.cryptoid.info/lcc/",
                explorer_tx: "https://chainz.cryptoid.info/lcc/tx.dws?h=",
                explorer_address: "https://chainz.cryptoid.info/lcc/address.dws?u=",
                // No public index API (chainz is explorer-only).
                rpc: "",
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
                explorer: "https://explorer.feathercoin.com",
                explorer_tx: "https://explorer.feathercoin.com/tx/",
                explorer_address: "https://explorer.feathercoin.com/address/",
                rpc: "https://explorer.feathercoin.com",
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
                explorer: "https://blockbook.viacoin.org",
                explorer_tx: "https://blockbook.viacoin.org/tx/",
                explorer_address: "https://blockbook.viacoin.org/address/",
                rpc: "https://blockbook.viacoin.org",
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
                explorer: "https://chainz.cryptoid.info/pot/",
                explorer_tx: "https://chainz.cryptoid.info/pot/tx.dws?h=",
                explorer_address: "https://chainz.cryptoid.info/pot/address.dws?u=",
                // No public index API (chainz is explorer-only).
                rpc: "",
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
                explorer: "https://bitinfocharts.com/e-gulden/explorer/",
                explorer_tx: "https://bitinfocharts.com/e-gulden/explorer/",
                explorer_address: "https://bitinfocharts.com/e-gulden/explorer/",
                // No public index API (bitinfocharts is explorer-only).
                rpc: "",
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
                rpc: "https://blockbook.dash.zelcore.io",
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
                explorer: "https://blockbook.groestlcoin.org",
                explorer_tx: "https://blockbook.groestlcoin.org/tx/",
                explorer_address: "https://blockbook.groestlcoin.org/address/",
                rpc: "https://blockbook.groestlcoin.org",
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
                explorer: "https://bitinfocharts.com/vertcoin/explorer/",
                explorer_tx: "https://bitinfocharts.com/vertcoin/explorer/",
                explorer_address: "https://bitinfocharts.com/vertcoin/explorer/",
                // No public index API (bitinfocharts is explorer-only).
                rpc: "",
                price_id: "vertcoin",
                send_note: "Native SegWit (Bech32)",
            },
            ChainId::Ravencoin => ChainInfo {
                id: self,
                name: "Ravencoin",
                symbol: "RVN",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::KawPow,
                bip44_coin: 175,
                derivation_path: "m/44'/175'/0'/0/0",
                accent: "#B7B7B7",
                explorer: "https://blockbook.ravencoin.org",
                explorer_tx: "https://blockbook.ravencoin.org/tx/",
                explorer_address: "https://blockbook.ravencoin.org/address/",
                rpc: "https://blockbook.ravencoin.org",
                price_id: "ravencoin",
                send_note: "Legacy P2PKH (Base58)",
            },
            ChainId::Monacoin => ChainInfo {
                id: self,
                name: "Monacoin",
                symbol: "MONA",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Lyra2re,
                bip44_coin: 22,
                derivation_path: "m/84'/22'/0'/0/0",
                accent: "#B78C33",
                explorer: "https://blockbook.electrum-mona.org",
                explorer_tx: "https://blockbook.electrum-mona.org/tx/",
                explorer_address: "https://blockbook.electrum-mona.org/address/",
                rpc: "https://blockbook.electrum-mona.org",
                price_id: "monacoin",
                send_note: "Native SegWit (Bech32 mona)",
            },
            ChainId::Fujicoin => ChainInfo {
                id: self,
                name: "Fujicoin",
                symbol: "FJC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Lyra2re,
                bip44_coin: 75,
                derivation_path: "m/84'/75'/0'/0/0",
                accent: "#B7472A",
                explorer: "https://explorer.fujicoin.org",
                explorer_tx: "https://explorer.fujicoin.org/tx/",
                explorer_address: "https://explorer.fujicoin.org/address/",
                rpc: "https://explorer.fujicoin.org",
                price_id: "fujicoin",
                send_note: "Native SegWit (Bech32 fc)",
            },
            ChainId::Zcash => ChainInfo {
                id: self,
                name: "Zcash",
                symbol: "ZEC",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Equihash,
                bip44_coin: 133,
                // 透明地址(非屏蔽)。主源 zelcore Blockbook(索引透明池),
                // Blockchair 兜底。签名走 ZIP-243(见 tx::utxo)。
                derivation_path: "m/44'/133'/0'/0/0",
                accent: "#F4B728",
                explorer: "https://blockbook.zec.zelcore.io",
                explorer_tx: "https://blockbook.zec.zelcore.io/tx/",
                explorer_address: "https://blockbook.zec.zelcore.io/address/",
                rpc: "https://blockbook.zec.zelcore.io",
                price_id: "zcash",
                send_note: "透明地址 t1… (P2PKH, ZIP-243 签名)",
            },
            ChainId::Firo => ChainInfo {
                id: self,
                name: "Firo",
                symbol: "FIRO",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::Sha256,
                bip44_coin: 136,
                derivation_path: "m/44'/136'/0'/0/0",
                accent: "#7C4DFF",
                explorer: "https://blockbook.firo.org",
                explorer_tx: "https://blockbook.firo.org/tx/",
                explorer_address: "https://blockbook.firo.org/address/",
                rpc: "https://blockbook.firo.org",
                price_id: "zcoin",
                send_note: "Legacy P2PKH (Base58 a…)",
            },
            ChainId::Neurai => ChainInfo {
                id: self,
                name: "Neurai",
                symbol: "XNA",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::KawPow,
                bip44_coin: 733,
                derivation_path: "m/44'/733'/0'/0/0",
                accent: "#00C2A8",
                explorer: "https://blockbook.neurai.org",
                explorer_tx: "https://blockbook.neurai.org/tx/",
                explorer_address: "https://blockbook.neurai.org/address/",
                rpc: "https://blockbook.neurai.org",
                price_id: "neurai",
                send_note: "Legacy P2PKH (Base58 X…)",
            },
            ChainId::Clore => ChainInfo {
                id: self,
                name: "Clore.ai",
                symbol: "CLORE",
                decimals: 8,
                kind: ChainKind::Utxo,
                hash: HashAlgorithm::KawPow,
                bip44_coin: 899,
                derivation_path: "m/44'/899'/0'/0/0",
                accent: "#33A6FF",
                explorer: "https://blockbook.clore.zelcore.io",
                explorer_tx: "https://blockbook.clore.zelcore.io/tx/",
                explorer_address: "https://blockbook.clore.zelcore.io/address/",
                rpc: "https://blockbook.clore.zelcore.io",
                price_id: "clore",
                send_note: "Legacy P2PKH (Base58 c…)",
            },
            ChainId::Zephyr => ChainInfo {
                id: self,
                name: "Zephyr Protocol",
                symbol: "ZEPH",
                decimals: 12,
                kind: ChainKind::Cryptonote,
                hash: HashAlgorithm::RandomX,
                bip44_coin: 0,
                derivation_path: "m/44'/128'/0'/0/0",
                accent: "#5E8B7E",
                explorer: "https://explorer.zephyrprotocol.com",
                explorer_tx: "https://explorer.zephyrprotocol.com/tx/",
                explorer_address: "https://explorer.zephyrprotocol.com/",
                rpc: "http://127.0.0.1:19797",
                price_id: "zephyr-protocol",
                send_note: "CryptoNote 双公钥 · 隐私币",
            },
            ChainId::OctaSpace => ChainInfo {
                id: self,
                name: "OctaSpace",
                symbol: "OCTA",
                decimals: 18,
                kind: ChainKind::Evm,
                hash: HashAlgorithm::Keccak256,
                bip44_coin: 60,
                derivation_path: "m/44'/60'/0'/0/0",
                accent: "#8A5CF6",
                explorer: "https://explorer.octa.space",
                explorer_tx: "https://explorer.octa.space/tx/",
                explorer_address: "https://explorer.octa.space/address/",
                rpc: "https://rpc.octa.space",
                price_id: "octaspace",
                send_note: "EVM · PoW (Ethash)",
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
            ChainId::Ethw => "ethw",
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
            ChainId::Ravencoin => "ravencoin",
            ChainId::Monacoin => "monacoin",
            ChainId::Fujicoin => "fujicoin",
            ChainId::Zcash => "zcash",
            ChainId::Firo => "firo",
            ChainId::Neurai => "neurai",
            ChainId::Clore => "clore",
            ChainId::Zephyr => "zephyr",
            ChainId::OctaSpace => "octaspace",
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
                // Ravencoin never activated SegWit; its R… addresses are
                // plain P2PKH with the classic pre-BIP143 digest.
                | ChainId::Ravencoin
                // Firo / Neurai / Clore are Ravencoin-family forks that never
                // activated SegWit either: plain P2PKH, classic digest.
                | ChainId::Firo
                | ChainId::Neurai
                | ChainId::Clore
        )
    }

    /// Which digest a legacy input is signed with.
    pub fn legacy_sighash(self) -> crate::tx::utxo::LegacySighash {
        match self {
            ChainId::BitcoinCash => crate::tx::utxo::LegacySighash::ForkId,
            ChainId::Zcash => crate::tx::utxo::LegacySighash::Zcash,
            _ => crate::tx::utxo::LegacySighash::Classic,
        }
    }

    /// Which index API serves this UTXO chain. Drives balance/UTXO/broadcast/
    /// history routing in the Tauri layer.
    pub fn utxo_backend(self) -> UtxoBackend {
        match self {
            // Trezor Suite's public Blockbook instances come first (keyless,
            // no rate plan); Blockchair is the configured fallback.
            ChainId::Dogecoin
            | ChainId::BitcoinCash
            | ChainId::Viacoin
            | ChainId::Feathercoin
            | ChainId::Groestlcoin
            | ChainId::Dash
            // Ravencoin / Monacoin / Fujicoin community Blockbook instances
            // (verified live: balance, UTXO and history endpoints answer).
            | ChainId::Ravencoin
            | ChainId::Monacoin
            | ChainId::Fujicoin
            // Firo / Neurai / Clore community Blockbook instances
            // (firo.org and neurai.org verified live; clore runs on
            // zelcore's Blockbook fleet).
            | ChainId::Firo
            | ChainId::Neurai
            | ChainId::Clore => UtxoBackend::Blockbook,
            ChainId::Bitcoin | ChainId::Litecoin => UtxoBackend::Esplora,
            ChainId::BitcoinSv => UtxoBackend::WhatsOnChain,
            // Zcash transparent addresses: zelcore's Blockbook indexes the
            // transparent pool; Blockchair (keyless, rate-limited) backs it
            // up. Shielded pools are out of scope by design.
            ChainId::Zcash => UtxoBackend::Blockbook,
            // Chainz-only chains: balance via the keyless cryptoid API.
            ChainId::Namecoin
            | ChainId::LitecoinCash
            | ChainId::Potcoin
            | ChainId::EGulden
            | ChainId::Vertcoin => UtxoBackend::Chainz,
            // Non-UTXO kinds never route through here; the arm keeps the
            // match exhaustive if chains are added later.
            _ => UtxoBackend::None,
        }
    }

    /// Secondary source tried when the primary backend fails, so balance
    /// queries keep working without any user configuration.
    /// Balance with automatic multi-source fallback. Sources are tried in
    /// order: primary backend first, then `fallback_backend()`, then
    /// `final_backend()` — so out-of-the-box wallets never need an API key,
    /// and a chain stays usable when its primary fleet is unreachable.
    pub fn fallback_backend(self) -> Option<UtxoBackend> {
        match self {
            ChainId::Dogecoin | ChainId::Dash => Some(UtxoBackend::Blockcypher),
            ChainId::BitcoinCash
            | ChainId::Groestlcoin
            | ChainId::Feathercoin
            | ChainId::Viacoin
            | ChainId::Zcash => Some(UtxoBackend::Blockchair),
            _ => None,
        }
    }

    /// Last-resort backend after `fallback_backend()` also fails.
    pub fn final_backend(self) -> Option<UtxoBackend> {
        match self {
            // DOGE/DASH: Blockcypher first, Blockchair behind it.
            ChainId::Dogecoin | ChainId::Dash => Some(UtxoBackend::Blockchair),
            _ => None,
        }
    }

    /// The chain's slug on chainz.cryptoid.info (see `ChainzClient`).
    pub fn chainz_coin(self) -> &'static str {
        match self {
            ChainId::Namecoin => "nmc",
            ChainId::LitecoinCash => "lcc",
            ChainId::Potcoin => "pot",
            ChainId::EGulden => "efl",
            ChainId::Vertcoin => "vtc",
            _ => "",
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
            ChainId::Ravencoin => "渡鸦",
            ChainId::Monacoin => "萌",
            ChainId::Fujicoin => "富士",
            ChainId::Zcash => "zatoshi",
            ChainId::Firo => "聪", // 比特币同构,社区无专名
            ChainId::Neurai => "聪",
            ChainId::Clore => "聪",
            ChainId::Ethereum
            | ChainId::EthereumClassic
            | ChainId::Ethw
            | ChainId::Bsc
            | ChainId::Polygon
            | ChainId::OctaSpace => "gwei",
            ChainId::Solana => "lamports",
            ChainId::Monero | ChainId::Zephyr => "piconero",
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
            ChainId::Ethw => Some(10001),
            ChainId::Bsc => Some(56),
            ChainId::Polygon => Some(137),
            // OctaSpace (docs.octa.space / chainlist 800001).
            ChainId::OctaSpace => Some(800001),
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
                p2pkh_version: &[0x00],
                p2sh_version: &[0x05],
                wif_version: 0x80,
            }),
            ChainId::Litecoin => Some(UtxoParams {
                hrp: "ltc",
                p2pkh_version: &[0x30],
                p2sh_version: &[0x32],
                wif_version: 0xb0,
            }),
            ChainId::Dogecoin => Some(UtxoParams {
                hrp: "doge",
                p2pkh_version: &[0x1e],
                p2sh_version: &[0x16],
                wif_version: 0x9e,
            }),
            ChainId::BitcoinCash => Some(UtxoParams {
                hrp: "bitcoincash",
                p2pkh_version: &[0x00],
                p2sh_version: &[0x05],
                wif_version: 0x80,
            }),
            // Same address version bytes as Bitcoin (BSV kept the legacy
            // format when it split from BCH).
            ChainId::BitcoinSv => Some(UtxoParams {
                hrp: "bsv",
                p2pkh_version: &[0x00],
                p2sh_version: &[0x05],
                wif_version: 0x80,
            }),
            ChainId::Namecoin => Some(UtxoParams {
                hrp: "nmc",
                p2pkh_version: &[0x34],
                p2sh_version: &[0x0d],
                wif_version: 0xb4,
            }),
            ChainId::LitecoinCash => Some(UtxoParams {
                hrp: "lcc",
                p2pkh_version: &[0x1c],
                p2sh_version: &[0x05],
                wif_version: 0xb0,
            }),
            ChainId::Feathercoin => Some(UtxoParams {
                hrp: "fc",
                p2pkh_version: &[0x0e],
                p2sh_version: &[0x05],
                wif_version: 0x8e,
            }),
            ChainId::Viacoin => Some(UtxoParams {
                hrp: "via",
                p2pkh_version: &[0x47],
                p2sh_version: &[0x21],
                wif_version: 0xc7,
            }),
            ChainId::Potcoin => Some(UtxoParams {
                hrp: "pot",
                p2pkh_version: &[0x37],
                p2sh_version: &[0x05],
                wif_version: 0xb7,
            }),
            ChainId::EGulden => Some(UtxoParams {
                hrp: "efl",
                p2pkh_version: &[0x30],
                p2sh_version: &[0x05],
                wif_version: 0xb0,
            }),
            ChainId::Dash => Some(UtxoParams {
                hrp: "dash",
                p2pkh_version: &[0x4c],
                p2sh_version: &[0x10],
                wif_version: 0xcc,
            }),
            ChainId::Groestlcoin => Some(UtxoParams {
                hrp: "grs",
                p2pkh_version: &[0x24],
                p2sh_version: &[0x05],
                wif_version: 0x80,
            }),
            ChainId::Vertcoin => Some(UtxoParams {
                hrp: "vtc",
                p2pkh_version: &[0x47],
                p2sh_version: &[0x05],
                wif_version: 0x80,
            }),
            // Ravencoin: PUBKEY=60 (R…), SCRIPT=122, SECRET=128 — verified
            // against Ravencoin's src/chainparams.cpp CMainParams.
            ChainId::Ravencoin => Some(UtxoParams {
                hrp: "rvn",
                p2pkh_version: &[60],
                p2sh_version: &[122],
                wif_version: 0x80,
            }),
            // Monacoin: PUBKEY=50 (M…), SCRIPT=55 (P…), SECRET=176 — verified
            // against monacoin's src/chainparams.cpp CMainParams.
            ChainId::Monacoin => Some(UtxoParams {
                hrp: "mona",
                p2pkh_version: &[50],
                p2sh_version: &[55],
                wif_version: 0xb0,
            }),
            // Fujicoin: PUBKEY=36 (F…), SCRIPT=16, bech32 "fc" — verified
            // against trezor coins.json and a live mainnet coinbase payout
            // (FoFS1uDd5CtBPm2wmSWmorfU1eubVJRAdJ). WIF keeps the Bitcoin
            // fork default 0x80.
            ChainId::Fujicoin => Some(UtxoParams {
                hrp: "fc",
                p2pkh_version: &[36],
                p2sh_version: &[16],
                wif_version: 0x80,
            }),
            // Zcash transparent addresses: PUBKEY=7352 (t1…), SCRIPT=7357
            // (t3…) — verified against trezor defs and zcashd chainparams.
            // No bech32: transparent pool is base58 only.
            ChainId::Zcash => Some(UtxoParams {
                hrp: "zc",
                p2pkh_version: &[0x1c, 0xb8], // 7352 → t1…
                p2sh_version: &[0x1c, 0xbd],  // 7357 → t3…
                wif_version: 0x80,
            }),
            // Firo: PUBKEY=82 (a…), SCRIPT=7, SECRET=210 — verified against
            // firoorg/firo src/chainparams.cpp CMainParams.
            ChainId::Firo => Some(UtxoParams {
                hrp: "firo",
                p2pkh_version: &[82],
                p2sh_version: &[7],
                wif_version: 210,
            }),
            // Neurai: PUBKEY=53 (X…), SCRIPT=117, SECRET=128 — verified
            // against neuraiproject/Neurai src/chainparams.cpp CMainParams.
            ChainId::Neurai => Some(UtxoParams {
                hrp: "xna",
                p2pkh_version: &[53],
                p2sh_version: &[117],
                wif_version: 128,
            }),
            // Clore: PUBKEY=23 (c…), SCRIPT=122, SECRET=112 — verified
            // against cloreai-public/clore src/chainparams.cpp CMainParams.
            ChainId::Clore => Some(UtxoParams {
                hrp: "clore",
                p2pkh_version: &[23],
                p2sh_version: &[122],
                wif_version: 112,
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

/// The index API dialect a UTXO chain speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtxoBackend {
    /// mempool.space-style `/address/…` endpoints.
    Esplora,
    /// trezor Blockbook `/api/v2/…` endpoints.
    Blockbook,
    /// Blockcypher v1 API (DOGE/DASH keyless fallback).
    Blockcypher,
    /// Blockchair dashboards (API key recommended).
    Blockchair,
    /// WhatsOnChain REST API (BSV).
    WhatsOnChain,
    /// Chainz cryptoid keyless balance API (Namecoin, Litecoin Cash,
    /// Potcoin, e-Gulden, Vertcoin) — balance only, no UTXO feed.
    Chainz,
    /// No public index API here; only address derivation and explorer links
    /// work, balance queries report a friendly error instead.
    None,
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
    KawPow,
    Equihash,
    RandomX,
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
            HashAlgorithm::KawPow => "KawPow",
            HashAlgorithm::Equihash => "Equihash",
            HashAlgorithm::RandomX => "RandomX",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            HashAlgorithm::Sha256 => "FIPS 180-4 · 32 byte digest",
            HashAlgorithm::Keccak256 => "Ethereum native hash · 32 byte digest",
            HashAlgorithm::Scrypt => "N=16384, r=8, p=1 · 32 byte digest",
            HashAlgorithm::X11 => "Dash PoW 哈希 · 32 byte digest",
            HashAlgorithm::Groestl => "Groestl-512 → 前 32 字节",
            HashAlgorithm::Lyra2re => "Lyra2RE(v2) 哈希链",
            HashAlgorithm::KawPow => "Ravencoin PoW 哈希 · progpow 变体",
            HashAlgorithm::Equihash => "Zcash PoW · 内存困难",
            HashAlgorithm::RandomX => "Monero PoW · CPU 友好",
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

    /// EVM/Ed25519 链的 RPC 备选列表(主源 `rpc` 在前, 按 `#[cfg]` 之外的静态
    /// 顺序依次回退)。全部端点均经主网实测;备用源覆盖主源限速/宕机/被墙场景。
    pub fn rpc_fallbacks(&self) -> &'static [&'static str] {
        match self.id {
            ChainId::EthereumClassic => &["https://etc.etcdesktop.com"],
            ChainId::Solana => &["https://solana-rpc.publicnode.com"],
            ChainId::Ethereum => &[
                "https://ethereum-rpc.publicnode.com",
                "https://cloudflare-eth.com",
            ],
            ChainId::Bsc => &["https://bsc-rpc.publicnode.com"],
            ChainId::Polygon => &["https://polygon-bor-rpc.publicnode.com"],
            _ => &[],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UtxoParams {
    pub hrp: &'static str,
    /// Base58 version prefix bytes (Zcash needs two: 0x1c 0xb8 for t1…).
    pub p2pkh_version: &'static [u8],
    pub p2sh_version: &'static [u8],
    pub wif_version: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_utxo_chain_routes_to_a_backend() {
        for chain in ChainId::ALL {
            if chain.info().kind == ChainKind::Utxo {
                assert_ne!(chain.utxo_backend(), UtxoBackend::None, "{chain:?}");
            }
        }
    }

    #[test]
    fn keyless_balance_sources_cover_all_utxo_chains() {
        // Balance must work out of the box: the primary source is keyless
        // (Esplora/Blockbook/WoC/Chainz) and only the fallback may be
        // Blockchair, where a key merely raises the rate limit.
        for chain in ChainId::ALL {
            if chain.info().kind != ChainKind::Utxo {
                continue;
            }
            assert_ne!(
                chain.utxo_backend(),
                UtxoBackend::Blockchair,
                "{chain:?} should not depend on a Blockchair key"
            );
            if let Some(fallback) = chain.fallback_backend() {
                assert!(
                    fallback == UtxoBackend::Blockchair || fallback == UtxoBackend::Blockcypher,
                    "{chain:?} fallback must be keyless (Blockchair/Blockcypher)"
                );
            }
        }
    }

    #[test]
    fn chainz_slugs_match_the_supported_chains() {
        assert_eq!(ChainId::Namecoin.chainz_coin(), "nmc");
        assert_eq!(ChainId::LitecoinCash.chainz_coin(), "lcc");
        assert_eq!(ChainId::Potcoin.chainz_coin(), "pot");
        assert_eq!(ChainId::EGulden.chainz_coin(), "efl");
        assert_eq!(ChainId::Vertcoin.chainz_coin(), "vtc");
        // Chainz chains have no fallback: cryptoid has no UTXO feed either.
        assert_eq!(ChainId::Potcoin.fallback_backend(), None);
    }

    #[test]
    fn dogecoin_and_cash_use_trezor_blockbook() {
        assert_eq!(ChainId::Dogecoin.utxo_backend(), UtxoBackend::Blockbook);
        assert_eq!(ChainId::Dogecoin.info().rpc, "https://doge.trezor.io");
        assert_eq!(ChainId::BitcoinCash.utxo_backend(), UtxoBackend::Blockbook);
        assert_eq!(ChainId::BitcoinCash.info().rpc, "https://bch.trezor.io");
    }

    #[test]
    fn pow_forks_route_to_live_blockbooks() {
        // Endpoints verified live (balance/UTXO/history answered 200).
        for (chain, base) in [
            (ChainId::Ravencoin, "https://blockbook.ravencoin.org"),
            (ChainId::Monacoin, "https://blockbook.electrum-mona.org"),
            (ChainId::Fujicoin, "https://explorer.fujicoin.org"),
        ] {
            assert_eq!(chain.utxo_backend(), UtxoBackend::Blockbook);
            assert_eq!(chain.info().rpc, base);
        }
    }

    #[test]
    fn ethw_is_a_legacy_gas_evm_fork() {
        // EVM chain id 10001 (0x2711), verified live against the official
        // RPC mainnet.ethereumpow.org.
        assert_eq!(ChainId::Ethw.evm_chain_id(), Some(10001));
        // ETHW kept legacy gas pricing; only ETH/POL use EIP-1559 here.
        assert!(!ChainId::Ethw.supports_eip1559());
        assert_eq!(ChainId::Ethw.info().bip44_coin, 10001);
        assert_eq!(ChainId::Ethw.info().price_id, "ethereum-pow");
    }

    #[test]
    fn pow_fork_address_params_match_chainparams() {
        // Version bytes cross-checked against each coin's chainparams.cpp
        // CMainParams (see comments on utxo_params).
        let rvn = ChainId::Ravencoin.utxo_params().unwrap();
        assert_eq!(
            (rvn.p2pkh_version, rvn.p2sh_version, rvn.wif_version),
            (&[60u8][..], &[122u8][..], 0x80)
        );
        let mona = ChainId::Monacoin.utxo_params().unwrap();
        assert_eq!(
            (mona.p2pkh_version, mona.p2sh_version, mona.wif_version),
            (&[50u8][..], &[55u8][..], 0xb0)
        );
        let fjc = ChainId::Fujicoin.utxo_params().unwrap();
        assert_eq!(
            (fjc.p2pkh_version, fjc.p2sh_version, fjc.wif_version),
            (&[36u8][..], &[16u8][..], 0x80)
        );
        // Ravencoin is the only SegWit-less one of the three.
        assert!(ChainId::Ravencoin.legacy_utxo());
        assert!(!ChainId::Monacoin.legacy_utxo());
        assert!(!ChainId::Fujicoin.legacy_utxo());
        // BIP-44/84 coin types match SLIP-44.
        assert_eq!(ChainId::Ravencoin.info().bip44_coin, 175);
        assert_eq!(ChainId::Monacoin.info().bip44_coin, 22);
        assert_eq!(ChainId::Fujicoin.info().bip44_coin, 75);
    }

    #[test]
    fn two_miners_chains_route_and_parametrize() {
        // Zcash: two-byte transparent prefixes straight out of zcashd.
        let zec = ChainId::Zcash.utxo_params().unwrap();
        assert_eq!(zec.p2pkh_version, &[0x1c, 0xb8]);
        assert_eq!(zec.p2sh_version, &[0x1c, 0xbd]);
        assert_eq!(ChainId::Zcash.utxo_backend(), UtxoBackend::Blockbook);
        assert_eq!(
            ChainId::Zcash.fallback_backend(),
            Some(UtxoBackend::Blockchair)
        );
        assert!(!ChainId::Zcash.legacy_utxo(), "t1… P2PKH signs ZIP-243");
        // Firo / Neurai / Clore: Ravencoin-family, SegWit-less, Blockbook.
        for (chain, p2pkh, p2sh, wif) in [
            (ChainId::Firo, &[82u8][..], &[7u8][..], 210u8),
            (ChainId::Neurai, &[53u8][..], &[117u8][..], 128u8),
            (ChainId::Clore, &[23u8][..], &[122u8][..], 112u8),
        ] {
            let params = chain.utxo_params().unwrap();
            assert_eq!(
                (
                    params.p2pkh_version,
                    params.p2sh_version,
                    params.wif_version
                ),
                (p2pkh, p2sh, wif),
                "{chain:?}"
            );
            assert!(chain.legacy_utxo(), "{chain:?}");
            assert_eq!(chain.utxo_backend(), UtxoBackend::Blockbook, "{chain:?}");
        }
        // OctaSpace is a legacy-gas EVM chain (chain id 800001).
        assert_eq!(ChainId::OctaSpace.evm_chain_id(), Some(800001));
        assert!(!ChainId::OctaSpace.supports_eip1559());
        // Zephyr validates against its own CryptoNote prefix family.
        assert_eq!(
            crate::cryptonote::ZEPHYR_PREFIX_BYTES,
            [0xc0, 0xb1, 0xf4, 0xa0, 0x62]
        );
    }
}
