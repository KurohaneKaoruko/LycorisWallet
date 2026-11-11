//! Air-gapped signing protocol: export an unsigned transaction from the
//! online wallet, carry it to the offline signer (QR / clipboard / file),
//! sign it there with the secret, bring the signed raw transaction back and
//! broadcast from the online wallet.
//!
//! Payloads are self-describing text with a versioned prefix so a human can
//! tell them apart at a glance:
//!
//! ```text
//! LYCOSIGN1:<base64(json)>     unsigned request (online → offline)
//! LYCOSIGNED1:<base64(json)>   signed reply  (offline → online)
//! ```
//!
//! The inner JSON is deliberately small: `format` selects the serializer and
//! every other field is only what the signing function needs. No xpub /
//! watch-only machinery is required — the offline signer holds the secret
//! (mnemonic or raw key), the online wallet never needs to.

use crate::chains::ChainId;
use crate::error::{Error, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};

/// Prefix of an unsigned transaction payload.
pub const UNSIGNED_PREFIX: &str = "LYCOSIGN1:";
/// Prefix of a signed transaction payload.
pub const SIGNED_PREFIX: &str = "LYCOSIGNED1:";

/// What the online wallet asks the offline signer to sign.
///
/// Every variant carries exactly the context its signing routine needs; the
/// offline device never talks to the network.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UnsignedTx {
    /// UTXO native SegWit (P2WPKH inputs). `change_address` is optional so
    /// the offline signer can rebuild the change script from its own pubkey
    /// when omitted.
    P2wpkh {
        chain: ChainId,
        #[serde(default)]
        change_address: Option<String>,
        inputs: Vec<OfflineInput>,
        to: String,
        value: u64,
        fee_rate: u64,
    },
    /// UTXO legacy P2PKH inputs (DOGE/BCH/ZEC/DASH/…).
    P2pkh {
        chain: ChainId,
        #[serde(default)]
        change_address: Option<String>,
        inputs: Vec<OfflineInput>,
        to: String,
        value: u64,
        fee_rate: u64,
    },
    /// EVM native transfer. The signer must use the same nonce/fee — the
    /// online wallet picked them from the live chain state. Wei/gwei values
    /// are decimal **strings** because serde_json refuses u128 by default
    /// and JavaScript numbers lose precision above 2^53.
    Evm {
        chain: ChainId,
        chain_id: u64,
        nonce: u64,
        to: String,
        /// Decimal string of wei.
        value: String,
        gas_limit: u64,
        fee: OfflineEvmFee,
        #[serde(default)]
        data: Option<String>,
        /// The sending address, shown to the user for verification only.
        from: String,
    },
}

/// Base58check address → bech32 is the online wallet's job; the offline
/// signer only needs the input outpoints and amounts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineInput {
    pub txid: String,
    pub vout: u32,
    pub value: u64,
}

/// EVM fee as serialized in the payload — mirror of `crate::tx::evm::EvmFee`
/// minus the borrowing.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OfflineEvmFee {
    Eip1559 {
        /// Decimal string of wei.
        max_fee_per_gas: String,
        /// Decimal string of wei.
        max_priority_fee_per_gas: String,
    },
    Legacy {
        /// Decimal string of wei.
        gas_price: String,
    },
}

impl OfflineEvmFee {
    /// Capture a live fee (online side) into the transferable payload.
    /// Wei/gwei values are decimal strings; see [`UnsignedTx::Evm`].
    pub fn capture(fee: &crate::tx::evm::EvmFee) -> Self {
        match fee {
            crate::tx::evm::EvmFee::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            } => Self::Eip1559 {
                max_fee_per_gas: max_fee_per_gas.to_string(),
                max_priority_fee_per_gas: max_priority_fee_per_gas.to_string(),
            },
            crate::tx::evm::EvmFee::Legacy { gas_price } => Self::Legacy {
                gas_price: gas_price.to_string(),
            },
        }
    }

    /// Parse the decimal-string fields into the numeric fee. Errors carry
    /// the offending field so a mismatched payload is diagnosable offline.
    pub(crate) fn into_evm_fee(self) -> crate::tx::evm::EvmFee {
        let parse = |name: &str, text: &str| -> u128 {
            text.parse::<u128>()
                .unwrap_or_else(|_| panic!("offline payload: {name} 不是有效的数值: {text}"))
        };
        match self {
            Self::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            } => crate::tx::evm::EvmFee::Eip1559 {
                max_fee_per_gas: parse("max_fee_per_gas", &max_fee_per_gas),
                max_priority_fee_per_gas: parse(
                    "max_priority_fee_per_gas",
                    &max_priority_fee_per_gas,
                ),
            },
            Self::Legacy { gas_price } => crate::tx::evm::EvmFee::Legacy {
                gas_price: parse("gas_price", &gas_price),
            },
        }
    }
}

