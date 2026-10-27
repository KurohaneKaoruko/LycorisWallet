//! Standalone utilities exposed through the「工具箱」view.
//!
//! Everything here is a pure function over public data — no secrets, no
//! network, no vault access. The Tauri shell and the WebAssembly demo both
//! call into this module so the two builds stay byte-identical.

use crate::address;
use crate::chains::{ChainId, ChainKind};
use crate::cryptonote::monero_address_validate;
use crate::error::{Error, Result};
use crate::units::{format_units, parse_units};
use base64::Engine;
use serde::{Deserialize, Serialize};

/* ------------------------------------------------------------ address inspector */

/// What the inspector could tell about one address string.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressReport {
    /// Chains whose address grammar + checksum accept this string. EVM
    /// lookalikes list every compatible chain; UTXO keeps cross-network
    /// lookalikes together so the user can spot a wrong-network paste.
    pub matches: Vec<AddressMatch>,
    /// Human notes: why it failed, what looks off, safety advice.
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AddressMatch {
    pub chain: ChainId,
    pub chain_name: String,
    pub symbol: String,
    /// evm / p2pkh / p2wpkh / p2sh / cashaddr / ed25519 / cryptonote.
    pub address_kind: String,
    /// Normalised address (EIP-55 checksummed for EVM).
    pub normalised: String,
    /// EVM only: whether the input casing already matched EIP-55.
    pub checksum_ok: Option<bool>,
}

fn looks_like_evm(text: &str) -> bool {
    let body = text.strip_prefix("0x").unwrap_or(text);
    body.len() == 40 && body.chars().all(|c| c.is_ascii_hexdigit())
}

fn evm_casing_matches(text: &str) -> bool {
    let checksum = address::to_checksum_address(text);
    checksum == text
}

/// Inspect any address string across every supported chain.
pub fn inspect_address(input: &str) -> Result<AddressReport> {
    let text = input.trim();
    let mut matches: Vec<AddressMatch> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    if text.is_empty() {
        return Err(Error::InvalidAddress("address is empty".into()));
    }

    if looks_like_evm(text) {
        if !text.starts_with("0x") {
            notes.push("EVM 地址缺少 0x 前缀，多数钱包仍可识别，但建议补全。".into());
        }
        let casing_ok = evm_casing_matches(text);
        for chain in [
            ChainId::Ethereum,
            ChainId::EthereumClassic,
            ChainId::Bsc,
            ChainId::Polygon,
        ] {
            if address::validate_address(chain, text).is_ok() {
                matches.push(AddressMatch {
                    chain,
                    chain_name: chain.info().name.to_string(),
                    symbol: chain.info().symbol.to_string(),
                    address_kind: "evm".into(),
                    normalised: address::to_checksum_address(text),
                    checksum_ok: Some(casing_ok),
                });
            }
        }
        if !casing_ok {
            notes.push(
                "EIP-55 大小写校验不匹配：全小写/全大写也能用，但混合大小写不符说明可能有抄写错误，请逐字核对。"
                    .into(),
            );
        }
        return Ok(AddressReport { matches, notes });
    }

    // UTXO + Solana + Monero: try every chain, keep the ones that validate.
    for chain in ChainId::ALL {
        if matches!(
            chain,
            ChainId::Ethereum | ChainId::EthereumClassic | ChainId::Bsc | ChainId::Polygon
        ) {
            continue; // handled above
        }
        if let Ok(match_info) = classify_non_evm(chain, text) {
            matches.push(match_info);
        }
    }

    if matches.is_empty() {
        notes.push("没有一条受支持的链接受这个地址。请检查是否复制完整（首尾空格已忽略）。".into());
        if text.len() >= 26
            && text.len() <= 35
            && !text.contains(|c: char| !c.is_ascii_alphanumeric())
            && text.starts_with(|c: char| c.is_ascii_digit())
        {
            notes.push("长度像 base58 的 P2PKH/P2SH 地址，但版本字节不属于受支持的链。".into());
        }
    } else if matches.len() > 1 {
        notes.push(format!(
            "该地址同时通过了 {} 条链的校验（同源地址格式），转账前请确认目标链。",
            matches.len()
        ));
    }

    Ok(AddressReport { matches, notes })
}

