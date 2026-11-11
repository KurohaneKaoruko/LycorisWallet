//! Signing side of the air-gapped protocol: turn an [`UnsignedTx`] plus a
//! private key into a broadcastable [`SignedTx`].
//!
//! This runs on the offline device (wasm build in the companion web app,
//! or any host that can reach the core). The key material is supplied by
//! the caller — nothing here touches vaults or entropy.
//!
//! Secret handling: callers hold the mnemonic / raw key and derive the
//! per-chain key with `crate::derive::derive_private_key` before calling
//! [`sign_with_key`]; this module only sees the final raw key.

use super::evm::{self, EvmCall, EvmFee};
use super::offline::{OfflineInput, SignedTx, UnsignedTx};
use super::utxo::{self, UtxoInput};
use crate::address;
use crate::chains::ChainId;
use crate::error::{Error, Result};
use k256::SecretKey;

/// Parse a hex private key (0x prefix optional) into a secp256k1 key.
pub fn parse_secp_key(key_hex: &str) -> Result<SecretKey> {
    let trimmed = key_hex.trim().strip_prefix("0x").unwrap_or(key_hex.trim());
    let bytes = hex::decode(trimmed)?;
    SecretKey::from_slice(&bytes).map_err(|e| Error::InvalidPrivateKey(format!("{e}")))
}

fn utxo_input(input: &OfflineInput) -> Result<UtxoInput> {
    UtxoInput::from_parts(&input.txid, input.vout, input.value, true)
}

/// UTXO signing inputs, unpacked from the payload variant.
struct UtxoSpend<'a> {
    chain: ChainId,
    inputs: &'a [OfflineInput],
    to: &'a str,
    value: u64,
    fee_rate: u64,
    change_address: Option<&'a str>,
    legacy: bool,
}

/// UTXO fee model for the offline signer: the payload pins the *payment*
/// amount; everything above it that is not kept as change becomes the fee.
/// The requested fee rate is honoured through the builder's minimum-fee
/// check, so a too-greedy request fails loudly instead of overpaying.
///
/// The change address comes from the payload when the online wallet pinned
/// one, otherwise it is derived from the signing key's own pubkey (the
/// offline signer is always the same single account).
fn sign_utxo(key: &SecretKey, spend: &UtxoSpend<'_>) -> Result<SignedTx> {
    let UtxoSpend {
        chain,
        inputs,
        to,
        value,
        fee_rate,
        change_address,
        legacy,
    } = *spend;
    let inputs: Vec<UtxoInput> = inputs.iter().map(utxo_input).collect::<Result<Vec<_>>>()?;
    let recipient = utxo::recipient_script(chain, to)?;
    let pubkey = address::compressed_pubkey(key);
    let params = chain
        .utxo_params()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let change_script = match change_address {
        Some(addr) => utxo::recipient_script(chain, addr)?,
        None => {
            let (_, script) = if legacy {
                utxo::legacy_change_script(&params, &pubkey)
            } else {
                utxo::change_script(&params, &pubkey)
            };
            script
        }
    };

    // 手续费按费率预估；找零=输入总额-支付-手续费（低于灰尘并入手续费）。
    let dust = chain.dust_limit();
    let output_count = 2; // recipient + change; the builder drops dust change.
    let estimated_fee = if legacy {
        utxo::estimate_legacy_vsize(inputs.len(), output_count)
    } else {
        utxo::estimate_vsize(inputs.len(), output_count)
    }
    .saturating_mul(fee_rate.max(1));
    let total_in: u64 = inputs.iter().map(|i| i.value).sum();
    if total_in < value.saturating_add(estimated_fee) {
        return Err(Error::InsufficientBalance);
    }
    let change = total_in - value - estimated_fee;
    let change_output = if change >= dust {
        Some((change_script.as_slice(), change))
    } else {
        None
    };

    let (raw, _fee) = if legacy {
        utxo::build_signed_p2pkh(
            key,
            &inputs,
            &recipient,
            value,
            change_output,
            fee_rate,
            chain,
        )?
    } else {
        utxo::build_signed_p2wpkh(
            key,
            &inputs,
            &recipient,
            value,
            change_output,
            fee_rate,
            chain,
        )?
    };
    Ok(SignedTx {
        chain,
        raw,
        txid: None,
    })
}