/// The signed reply: raw transaction bytes plus what the online wallet needs
/// to verify-before-broadcast (and to record activity).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedTx {
    pub chain: ChainId,
    /// Ready-to-broadcast hex (0x-prefix for EVM chains).
    pub raw: String,
    /// Transaction id when the signer could compute it (EVM always can).
    #[serde(default)]
    pub txid: Option<String>,
}

impl UnsignedTx {
    pub fn chain(&self) -> ChainId {
        match self {
            Self::P2wpkh { chain, .. }
            | Self::P2pkh {
                inputs: _, chain, ..
            }
            | Self::Evm { chain, .. } => *chain,
        }
    }

    /// Human-readable summary shown on the offline device before signing:
    /// the user must be able to verify recipient and amount at a glance.
    /// Amounts are smallest-unit integers from the payload; a friendly
    /// decimal conversion belongs to the UI which knows the decimals.
    pub fn summary(&self) -> String {
        match self {
            Self::P2wpkh {
                chain,
                inputs,
                value,
                fee_rate,
                ..
            }
            | Self::P2pkh {
                chain,
                inputs,
                value,
                fee_rate,
                ..
            } => format!(
                "{} · {} 输入 · 支付 {} {} · 费率 {} sat/vB",
                chain,
                inputs.len(),
                value,
                chain.base_unit(),
                fee_rate
            ),
            Self::Evm {
                chain, to, value, ..
            } => {
                format!("{chain} · 支付 {value} wei 至 {to}")
            }
        }
    }
}

/// Encode an unsigned request as transferable text.
pub fn encode_unsigned(tx: &UnsignedTx) -> Result<String> {
    let json = serde_json::to_string(tx).map_err(|e| Error::Message(format!("encode: {e}")))?;
    Ok(format!(
        "{UNSIGNED_PREFIX}{}",
        base64::engine::general_purpose::STANDARD.encode(json)
    ))
}

/// Decode and validate an unsigned payload (prefix + base64 + JSON shape).
pub fn decode_unsigned(text: &str) -> Result<UnsignedTx> {
    let text = text.trim();
    let payload = text
        .strip_prefix(UNSIGNED_PREFIX)
        .ok_or_else(|| Error::Message(format!("不是未签名交易（应以 {UNSIGNED_PREFIX} 开头）")))?;
    let json = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|e| Error::Message(format!("payload 不是有效的 base64: {e}")))?;
    serde_json::from_slice(&json).map_err(|e| Error::Message(format!("无法解析交易: {e}")))
}

/// Encode a signed reply as transferable text.
pub fn encode_signed(tx: &SignedTx) -> Result<String> {
    let json = serde_json::to_string(tx).map_err(|e| Error::Message(format!("encode: {e}")))?;
    Ok(format!(
        "{SIGNED_PREFIX}{}",
        base64::engine::general_purpose::STANDARD.encode(json)
    ))
}

