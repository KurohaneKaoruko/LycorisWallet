//! Lycoris Offline Signer — air-gapped 交易签名器（Android 应用壳）。
//!
//! 三个 Tauri 命令，全部纯本地计算、零网络：
//! - `signer_prepare`: 载荷解析 + 摘要 + 按本机密钥派生的收款地址（供核对）
//! - `signer_sign`:    用派生私钥签名，返回 LYCOSIGNED1: 文本
//! - `signer_validate`: 独立校验一个签名载荷
//!
//! 私钥只存在于单次命令调用的栈上，命令返回即 drop，不落盘、不进全局状态。

use lycoris_core::chains::ChainId;
use lycoris_core::derive::{derive_accounts, derive_private_key, WalletSecret};
use lycoris_core::tx::offline::{decode_signed, decode_unsigned, encode_signed, UnsignedTx};
use lycoris_core::tx::sign::sign_with_key;
use serde::Serialize;
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PrepareResult {
    /// 人读摘要：链 / 支付金额 / 收款方 / 输入数 / 费率 / nonce。
    summary: Vec<SummaryLine>,
    /// 用本机密钥派生的该链地址。EVM 载荷里的 from 应与它一致——
    /// 不一致说明载荷不是发给这台设备的，签名即花钱，必须醒目警告。
    from_address: Option<String>,
    from_matches: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SummaryLine {
    label: String,
    value: String,
    /// 金额行/地址行在前端加粗显示。
    emphasis: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignResult {
    signed: String,
}

/// 该链地址：走与主钱包完全一致的派生（`derive_accounts` 内部就是
/// BIP-32/SLIP-10 标准路径）。签名器与主钱包同一把核心，地址必然一致。
fn address_for(secret: &WalletSecret, chain: ChainId) -> Result<String, String> {
    derive_accounts(secret)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|account| account.chain == chain)
        .map(|account| account.address)
        .ok_or_else(|| format!("该密钥不包含 {} 账户", chain.info().name))
}

/// 载荷 → 结构化摘要行。
fn summarize(tx: &UnsignedTx) -> Vec<SummaryLine> {
    let info = tx.chain().info();
    let decimals = info.decimals;
    let mut lines = Vec::new();
    lines.push(SummaryLine {
        label: "链".into(),
        value: info.name.to_string(),
        emphasis: false,
    });
    match tx {
        UnsignedTx::P2wpkh {
            inputs,
            to,
            value,
            fee_rate,
            ..
        }
        | UnsignedTx::P2pkh {
            inputs,
            to,
            value,
            fee_rate,
            ..
        } => {
            let units = *value as f64 / 10f64.powi(decimals as i32);
            lines.push(SummaryLine {
                label: "支付".into(),
                value: format!("{} {}", units, info.symbol),
                emphasis: true,
            });
            lines.push(SummaryLine {
                label: "收款方".into(),
                value: to.clone(),
                emphasis: true,
            });
            lines.push(SummaryLine {
                label: "输入数".into(),
                value: inputs.len().to_string(),
                emphasis: false,
            });
            lines.push(SummaryLine {
                label: "费率".into(),
                value: format!("{fee_rate} sat/vB"),
                emphasis: false,
            });
        }
        UnsignedTx::Evm {
            chain_id,
            nonce,
            to,
            value,
            gas_limit,
            ..
        } => {
            let wei = value.parse::<u128>().unwrap_or(0);
            let units = wei as f64 / 10f64.powi(decimals as i32);
            lines.push(SummaryLine {
                label: "支付".into(),
                value: format!("{} {}", units, info.symbol),
                emphasis: true,
            });
            lines.push(SummaryLine {
                label: "收款方".into(),
                value: to.clone(),
                emphasis: true,
            });
            lines.push(SummaryLine {
                label: "链 ID".into(),
                value: chain_id.to_string(),
                emphasis: false,
            });
            lines.push(SummaryLine {
                label: "Nonce".into(),
                value: nonce.to_string(),
                emphasis: false,
            });
            lines.push(SummaryLine {
                label: "Gas".into(),
                value: gas_limit.to_string(),
                emphasis: false,
            });
        }
    }
    lines
}

#[tauri::command]
fn signer_prepare(payload: String, secret: WalletSecret) -> Result<PrepareResult, String> {
    let tx = decode_unsigned(&payload).map_err(|e| e.to_string())?;
    let from_address = address_for(&secret, tx.chain()).ok();
    let from_matches = match (&tx, &from_address) {
        // EVM 载荷带 from 字段，必须与派生地址一致。
        (UnsignedTx::Evm { from, .. }, Some(ours)) => from.eq_ignore_ascii_case(ours),
        // UTXO 载荷不带 from：找零地址由签名侧按同一把钥匙派生，天然一致。
        (UnsignedTx::P2wpkh { .. } | UnsignedTx::P2pkh { .. }, _) => true,
        // 本机密钥没有这条链的账户 → 摘要里地址为空，前端警告。
        (_, None) => false,
    };
    Ok(PrepareResult {
        summary: summarize(&tx),
        from_address,
        from_matches,
    })
}

#[tauri::command]
fn signer_sign(payload: String, secret: WalletSecret) -> Result<SignResult, String> {
    let tx = decode_unsigned(&payload).map_err(|e| e.to_string())?;
    // 私钥：与主钱包同一条派生路径（助记词→BIP-32 / 私钥环→直连），
    // 保证签名账户与构造时的 from 地址是同一把钥匙。
    let material = derive_private_key(&secret, tx.chain()).map_err(|e| e.to_string())?;
    let key = material
        .secp()
        .map_err(|_| "该账户不是 secp256k1 链（Solana/Monero 暂不支持离线签名）".to_string())?;
    let key_hex = hex::encode(key.to_bytes());
    let signed = sign_with_key(&tx, &key_hex).map_err(|e| e.to_string())?;
    let text = encode_signed(&signed).map_err(|e| e.to_string())?;
    Ok(SignResult { signed: text })
}

#[tauri::command]
fn signer_validate(signed: String) -> Result<bool, String> {
    decode_signed(&signed)
        .map(|_| true)
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            signer_prepare,
            signer_sign,
            signer_validate,
        ])
        .run(tauri::generate_context!())
        .expect("error while running lycoris signer");
}

