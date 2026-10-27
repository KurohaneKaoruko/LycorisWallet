use crate::chains::ChainId;
use crate::error::{Error, Result};
use bech32::{segwit, Hrp};
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;
use ripemd::Ripemd160;
use sha2::{Digest, Sha256};
use sha3::Keccak256;

pub fn sha256(data: &[u8]) -> [u8; 32] {
    let digest = Sha256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn double_sha256(data: &[u8]) -> [u8; 32] {
    sha256(&sha256(data))
}

pub fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let ripemd = Ripemd160::digest(sha);
    let mut out = [0u8; 20];
    out.copy_from_slice(&ripemd);
    out
}

pub fn keccak256(data: &[u8]) -> [u8; 32] {
    let digest = Keccak256::digest(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub fn compressed_pubkey(secret: &SecretKey) -> [u8; 33] {
    let point = secret.public_key().to_encoded_point(true);
    let mut out = [0u8; 33];
    out.copy_from_slice(point.as_bytes());
    out
}

pub fn uncompressed_pubkey(secret: &SecretKey) -> [u8; 65] {
    let point = secret.public_key().to_encoded_point(false);
    let mut out = [0u8; 65];
    out.copy_from_slice(point.as_bytes());
    out
}

/// Encode a version byte plus payload using Base58Check.
pub fn base58check(version: u8, payload: &[u8]) -> String {
    let mut data = Vec::with_capacity(payload.len() + 1);
    data.push(version);
    data.extend_from_slice(payload);
    bs58::encode(data).with_check().into_string()
}

pub fn decode_base58check(address: &str) -> Result<Vec<u8>> {
    bs58::decode(address)
        .with_check(None)
        .into_vec()
        .map_err(|e| Error::InvalidAddress(e.to_string()))
}

pub fn p2pkh_address(pubkey: &[u8], version: u8) -> String {
    base58check(version, &hash160(pubkey))
}

pub fn p2wpkh_address(pubkey: &[u8], hrp: &str) -> Result<String> {
    let program = hash160(pubkey);
    let hrp = Hrp::parse(hrp).map_err(|e| Error::Crypto(format!("invalid bech32 hrp: {e}")))?;
    segwit::encode(hrp, segwit::VERSION_0, &program)
        .map_err(|e| Error::Crypto(format!("bech32 encode failed: {e}")))
}

pub fn evm_address(pubkey: &[u8]) -> Result<String> {
    if pubkey.len() != 65 || pubkey[0] != 0x04 {
        return Err(Error::Crypto("expected uncompressed secp256k1 key".into()));
    }
    let hash = keccak256(&pubkey[1..]);
    let mut address = [0u8; 20];
    address.copy_from_slice(&hash[12..]);
    Ok(to_checksum_address(&hex::encode(address)))
}

/// EIP-55 checksum encoding.
pub fn to_checksum_address(lower_hex: &str) -> String {
    let lower = lower_hex.trim_start_matches("0x").to_ascii_lowercase();
    let hash = keccak256(lower.as_bytes());
    let mut out = String::with_capacity(42);
    out.push_str("0x");
    for (i, ch) in lower.chars().enumerate() {
        if ch.is_ascii_digit() {
            out.push(ch);
        } else {
            let nibble = (hash[i / 2] >> (4 * (1 - (i % 2)))) & 0x0f;
            if nibble >= 8 {
                out.push(ch.to_ascii_uppercase());
            } else {
                out.push(ch);
            }
        }
    }
    out
}

pub fn solana_address(pubkey: &[u8; 32]) -> String {
    bs58::encode(pubkey).into_string()
}

pub fn parse_solana_address(address: &str) -> Result<[u8; 32]> {
    let bytes = bs58::decode(address.trim())
        .into_vec()
        .map_err(|_| Error::InvalidAddress("not a base58 Solana address".into()))?;
    if bytes.len() != 32 {
        return Err(Error::InvalidAddress(
            "Solana addresses must decode to 32 bytes".into(),
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn parse_evm_address(address: &str) -> Result<[u8; 20]> {
    let trimmed = address.trim();
    if !trimmed.starts_with("0x") || trimmed.len() != 42 {
        return Err(Error::InvalidAddress(
            "EVM addresses must be 0x followed by 40 hex characters".into(),
        ));
    }
    let body = &trimmed[2..];
    if !body.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::InvalidAddress(
            "address contains non-hex characters".into(),
        ));
    }
    let has_lower = body.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = body.chars().any(|c| c.is_ascii_uppercase());
    if has_lower && has_upper {
        let checksum = to_checksum_address(body);
        if checksum[2..] != *body {
            return Err(Error::InvalidAddress(
                "mixed-case address has an invalid EIP-55 checksum".into(),
            ));
        }
    }
    let bytes = hex::decode(body)?;
    let mut out = [0u8; 20];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn validate_address(chain: ChainId, address: &str) -> Result<()> {
    match chain {
        ChainId::Ethereum
        | ChainId::EthereumClassic
        | ChainId::Ethw
        | ChainId::Bsc
        | ChainId::Polygon => parse_evm_address(address).map(|_| ()),
        ChainId::Solana => parse_solana_address(address).map(|_| ()),
        ChainId::Bitcoin
        | ChainId::Litecoin
        | ChainId::Dogecoin
        | ChainId::BitcoinCash
        | ChainId::BitcoinSv
        | ChainId::Namecoin
        | ChainId::LitecoinCash
        | ChainId::Feathercoin
        | ChainId::Viacoin
        | ChainId::Potcoin
        | ChainId::EGulden
        | ChainId::Dash
        | ChainId::Groestlcoin
        | ChainId::Vertcoin
        | ChainId::Ravencoin
        | ChainId::Monacoin
        | ChainId::Fujicoin => validate_utxo_address(chain, address),
        ChainId::Monero => {
            if crate::cryptonote::monero_address_validate(address) {
                Ok(())
            } else {
                Err(Error::InvalidAddress(
                    "不是有效的 Monero 主网地址（长度 / 校验和不符）".into(),
                ))
            }
        }
    }
}

pub fn validate_utxo_address(chain: ChainId, address: &str) -> Result<()> {
    let params = chain
        .utxo_params()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let address = address.trim();
    // Bitcoin Cash also uses cashaddr (`bitcoincash:q…`), which is neither
    // bech32 nor base58. Accept it when the prefix matches this chain.
    if let Some((prefix, _)) = address.split_once(':') {
        if prefix.eq_ignore_ascii_case(params.hrp) {
            let (version, hash) = cashaddr_decode(address)?;
            if version != 0 {
                return Err(Error::InvalidAddress(
                    "only P2PKH cashaddr is supported".into(),
                ));
            }
            if hash.len() != 20 {
                return Err(Error::InvalidAddress("invalid cashaddr hash".into()));
            }
            return Ok(());
        }
        return Err(Error::InvalidAddress("wrong network".into()));
    }
    if let Ok((hrp, version, program)) = segwit::decode(address) {
        if hrp.as_str() != params.hrp {
            return Err(Error::InvalidAddress(format!(
                "address belongs to {}, expected {}",
                hrp.as_str(),
                params.hrp
            )));
        }
        if version.to_u8() > 1 {
            return Err(Error::InvalidAddress("unsupported witness version".into()));
        }
        if program.len() < 2 || program.len() > 40 {
            return Err(Error::InvalidAddress("invalid witness program".into()));
        }
        return Ok(());
    }
    let payload = decode_base58check(address)?;
    if payload.len() != 21 {
        return Err(Error::InvalidAddress(
            "invalid base58 address length".into(),
        ));
    }
    let version = payload[0];
    if version != params.p2pkh_version && version != params.p2sh_version {
        return Err(Error::InvalidAddress(format!(
            "unexpected address version 0x{version:02x}"
        )));
    }
    Ok(())
}

/// Parse a UTXO address into its output script.
pub fn utxo_output_script(chain: ChainId, address: &str) -> Result<Vec<u8>> {
    let params = chain
        .utxo_params()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let address = address.trim();
    if address.contains(':') {
        // Cashaddr (Bitcoin Cash) and bech32 (SegWit).
        if let Some((prefix, _)) = address.split_once(':') {
            if prefix.eq_ignore_ascii_case(params.hrp) {
                let (version, hash) = cashaddr_decode(address)?;
                if version != 0 {
                    return Err(Error::InvalidAddress(
                        "only P2PKH cashaddr is supported".into(),
                    ));
                }
                let mut script = Vec::with_capacity(25);
                script.extend_from_slice(&[0x76, 0xa9, 0x14]);
                script.extend_from_slice(&hash);
                script.extend_from_slice(&[0x88, 0xac]);
                return Ok(script);
            }
            return Err(Error::InvalidAddress("wrong network".into()));
        }
    }
    if let Ok((hrp, version, program)) = segwit::decode(address) {
        if hrp.as_str() != params.hrp {
            return Err(Error::InvalidAddress("wrong network".into()));
        }
        let mut script = Vec::with_capacity(program.len() + 2);
        let v = version.to_u8();
        if v == 0 {
            script.push(0x00);
        } else {
            script.push(0x50 + v);
        }
        script.push(program.len() as u8);
        script.extend_from_slice(&program);
        return Ok(script);
    }
    let payload = decode_base58check(address)?;
    if payload.len() != 21 {
        return Err(Error::InvalidAddress("invalid base58 address".into()));
    }
    let version = payload[0];
    let hash = &payload[1..];
    if version == params.p2pkh_version {
        let mut script = Vec::with_capacity(25);
        script.extend_from_slice(&[0x76, 0xa9, 0x14]);
        script.extend_from_slice(hash);
        script.extend_from_slice(&[0x88, 0xac]);
        Ok(script)
    } else if version == params.p2sh_version {
        let mut script = Vec::with_capacity(23);
        script.extend_from_slice(&[0xa9, 0x14]);
        script.extend_from_slice(hash);
        script.push(0x87);
        Ok(script)
    } else {
        Err(Error::InvalidAddress("unsupported address version".into()))
    }
}

/// WIF for UTXO chains. The corresponding public key is always compressed.
pub fn wif(secret: &SecretKey, chain: ChainId) -> Result<String> {
    let params = chain
        .utxo_params()
        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
    let mut data = Vec::with_capacity(34);
    data.push(params.wif_version);
    data.extend_from_slice(&secret.to_bytes());
    data.push(0x01);
    Ok(bs58::encode(data).with_check().into_string())
}

/* ---------------------------------------------------------------- cashaddr */

const CASHADDR_CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// Polymod over the BCH cashaddr generator matrix (see the cashaddr spec).
fn cashaddr_polymod(values: &[u8]) -> u64 {
    const GENERATORS: [u64; 5] = [
        0x0098_f2bc_8e61,
        0x0079_b76d_99e2,
        0x00f3_3e5f_b3c4,
        0x00ae_2eab_e2a8,
        0x001e_4f43_e470,
    ];
    // The working value keeps 35 bits, the generators are folded in below.
    const MASK: u64 = (1 << 35) - 1;
    let mut c: u64 = 1;
    for value in values {
        let c0 = c >> 35;
        c = ((c & MASK) << 5) ^ u64::from(*value);
        for (index, mask) in GENERATORS.iter().enumerate() {
            if (c0 >> index) & 1 == 1 {
                c ^= mask;
            }
        }
    }
    c ^ 1
}

fn cashaddr_data(payload: &[u8], version: u8) -> Vec<u8> {
    // The payload is `version byte || hash` converted to 5-bit groups, msb
    // first, padded to a full group with zeroes.
    let mut bits: Vec<bool> = Vec::with_capacity(8 + payload.len() * 8);
    for index in (0..8).rev() {
        bits.push((version >> index) & 1 == 1);
    }
    for byte in payload {
        for index in (0..8).rev() {
            bits.push((*byte >> index) & 1 == 1);
        }
    }
    while !bits.len().is_multiple_of(5) {
        bits.push(false);
    }
    bits.chunks(5)
        .map(|chunk| {
            chunk
                .iter()
                .fold(0u8, |acc, bit| (acc << 1) | u8::from(*bit))
        })
        .collect()
}

/// Encode a hash160 payload as a BCH cashaddr string (`prefix:address`).
pub fn cashaddr_encode(hash160: &[u8], prefix: &str) -> Result<String> {
    if hash160.len() != 20 {
        return Err(Error::InvalidAddress(
            "cashaddr payload must be 20 bytes".into(),
        ));
    }
    let prefix_lower: Vec<u8> = prefix.bytes().map(|b| b.to_ascii_lowercase()).collect();
    let data = cashaddr_data(hash160, 0);
    let mut values: Vec<u8> = Vec::with_capacity(prefix_lower.len() + 1 + data.len() + 8);
    for byte in &prefix_lower {
        values.push(byte & 0x1f);
    }
    values.push(0);
    values.extend_from_slice(&data);
    // The checksum covers eight zero groups that hold the result.
    values.extend_from_slice(&[0u8; 8]);
    let checksum = cashaddr_polymod(&values);
    let mut body = String::new();
    for value in &data {
        body.push(CASHADDR_CHARSET[*value as usize] as char);
    }
    let mut checksum_chars = String::new();
    for index in (0..8).rev() {
        checksum_chars.push(CASHADDR_CHARSET[((checksum >> (index * 5)) & 0x1f) as usize] as char);
    }
    let prefix_text: String = prefix.chars().map(|c| c.to_ascii_lowercase()).collect();
    Ok(format!("{prefix_text}:{body}{checksum_chars}"))
}

/// Decode a BCH cashaddr string into its version byte and hash160 payload.
///
/// The address is `prefix:body`, where the body is base32 (cashaddr charset)
/// over `version byte || hash160` plus an eight group checksum. Both the
/// prefix and the checksum are verified, so a typo cannot silently produce a
/// script for the wrong address.
pub fn cashaddr_decode(address: &str) -> Result<(u8, Vec<u8>)> {
    let (prefix, body) = address
        .trim()
        .split_once(':')
        .ok_or_else(|| Error::InvalidAddress("cashaddr needs a prefix".into()))?;
    let prefix_lower: Vec<u8> = prefix.bytes().map(|b| b.to_ascii_lowercase()).collect();

    let mut values: Vec<u8> = Vec::with_capacity(body.len());
    for character in body.chars() {
        let lower = character.to_ascii_lowercase() as u8;
        let index = CASHADDR_CHARSET
            .iter()
            .position(|c| *c == lower)
            .ok_or_else(|| Error::InvalidAddress("invalid cashaddr character".into()))?;
        values.push(index as u8);
    }
    if values.len() < 9 {
        return Err(Error::InvalidAddress("cashaddr is too short".into()));
    }

    // The checksum covers the prefix, the payload and eight zero groups.
    let mut check: Vec<u8> = prefix_lower.iter().map(|b| b & 0x1f).collect();
    check.push(0);
    check.extend_from_slice(&values);
    if cashaddr_polymod(&check) != 0 {
        return Err(Error::InvalidAddress(
            "cashaddr checksum does not match".into(),
        ));
    }

    // Drop the checksum and convert the remaining 5-bit groups back to bytes.
    let payload_bits = &values[..values.len() - 8];
    let mut bits: Vec<bool> = Vec::with_capacity(payload_bits.len() * 5);
    for value in payload_bits {
        for index in (0..5).rev() {
            bits.push((value >> index) & 1 == 1);
        }
    }
    let mut bytes: Vec<u8> = Vec::with_capacity(bits.len() / 8);
    for chunk in bits.chunks(8) {
        let byte = chunk
            .iter()
            .fold(0u8, |acc, bit| (acc << 1) | u8::from(*bit));
        bytes.push(byte);
    }
    if bytes.len() < 21 {
        return Err(Error::InvalidAddress(
            "cashaddr payload is too short".into(),
        ));
    }
    let version = bytes[0];
    Ok((version, bytes[1..21].to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eip55_known_vector() {
        assert_eq!(
            to_checksum_address("52908400098527886e0f7030069857d2e4169ee7"),
            "0x52908400098527886E0F7030069857D2E4169EE7"
        );
        assert_eq!(
            to_checksum_address("5aaeb6053f3e94c9b9a09f33669435e7ef1beaed"),
            "0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed"
        );
        assert_eq!(
            to_checksum_address("fb6916095ca1df60bb79ce92ce3ea74c37c5d359"),
            "0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359"
        );
    }

    #[test]
    fn base58check_roundtrip() {
        let encoded = base58check(0x00, &[0x11; 20]);
        let decoded = decode_base58check(&encoded).unwrap();
        assert_eq!(decoded[0], 0x00);
        assert_eq!(&decoded[1..], &[0x11; 20]);
    }

    #[test]
    fn cashaddr_known_vector() {
        // Official test vector from the cashaddr specification: 20-byte
        // P2PKH payload F5BF…DAC9 on mainnet.
        let hash = hex::decode("f5bf48b397dae70be82b3cca4793f8eb2b6cdac9").unwrap();
        let encoded = cashaddr_encode(&hash, "bitcoincash").unwrap();
        assert_eq!(
            encoded,
            "bitcoincash:qr6m7j9njldwwzlg9v7v53unlr4jkmx6eylep8ekg2"
        );

        // The decoder has to agree with the encoder.
        let (version, decoded) = cashaddr_decode(&encoded).unwrap();
        assert_eq!(version, 0);
        assert_eq!(decoded, hash);

        // A broken checksum must not decode.
        let mut broken = encoded.clone();
        let last = broken.pop().unwrap();
        broken.push(if last == 'q' { 'p' } else { 'q' });
        assert!(cashaddr_decode(&broken).is_err());
    }

    #[test]
    fn cashaddr_address_becomes_a_p2pkh_script() {
        // The wallet's own Bitcoin Cash address (cashaddr) has to resolve to
        // the legacy P2PKH script, so it can pay and receive.
        let address = "bitcoincash:qpdtccrxx78kcuc65mceurfwg60gmmqu9cwpjdt25n";
        let script = utxo_output_script(ChainId::BitcoinCash, address).unwrap();
        assert_eq!(script.len(), 25);
        assert_eq!(&script[..3], &[0x76, 0xa9, 0x14]);
        assert_eq!(&script[23..], &[0x88, 0xac]);
        // Without its prefix the same string is not a valid cashaddr.
        assert!(utxo_output_script(
            ChainId::BitcoinCash,
            "qpdtccrxx78kcuc65mceurfwg60gmmqu9cwpjdt25n"
        )
        .is_err());
    }

    #[test]
    fn cashaddr_addresses_validate() {
        let address = "bitcoincash:qpdtccrxx78kcuc65mceurfwg60gmmqu9cwpjdt25n";
        assert!(validate_address(ChainId::BitcoinCash, address).is_ok());
        // The same payload on another chain's prefix must be rejected.
        assert!(validate_address(ChainId::Bitcoin, address).is_err());
        // Checksum errors must not slip through.
        let mut broken = address.to_string();
        let last = broken.pop().unwrap();
        broken.push(if last == 'q' { 'p' } else { 'q' });
        assert!(validate_address(ChainId::BitcoinCash, &broken).is_err());
    }
}