/// Decode and validate a signed payload.
pub fn decode_signed(text: &str) -> Result<SignedTx> {
    let text = text.trim();
    let payload = text
        .strip_prefix(SIGNED_PREFIX)
        .ok_or_else(|| Error::Message(format!("不是已签名交易（应以 {SIGNED_PREFIX} 开头）")))?;
    let json = base64::engine::general_purpose::STANDARD
        .decode(payload.trim())
        .map_err(|e| Error::Message(format!("payload 不是有效的 base64: {e}")))?;
    serde_json::from_slice(&json).map_err(|e| Error::Message(format!("无法解析交易: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p2wpkh_roundtrip() {
        let tx = UnsignedTx::P2wpkh {
            chain: ChainId::Bitcoin,
            change_address: Some("bc1qchange".into()),
            inputs: vec![OfflineInput {
                txid: "aa".repeat(32),
                vout: 1,
                value: 90_000,
            }],
            to: "bc1qrecipient".into(),
            value: 50_000,
            fee_rate: 12,
        };
        let text = encode_unsigned(&tx).unwrap();
        assert!(text.starts_with(UNSIGNED_PREFIX));
        let back = decode_unsigned(&text).unwrap();
        match back {
            UnsignedTx::P2wpkh {
                chain,
                inputs,
                to,
                value,
                fee_rate,
                ..
            } => {
                assert_eq!(chain, ChainId::Bitcoin);
                assert_eq!(inputs.len(), 1);
                assert_eq!(inputs[0].value, 90_000);
                assert_eq!(value, 50_000);
                assert_eq!(to, "bc1qrecipient");
                assert_eq!(fee_rate, 12);
            }
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn evm_roundtrip_preserves_big_fees() {
        let tx = UnsignedTx::Evm {
            chain: ChainId::Ethereum,
            chain_id: 1,
            nonce: 7,
            to: "0xabc".into(),
            value: "1000000000000000000".into(),
            gas_limit: 21_000,
            fee: OfflineEvmFee::Eip1559 {
                max_fee_per_gas: "52000000000".into(),
                max_priority_fee_per_gas: "2000000000".into(),
            },
            data: None,
            from: "0xfrom".into(),
        };
        let back = decode_unsigned(&encode_unsigned(&tx).unwrap()).unwrap();
        let UnsignedTx::Evm { fee, value, .. } = &back else {
            panic!("wrong variant");
        };
        assert_eq!(value, "1000000000000000000");
        let OfflineEvmFee::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } = fee
        else {
            panic!("wrong fee variant");
        };
        assert_eq!(max_fee_per_gas, "52000000000");
        assert_eq!(max_priority_fee_per_gas, "2000000000");
        // 数值字段经 into_evm_fee 解析后与原值一致。
        match fee.clone().into_evm_fee() {
            crate::tx::evm::EvmFee::Eip1559 {
                max_fee_per_gas,
                max_priority_fee_per_gas,
            } => {
                assert_eq!(max_fee_per_gas, 52_000_000_000);
                assert_eq!(max_priority_fee_per_gas, 2_000_000_000);
            }
            _ => panic!("wrong fee variant"),
        }
    }

    #[test]
    fn signed_roundtrip() {
        let tx = SignedTx {
            chain: ChainId::Ethereum,
            raw: "0x02f8".into(),
            txid: Some("0xdeadbeef".into()),
        };
        let back = decode_signed(&encode_signed(&tx).unwrap()).unwrap();
        assert_eq!(back.raw, "0x02f8");
        assert_eq!(back.txid.as_deref(), Some("0xdeadbeef"));
    }

    #[test]
    fn rejects_wrong_prefix_and_garbage() {
        let unsigned = UnsignedTx::P2wpkh {
            chain: ChainId::Bitcoin,
            change_address: None,
            inputs: vec![],
            to: "x".into(),
            value: 1,
            fee_rate: 1,
        };
        assert!(decode_unsigned("hello").is_err());
        assert!(decode_signed(&encode_unsigned(&unsigned).unwrap()).is_err());
        assert!(decode_unsigned(&format!("{UNSIGNED_PREFIX}not-base64!!!")).is_err());
    }

    #[test]
    fn p2pkh_roundtrip_with_multi_inputs() {
        let tx = UnsignedTx::P2pkh {
            chain: ChainId::Dogecoin,
            change_address: None,
            inputs: vec![
                OfflineInput {
                    txid: "bb".repeat(32),
                    vout: 0,
                    value: 1_000_000_000,
                },
                OfflineInput {
                    txid: "cc".repeat(32),
                    vout: 2,
                    value: 2_000_000_000,
                },
            ],
            to: "Drecipient".into(),
            value: 1_500_000_000,
            fee_rate: 40,
        };
        let back = decode_unsigned(&encode_unsigned(&tx).unwrap()).unwrap();
        match back {
            UnsignedTx::P2pkh { inputs, .. } => assert_eq!(inputs.len(), 2),
            _ => panic!("wrong variant"),
        }
    }
}