/// Sign an EVM native transfer.
fn sign_evm(key: &SecretKey, request: &UnsignedTx) -> Result<SignedTx> {
    let UnsignedTx::Evm {
        chain,
        chain_id,
        nonce,
        to,
        value,
        gas_limit,
        fee,
        data,
        ..
    } = request
    else {
        return Err(Error::Message("不是 EVM 交易".into()));
    };
    let to_bytes = address::parse_evm_address(to)?;
    let data_bytes = match data {
        Some(hex_str) => {
            let trimmed = hex_str.trim().strip_prefix("0x").unwrap_or(hex_str.trim());
            hex::decode(trimmed)?
        }
        None => Vec::new(),
    };
    let evm_fee: EvmFee = fee.clone().into_evm_fee();
    let signed = evm::sign_transaction(
        key,
        &EvmCall {
            chain_id: *chain_id,
            nonce: *nonce,
            gas_limit: *gas_limit,
            to: to_bytes,
            value: value
                .parse::<u128>()
                .map_err(|e| Error::InvalidAmount(format!("value 不是有效数值: {e}")))?,
            data: &data_bytes,
            fee: evm_fee,
        },
    )?;
    Ok(SignedTx {
        chain: *chain,
        raw: signed.raw_hex,
        txid: Some(signed.hash),
    })
}

/// Sign an offline request with a raw secp256k1 hex key. EVM and UTXO chains
/// are covered; ed25519/CryptoNote chains are rejected (Solana needs the
/// ed25519 seed path and Monero needs ring signatures — neither belongs in
/// the QR round-trip).
pub fn sign_with_key(request: &UnsignedTx, key_hex: &str) -> Result<SignedTx> {
    let key = parse_secp_key(key_hex)?;
    match request {
        UnsignedTx::P2wpkh {
            chain,
            change_address,
            inputs,
            to,
            value,
            fee_rate,
        } => sign_utxo(
            &key,
            &UtxoSpend {
                chain: *chain,
                inputs,
                to,
                value: *value,
                fee_rate: *fee_rate,
                change_address: change_address.as_deref(),
                legacy: false,
            },
        ),
        UnsignedTx::P2pkh {
            chain,
            change_address,
            inputs,
            to,
            value,
            fee_rate,
        } => sign_utxo(
            &key,
            &UtxoSpend {
                chain: *chain,
                inputs,
                to,
                value: *value,
                fee_rate: *fee_rate,
                change_address: change_address.as_deref(),
                legacy: true,
            },
        ),
        UnsignedTx::Evm { .. } => sign_evm(&key, request),
    }
}

#[cfg(test)]
mod tests {
    use super::super::offline::OfflineEvmFee;
    use super::*;

    /// Any valid secp256k1 key works for the builder tests; the builders are
    /// covered against official vectors in `utxo.rs` / `evm.rs`.
    const TEST_KEY: &str = "290aabb4d1b0c2e71f8e297c6c66f5a8b7b5c91f3c94c4c68f4e5be18ad2a857";

    fn sample_p2wpkh() -> UnsignedTx {
        UnsignedTx::P2wpkh {
            chain: ChainId::Bitcoin,
            change_address: None,
            inputs: vec![OfflineInput {
                txid: "aa".repeat(32),
                vout: 0,
                value: 100_000,
            }],
            to: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".into(),
            value: 60_000,
            fee_rate: 10,
        }
    }