fn classify_non_evm(chain: ChainId, text: &str) -> Result<AddressMatch> {
    let info = chain.info();
    let address_kind = match chain {
        ChainId::Solana => {
            address::validate_address(chain, text)?;
            "ed25519"
        }
        ChainId::Monero => {
            if !monero_address_validate(text) {
                return Err(Error::InvalidAddress("not a Monero address".into()));
            }
            if text.starts_with('4') {
                "cryptonote primary"
            } else {
                "cryptonote subaddress"
            }
        }
        ChainId::BitcoinCash => {
            address::validate_address(chain, text)?;
            if text.contains(':') {
                "cashaddr"
            } else {
                "p2pkh (legacy)"
            }
        }
        other => {
            address::validate_address(other, text)?;
            if text.starts_with("bc1") || text.starts_with("ltc1") {
                "p2wpkh (bech32)"
            } else if let Some(params) = other.utxo_params() {
                if let Ok(raw) = address::decode_base58check(text) {
                    if raw.first() == Some(&params.p2sh_version) {
                        "p2sh"
                    } else {
                        "p2pkh (legacy)"
                    }
                } else {
                    "p2pkh (legacy)"
                }
            } else {
                "utxo"
            }
        }
    };
    Ok(AddressMatch {
        chain,
        chain_name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        address_kind: address_kind.to_string(),
        normalised: text.to_string(),
        checksum_ok: None,
    })
}

/* --------------------------------------------------------------- unit converter */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitConversion {
    pub chain: ChainId,
    pub symbol: String,
    pub decimals: u8,
    /// Input restated in the smallest unit (satoshi / wei / lamport …).
    pub smallest: String,
    /// Input restated as a plain decimal string.
    pub decimal: String,
}

/// Convert between display units and the smallest unit of any chain.
pub fn convert_units(chain: ChainId, value: &str, from_smallest: bool) -> Result<UnitConversion> {
    let info = chain.info();
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(Error::InvalidAmount("amount is empty".into()));
    }
    if from_smallest {
        let raw: u128 = trimmed.parse().map_err(|_| {
            Error::InvalidAmount("smallest-unit input must be a non-negative integer".into())
        })?;
        Ok(UnitConversion {
            chain,
            symbol: info.symbol.to_string(),
            decimals: info.decimals,
            smallest: raw.to_string(),
            decimal: format_units(raw, info.decimals),
        })
    } else {
        let raw = parse_units(trimmed, info.decimals)?;
        Ok(UnitConversion {
            chain,
            symbol: info.symbol.to_string(),
            decimals: info.decimals,
            smallest: raw.to_string(),
            decimal: trimmed.to_string(),
        })
    }
}