#[cfg(test)]
mod tests {
    use super::*;
    use lycoris_core::derive::KeyRing;
    use lycoris_core::tx::offline::{encode_unsigned, OfflineInput};

    /// 与 crates/core/src/tx/sign.rs 同一把测试私钥（BIP-32 向量派生）。
    const TEST_KEY: &str = "290aabb4d1b0c2e71f8e297c6c66f5a8b7b5c91f3c94c4c68f4e5be18ad2a857";

    fn keyring_secret() -> WalletSecret {
        WalletSecret::PrivateKey {
            keys: KeyRing {
                secp256k1: Some(TEST_KEY.into()),
                ed25519: None,
                monero: None,
            },
        }
    }

    fn sample_payload() -> String {
        encode_unsigned(&UnsignedTx::P2wpkh {
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
        })
        .unwrap()
    }

    #[test]
    fn prepare_and_sign_roundtrip() {
        let secret = keyring_secret();
        let prepared = signer_prepare(sample_payload(), secret.clone()).unwrap();
        assert!(prepared.from_matches);
        assert!(prepared.from_address.is_some());
        assert!(prepared.summary.iter().any(|line| line.emphasis));

        let signed = signer_sign(sample_payload(), secret).unwrap();
        assert!(signed.signed.starts_with("LYCOSIGNED1:"));
        assert!(signer_validate(signed.signed).unwrap());
    }

    #[test]
    fn rejects_garbage_payload() {
        let secret = keyring_secret();
        assert!(signer_prepare("garbage".into(), secret.clone()).is_err());
        assert!(signer_sign("LYCOSIGN1:!!!not-base64".into(), secret).is_err());
    }
}