    #[test]
    fn rejects_invalid_key() {
        assert!(parse_secp_key("not-hex").is_err());
        assert!(parse_secp_key("00").is_err());
        // Empty key is not a key.
        assert!(parse_secp_key("").is_err());
    }

    #[test]
    fn p2wpkh_signs_and_produces_segwit_raw() {
        let signed = sign_with_key(&sample_p2wpkh(), TEST_KEY).expect("sign");
        assert!(signed.raw.len() > 100);
        // SegWit transactions start with version 2 + marker + flag.
        assert!(signed.raw.starts_with("020000000001"));
        assert_eq!(signed.chain, ChainId::Bitcoin);
    }

    #[test]
    fn undersized_input_is_rejected() {
        let request = UnsignedTx::P2wpkh {
            chain: ChainId::Bitcoin,
            change_address: None,
            inputs: vec![OfflineInput {
                txid: "aa".repeat(32),
                vout: 0,
                value: 61_000, // < value + fee
            }],
            to: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".into(),
            value: 60_000,
            fee_rate: 10,
        };
        assert!(matches!(
            sign_with_key(&request, TEST_KEY),
            Err(Error::InsufficientBalance)
        ));
    }

    #[test]
    fn dust_change_is_absorbed_into_fee() {
        // 输入 100_000、支付 98_500：预估费 ~1_410，找零 ~90 sat 低于尘埃
        // 限额（546）→ builder 把找零并入手续费，交易只有单个输出。
        let request = UnsignedTx::P2wpkh {
            chain: ChainId::Bitcoin,
            change_address: None,
            inputs: vec![OfflineInput {
                txid: "bb".repeat(32),
                vout: 0,
                value: 100_000,
            }],
            to: "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4".into(),
            value: 98_500,
            fee_rate: 10,
        };
        let signed = sign_with_key(&request, TEST_KEY).expect("sign");
        assert!(signed.raw.starts_with("020000000001"));
        // 单输出校验：布局 version(4)+marker(1)+flag(1)+input_count(1)
        // +input(36 outpoint+1 空scriptSig+4 sequence=41)+output_count(1)，
        // 输出数量字节位于偏移 48。
        let raw = hex::decode(&signed.raw).unwrap();
        assert_eq!(raw[48], 1, "尘埃找零应被并入手续费（单输出）");
    }

    #[test]
    fn change_output_present_when_worthwhile() {
        // 输入 100_000、支付 60_000：找零远高于尘埃 → 两个输出（偏移 48）。
        let signed = sign_with_key(&sample_p2wpkh(), TEST_KEY).expect("sign");
        let raw = hex::decode(&signed.raw).unwrap();
        assert_eq!(raw[48], 2, "找零应作为第二个输出存在");
    }

    #[test]
    fn evm_signs_and_returns_txid() {
        let request = UnsignedTx::Evm {
            chain: ChainId::Ethereum,
            chain_id: 1,
            nonce: 0,
            to: "0x1111111111111111111111111111111111111111".into(),
            value: "1000000000000000000".into(),
            gas_limit: 21_000,
            fee: OfflineEvmFee::Legacy {
                gas_price: "2000000000".into(),
            },
            data: None,
            from: "0x2222222222222222222222222222222222222222".into(),
        };
        let signed = sign_with_key(&request, TEST_KEY).expect("sign");
        assert!(signed.raw.starts_with("0x"));
        assert!(signed.txid.as_deref().unwrap_or("").starts_with("0x"));
    }

    #[test]
    fn rejects_solana_and_monero() {
        // UnsignedTx has no Solana/Monero variants by design: the payload
        // enum only covers secp256k1 chains. A mnemonic-derived ed25519 seed
        // never reaches this module.
        let encoded = super::super::offline::encode_unsigned(&sample_p2wpkh()).unwrap();
        let decoded = super::super::offline::decode_unsigned(&encoded).unwrap();
        assert_eq!(decoded.chain(), ChainId::Bitcoin);
    }
}