/* ------------------------------------------------------------ mnemonic checksum */

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MnemonicCheck {
    pub valid: bool,
    pub word_count: usize,
    /// checksum correct / unknown word at index N / wrong length …
    pub verdict: String,
    pub words: Vec<MnemonicWordState>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MnemonicWordState {
    pub word: String,
    pub index: usize,
    pub valid: bool,
}

/// Check BIP-39 words + checksum without deriving anything.
pub fn check_mnemonic(input: &str) -> Result<MnemonicCheck> {
    use bip39::Mnemonic;

    let normalised: String = input.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalised.is_empty() {
        return Err(Error::InvalidMnemonic("mnemonic is empty".into()));
    }
    let words: Vec<&str> = normalised.split_whitespace().collect();
    let language = bip39::Language::English;

    let mut states = Vec::with_capacity(words.len());
    let mut unknown = false;
    for (index, word) in words.iter().enumerate() {
        let valid = language.find_word(word).is_some();
        if !valid {
            unknown = true;
        }
        states.push(MnemonicWordState {
            word: (*word).to_string(),
            index,
            valid,
        });
    }

    let checksum_ok = Mnemonic::parse_normalized(&normalised).is_ok();
    let valid = !unknown && checksum_ok;
    let verdict = if unknown {
        "存在不在 BIP-39 词表里的单词（已标红），请先修正拼写。".to_string()
    } else if !matches!(words.len(), 12 | 15 | 18 | 21 | 24) {
        format!(
            "{} 个单词：BIP-39 只接受 12 / 15 / 18 / 21 / 24 词。",
            words.len()
        )
    } else if checksum_ok {
        "校验和正确，这是一组有效的助记词。".to_string()
    } else {
        "词表匹配，但校验和不对——请逐词核对，常见错是相近词（road/rock）。".to_string()
    };

    Ok(MnemonicCheck {
        valid,
        word_count: words.len(),
        verdict,
        words: states,
    })
}

/* ------------------------------------------------------------------- hashing kit */

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DigestOutput {
    pub algorithm: String,
    pub hex: String,
    pub base64: String,
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hash an arbitrary UTF-8 string with the algorithm of your choice. The
/// preimage is exactly the UTF-8 bytes of the input (no trailing newline).
pub fn hash_string(algorithm: &str, input: &str) -> Result<DigestOutput> {
    let bytes = input.as_bytes();
    let (name, digest): (&str, Vec<u8>) = match algorithm {
        "sha256" => ("SHA-256", address::sha256(bytes).to_vec()),
        "sha512" => {
            use sha2::Digest;
            ("SHA-512", sha2::Sha512::digest(bytes).to_vec())
        }
        "double-sha256" => ("SHA-256d", address::double_sha256(bytes).to_vec()),
        "keccak256" => ("Keccak-256", address::keccak256(bytes).to_vec()),
        "hash160" => ("HASH160", address::hash160(bytes).to_vec()),
        "ripemd160" => {
            use ripemd::Digest;
            ("RIPEMD-160", ripemd::Ripemd160::digest(bytes).to_vec())
        }
        _ => {
            return Err(Error::Message(format!(
                "unsupported algorithm: {algorithm}"
            )))
        }
    };
    Ok(DigestOutput {
        algorithm: name.to_string(),
        hex: hex_encode(&digest),
        base64: base64::engine::general_purpose::STANDARD.encode(&digest),
    })
}

/* ---------------------------------------------------------------- encoding kit */

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EncodingOutput {
    pub hex: String,
    pub base64: String,
    pub base58: String,
}

fn decode_input(input: &str) -> Result<Vec<u8>> {
    let text = input.trim();
    if text.is_empty() {
        return Err(Error::Message("input is empty".into()));
    }
    if let Some(hex_body) = text.strip_prefix("0x") {
        return hex::decode(hex_body).map_err(|e| Error::Message(format!("hex decode: {e}")));
    }
    if text.len().is_multiple_of(2) && text.chars().all(|c| c.is_ascii_hexdigit()) {
        return hex::decode(text).map_err(|e| Error::Message(format!("hex decode: {e}")));
    }
    Ok(text.as_bytes().to_vec())
}

/// Encode / decode a payload through hex, base64 and base58 side by side.
pub fn transform_encoding(input: &str) -> Result<EncodingOutput> {
    let bytes = decode_input(input)?;
    Ok(EncodingOutput {
        hex: hex_encode(&bytes),
        base64: base64::engine::general_purpose::STANDARD.encode(&bytes),
        base58: bs58::encode(&bytes).into_string(),
    })
}

/* ------------------------------------------------------------------- payment URI */

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedUri {
    pub chain: ChainId,
    pub chain_name: String,
    pub symbol: String,
    pub address: String,
    /// Display-unit amount (SOL/ETH/BTC…), already converted from the URI's
    /// native denomination (EIP-681 `value` is wei).
    pub amount: Option<String>,
    pub label: Option<String>,
    pub message: Option<String>,
    pub memo: Option<String>,
    pub request_id: Option<String>,
    pub warnings: Vec<String>,
}

fn percent_decode(value: &str) -> String {
    let mut out = Vec::with_capacity(value.len());
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = (bytes[i + 1] as char).to_digit(16);
            let lo = (bytes[i + 2] as char).to_digit(16);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_query_fragments(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            (percent_decode(key), percent_decode(value))
        })
        .collect()
}

fn chain_from_scheme(scheme: &str) -> Option<ChainId> {
    match scheme {
        "bitcoin" => Some(ChainId::Bitcoin),
        "litecoin" => Some(ChainId::Litecoin),
        "dogecoin" => Some(ChainId::Dogecoin),
        "bitcoincash" => Some(ChainId::BitcoinCash),
        "ethereum" => Some(ChainId::Ethereum),
        "bsc" => Some(ChainId::Bsc),
        "polygon" => Some(ChainId::Polygon),
        "solana" => Some(ChainId::Solana),
        _ => None,
    }
}

/// Parse a BIP-21 (BTC/LTC/DOGE/BCH), EIP-681 (EVM) or Solana Pay URI.
pub fn parse_payment_uri(input: &str) -> Result<ParsedUri> {
    let text = input.trim();
    if text.is_empty() {
        return Err(Error::Message("URI 是空的。".into()));
    }
    let (scheme, rest) = text
        .split_once(':')
        .ok_or_else(|| Error::Message("不是 payment URI：缺少 scheme（例如 bitcoin:）。".into()))?;
    let scheme = scheme.to_ascii_lowercase();

    let chain = chain_from_scheme(&scheme).ok_or_else(|| {
        Error::Message(format!(
            "不支持的 scheme：{scheme}（支持 bitcoin/litecoin/dogecoin/bitcoincash/ethereum/bsc/polygon/solana）。"
        ))
    })?;

    let mut warnings = Vec::new();
    let (path_part, query) = rest.split_once('?').unwrap_or((rest, ""));
    let params = parse_query_fragments(query);
    let get = |key: &str| {
        params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.clone())
    };
    let info = chain.info();

    match chain {
        ChainId::Solana => {
            // solana:⟨address⟩?amount=…&label=…&message=…&spl-token=…
            let address = percent_decode(path_part);
            address::validate_address(chain, &address)?;
            if get("spl-token").is_some() {
                warnings.push("URI 带有 spl-token 参数：目标可能是 SPL 代币而非 SOL。".into());
            }
            Ok(ParsedUri {
                chain,
                chain_name: info.name.to_string(),
                symbol: info.symbol.to_string(),
                address,
                amount: get("amount"),
                label: get("label"),
                message: get("message"),
                memo: None,
                request_id: None,
                warnings,
            })
        }
        ChainId::Ethereum | ChainId::EthereumClassic | ChainId::Bsc | ChainId::Polygon => {
            // EIP-681: ethereum:0x…[@chain_id][/transfer?address=…&uint256=…]
            let (address_part, tail) = match path_part.split_once('/') {
                Some((addr, tail)) => (addr, Some(tail)),
                None => (path_part, None),
            };
            let address = percent_decode(address_part);
            if !address.is_empty() {
                address::validate_address(chain, &address)?;
            }
            if tail.is_some() || get("function").is_some() {
                warnings.push(
                    "该 URI 疑似 ERC-20 转移（带 /transfer 或 function 参数），请核对目标合约。"
                        .into(),
                );
            }
            // value is wei — convert to display units.
            let amount = get("value")
                .and_then(|wei| {
                    let wei = wei.trim();
                    if wei.is_empty() {
                        None
                    } else {
                        Some(format_units_opt(wei, info.decimals))
                    }
                })
                .flatten();
            Ok(ParsedUri {
                chain,
                chain_name: info.name.to_string(),
                symbol: info.symbol.to_string(),
                address,
                amount,
                label: get("label"),
                message: get("message"),
                memo: None,
                request_id: None,
                warnings,
            })
        }
        // BIP-21 family
        _ => {
            let address = percent_decode(path_part);
            address::validate_address(chain, &address)?;
            Ok(ParsedUri {
                chain,
                chain_name: info.name.to_string(),
                symbol: info.symbol.to_string(),
                address,
                amount: get("amount"),
                label: get("label"),
                message: get("message"),
                memo: None,
                request_id: None,
                warnings,
            })
        }
    }
}

/// Expand EIP-681 `value` which may use scientific notation (`1e18`) into an
/// integer amount of the smallest unit. Returns `None` for anything that is
/// not exactly representable.
fn expand_scientific(value: &str) -> Option<u128> {
    let (mantissa, exponent) = value.split_once(['e', 'E'])?;
    let exponent: i32 = exponent
        .strip_prefix('+')
        .unwrap_or(exponent)
        .parse()
        .ok()?;
    let (int_part, frac_part) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if int_part.is_empty() && frac_part.is_empty() {
        return None;
    }
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return None;
    }
    let digits: String = format!("{int_part}{frac_part}");
    let number: u128 = digits.parse().ok()?;
    let shift = exponent - frac_part.len() as i32;
    if shift < 0 {
        return None; // fractional wei — not representable
    }
    number.checked_mul(10u128.checked_pow(shift as u32)?)
}

fn format_units_opt(wei: &str, decimals: u8) -> Option<String> {
    if let Ok(raw) = wei.parse::<u128>() {
        return Some(format_units(raw, decimals));
    }
    expand_scientific(wei).map(|raw| format_units(raw, decimals))
}

/* -------------------------------------------------------------- URI composition */

fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.as_bytes() {
        match byte {
            b'0'..=b'9' | b'A'..=b'Z' | b'a'..=b'z' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Build a BIP-21 / EIP-681 / Solana Pay style URI from parts.
pub fn compose_payment_uri(
    chain: ChainId,
    address: &str,
    amount: Option<&str>,
    label: Option<&str>,
    message: Option<&str>,
) -> Result<String> {
    let info = chain.info();
    address::validate_address(chain, address)?;

    if let Some(amount) = amount.filter(|a| !a.trim().is_empty()) {
        parse_units(amount.trim(), info.decimals)?;
    }

    let scheme = match chain {
        ChainId::Bitcoin => "bitcoin",
        ChainId::Litecoin => "litecoin",
        ChainId::Dogecoin => "dogecoin",
        ChainId::BitcoinCash => "bitcoincash",
        ChainId::Ethereum | ChainId::EthereumClassic | ChainId::Ethw => "ethereum",
        ChainId::Bsc => "bsc",
        ChainId::Polygon => "polygon",
        ChainId::Solana => "solana",
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
        ChainId::Monero => {
            return Err(Error::Message(
                "Monero 的 payment URI 由 monero-wallet-rpc 生成，此处不构造。".into(),
            ))
        }
    };

    let mut uri = format!("{scheme}:{address}");
    let mut query: Vec<String> = Vec::new();
    if let Some(amount) = amount.filter(|a| !a.trim().is_empty()) {
        match chain.info().kind {
            ChainKind::Evm => {
                // EIP-681 wants wei as an integer.
                let wei = parse_units(amount.trim(), info.decimals)?;
                query.push(format!("value={wei}"));
            }
            _ => query.push(format!("amount={}", amount.trim())),
        }
    }
    if let Some(label) = label.filter(|l| !l.trim().is_empty()) {
        query.push(format!("label={}", percent_encode(label.trim())));
    }
    if let Some(message) = message.filter(|m| !m.trim().is_empty()) {
        query.push(format!("message={}", percent_encode(message.trim())));
    }
    if !query.is_empty() {
        uri.push('?');
        uri.push_str(&query.join("&"));
    }
    Ok(uri)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BTC_P2WPKH: &str = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";

    #[test]
    fn inspect_bech32_bitcoin() {
        let report = inspect_address(BTC_P2WPKH).unwrap();
        assert!(report.matches.iter().any(|m| m.chain == ChainId::Bitcoin));
    }

    #[test]
    fn inspect_rejects_garbage() {
        assert!(inspect_address("not-an-address")
            .unwrap()
            .matches
            .is_empty());
    }

    #[test]
    fn convert_btc_units() {
        let out = convert_units(ChainId::Bitcoin, "0.5", false).unwrap();
        assert_eq!(out.smallest, "50000000");
        assert_eq!(out.decimal, "0.5");
        let back = convert_units(ChainId::Bitcoin, "50000000", true).unwrap();
        assert_eq!(back.decimal, "0.5");
    }

    #[test]
    fn mnemonic_verdicts() {
        let good = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let check = check_mnemonic(good).unwrap();
        assert!(check.valid);
        let bad = check_mnemonic("abandon abandon abandon").unwrap();
        assert!(!bad.valid);
        let unknown_word = check_mnemonic("zzz abandon abandon").unwrap();
        assert!(unknown_word.words[0].word == "zzz" && !unknown_word.words[0].valid);
    }

    #[test]
    fn hash_kit_is_stable() {
        let out = hash_string("sha256", "abc").unwrap();
        assert_eq!(
            out.hex,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(out.base64, "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=");
    }

    #[test]
    fn encoding_round_trip() {
        let out = transform_encoding("0x0102ff").unwrap();
        assert_eq!(out.hex, "0102ff");
        assert_eq!(out.base58, "LiA");
        assert_eq!(out.base64, "AQL/");
    }

    #[test]
    fn parse_bip21() {
        let uri =
            parse_payment_uri(&format!("bitcoin:{BTC_P2WPKH}?amount=0.001&label=Test")).unwrap();
        assert_eq!(uri.chain, ChainId::Bitcoin);
        assert_eq!(uri.amount.as_deref(), Some("0.001"));
        assert_eq!(uri.label.as_deref(), Some("Test"));
    }

    #[test]
    fn parse_eip681() {
        let uri =
            parse_payment_uri("ethereum:0x314159265dD8dbb310642f98f50C066173C1259b?value=1e18")
                .unwrap();
        assert_eq!(uri.chain, ChainId::Ethereum);
        assert_eq!(uri.amount.as_deref(), Some("1"));
    }

    #[test]
    fn compose_then_parse_bip21() {
        let uri = compose_payment_uri(
            ChainId::Bitcoin,
            BTC_P2WPKH,
            Some("0.5"),
            Some("彼岸花"),
            None,
        )
        .unwrap();
        assert_eq!(
            uri,
            format!("bitcoin:{BTC_P2WPKH}?amount=0.5&label=%E5%BD%BC%E5%B2%B8%E8%8A%B1")
        );
        let parsed = parse_payment_uri(&uri).unwrap();
        assert_eq!(parsed.amount.as_deref(), Some("0.5"));
        assert_eq!(parsed.label.as_deref(), Some("彼岸花"));
    }

    #[test]
    fn compose_solana_pay() {
        let addr = "EVYJ4gTmTLNwr4FbrK7PdsdFTFQCQW9QL9tqSF6CWUtj";
        let uri = compose_payment_uri(ChainId::Solana, addr, Some("1.5"), None, None).unwrap();
        assert_eq!(uri, format!("solana:{addr}?amount=1.5"));
        assert!(parse_payment_uri(&uri).is_ok());
    }
}
