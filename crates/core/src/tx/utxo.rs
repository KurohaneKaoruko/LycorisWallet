//! Native SegWit (P2WPKH) transaction builder for Bitcoin and Litecoin.
//!
//! Only P2WPKH inputs are produced. The recipient may be any address whose
//! output script can be expressed as P2PKH, P2SH, P2WPKH or P2TR, which covers
//! every address a modern wallet is expected to pay.

use crate::address::{double_sha256, hash160};
use crate::chains::{ChainId, UtxoParams};
use crate::error::{Error, Result};
use k256::ecdsa::SigningKey;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;

pub const DUST_LIMIT: u64 = 546;
const SIGHASH_ALL: u32 = 1;
const SEQUENCE: u32 = 0xffff_fffd;

#[derive(Debug, Clone)]
pub struct UtxoInput {
    pub txid: [u8; 32],
    pub vout: u32,
    pub value: u64,
    pub confirmed: bool,
}

impl UtxoInput {
    pub fn from_parts(txid_hex: &str, vout: u32, value: u64, confirmed: bool) -> Result<Self> {
        Ok(Self {
            txid: parse_txid(txid_hex)?,
            vout,
            value,
            confirmed,
        })
    }
}

pub fn parse_txid(hex_str: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(hex_str.trim())?;
    if bytes.len() != 32 {
        return Err(Error::Transaction("transaction id must be 32 bytes".into()));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

pub fn varint(value: usize) -> Vec<u8> {
    if value < 0xfd {
        vec![value as u8]
    } else if value <= 0xffff {
        let mut out = vec![0xfd];
        out.extend_from_slice(&(value as u16).to_le_bytes());
        out
    } else if value <= 0xffff_ffff {
        let mut out = vec![0xfe];
        out.extend_from_slice(&(value as u32).to_le_bytes());
        out
    } else {
        let mut out = vec![0xff];
        out.extend_from_slice(&(value as u64).to_le_bytes());
        out
    }
}

fn outpoint(input: &UtxoInput) -> Vec<u8> {
    let mut out = Vec::with_capacity(36);
    let mut reversed = input.txid;
    reversed.reverse();
    out.extend_from_slice(&reversed);
    out.extend_from_slice(&input.vout.to_le_bytes());
    out
}

fn serialize_outputs(outputs: &[(Vec<u8>, u64)]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&varint(outputs.len()));
    for (script, value) in outputs {
        out.extend_from_slice(&value.to_le_bytes());
        out.extend_from_slice(&varint(script.len()));
        out.extend_from_slice(script);
    }
    out
}

pub fn p2wpkh_script(pubkey: &[u8; 33]) -> Vec<u8> {
    let mut script = Vec::with_capacity(22);
    script.push(0x00);
    script.push(0x14);
    script.extend_from_slice(&hash160(pubkey));
    script
}

fn script_code(pubkey: &[u8; 33]) -> Vec<u8> {
    let mut script = Vec::with_capacity(25);
    script.extend_from_slice(&[0x19, 0x76, 0xa9, 0x14]);
    script.extend_from_slice(&hash160(pubkey));
    script.extend_from_slice(&[0x88, 0xac]);
    script
}

pub struct Selection {
    pub inputs: Vec<UtxoInput>,
    pub change: u64,
    pub fee: u64,
}

pub fn estimate_vsize(input_count: usize, output_count: usize) -> u64 {
    // P2WPKH input ≈ 68 vbytes, P2WPKH output ≈ 31 vbytes, 11 vbytes overhead.
    11 + (input_count as u64) * 68 + (output_count as u64) * 31
}

/// Greedy largest-first coin selection. Prefers a change output, and absorbs
/// dust change into the fee when it would be uneconomical to create one.
pub fn select_utxos(
    mut utxos: Vec<UtxoInput>,
    target: u64,
    fee_rate: u64,
    include_change: bool,
) -> Result<Selection> {
    if target == 0 {
        return Err(Error::InvalidAmount(
            "amount must be greater than zero".into(),
        ));
    }
    utxos.sort_by(|a, b| b.value.cmp(&a.value));
    let output_count = if include_change { 2 } else { 1 };
    let mut selected: Vec<UtxoInput> = Vec::new();
    let mut total = 0u64;
    for utxo in &utxos {
        selected.push(utxo.clone());
        total = total.saturating_add(utxo.value);
        let vsize = estimate_vsize(selected.len(), output_count);
        let fee = vsize.saturating_mul(fee_rate.max(1));
        if total >= target.saturating_add(fee) {
            let mut change = total - target - fee;
            let mut fee = fee;
            if change < DUST_LIMIT {
                // Recompute with a single recipient output.
                let vsize = estimate_vsize(selected.len(), 1);
                let base_fee = vsize.saturating_mul(fee_rate.max(1));
                if total >= target.saturating_add(base_fee) {
                    fee = total - target;
                    change = 0;
                } else {
                    return Err(Error::InsufficientBalance);
                }
            }
            return Ok(Selection {
                inputs: selected,
                change,
                fee,
            });
        }
    }
    Err(Error::InsufficientBalance)
}

pub fn max_sendable(utxos: &[UtxoInput], fee_rate: u64) -> Result<(u64, u64)> {
    // Returns (max_amount, fee) by selecting all inputs and subtracting the
    // fee for a single output plus a change output when useful.
    if utxos.is_empty() {
        return Err(Error::InsufficientBalance);
    }
    let total: u64 = utxos.iter().map(|u| u.value).sum();
    let vsize = estimate_vsize(utxos.len(), 2);
    let fee = vsize.saturating_mul(fee_rate.max(1));
    if total <= fee {
        return Err(Error::InsufficientBalance);
    }
    let mut amount = total - fee;
    // If the remainder would create dust change, add it to the fee by using a
    // single output.
    let vsize_one = estimate_vsize(utxos.len(), 1);
    let fee_one = vsize_one.saturating_mul(fee_rate.max(1));
    let amount_one = total.saturating_sub(fee_one);
    let change = total - amount - fee;
    if change < DUST_LIMIT {
        amount = amount_one;
        Ok((amount, total - amount))
    } else {
        Ok((amount, fee))
    }
}

/// Build and sign a native SegWit transaction. The same key and P2WPKH script
/// is assumed for every input (the wallet only spends its own single account).
pub fn build_signed_p2wpkh(
    key: &SecretKey,
    inputs: &[UtxoInput],
    recipient_script: &[u8],
    recipient_value: u64,
    change: Option<(&[u8], u64)>,
    fee_rate: u64,
) -> Result<(String, u64)> {
    if inputs.is_empty() {
        return Err(Error::InsufficientBalance);
    }
    let pubkey_point = key.public_key().to_encoded_point(true);
    let mut pubkey = [0u8; 33];
    pubkey.copy_from_slice(pubkey_point.as_bytes());
    let signing = SigningKey::from_slice(&key.to_bytes())
        .map_err(|e| Error::Crypto(format!("invalid signing key: {e}")))?;

    let mut outputs: Vec<(Vec<u8>, u64)> = Vec::new();
    outputs.push((recipient_script.to_vec(), recipient_value));
    if let Some((script, value)) = change {
        if value >= DUST_LIMIT {
            outputs.push((script.to_vec(), value));
        }
    }

    let total_in: u64 = inputs.iter().map(|i| i.value).sum();
    let total_out: u64 = outputs.iter().map(|(_, v)| v).sum();
    if total_in < total_out {
        return Err(Error::InsufficientBalance);
    }
    let fee = total_in - total_out;
    let min_fee = estimate_vsize(inputs.len(), outputs.len()).saturating_mul(fee_rate.max(1));
    if fee < min_fee {
        return Err(Error::Transaction(format!(
            "fee {fee} is below the required {min_fee}"
        )));
    }

    let mut prevouts = Vec::new();
    let mut sequences = Vec::new();
    for input in inputs {
        prevouts.extend_from_slice(&outpoint(input));
        sequences.extend_from_slice(&SEQUENCE.to_le_bytes());
    }
    let hash_prevouts = double_sha256(&prevouts);
    let hash_sequence = double_sha256(&sequences);
    let hash_outputs = double_sha256(&serialize_outputs(&outputs));

    let code = script_code(&pubkey);
    let mut witnesses: Vec<Vec<Vec<u8>>> = Vec::with_capacity(inputs.len());
    for input in inputs {
        let mut preimage = Vec::with_capacity(156);
        preimage.extend_from_slice(&2i32.to_le_bytes());
        preimage.extend_from_slice(&hash_prevouts);
        preimage.extend_from_slice(&hash_sequence);
        preimage.extend_from_slice(&outpoint(input));
        preimage.extend_from_slice(&varint(code.len()));
        preimage.extend_from_slice(&code);
        preimage.extend_from_slice(&input.value.to_le_bytes());
        preimage.extend_from_slice(&SEQUENCE.to_le_bytes());
        preimage.extend_from_slice(&hash_outputs);
        preimage.extend_from_slice(&0u32.to_le_bytes());
        preimage.extend_from_slice(&SIGHASH_ALL.to_le_bytes());
        let sighash = double_sha256(&preimage);

        use k256::ecdsa::signature::hazmat::PrehashSigner;
        let signature: k256::ecdsa::Signature = signing
            .sign_prehash(&sighash)
            .map_err(|e| Error::Crypto(format!("Bitcoin signing failed: {e}")))?;
        let der = signature.to_der();
        let mut sig_with_type = der.as_bytes().to_vec();
        sig_with_type.push(0x01);
        witnesses.push(vec![sig_with_type, pubkey.to_vec()]);
    }

    // Serialize the SegWit transaction.
    let mut raw = Vec::new();
    raw.extend_from_slice(&2i32.to_le_bytes());
    raw.push(0x00);
    raw.push(0x01);
    raw.extend_from_slice(&varint(inputs.len()));
    for input in inputs {
        raw.extend_from_slice(&outpoint(input));
        raw.push(0x00);
        raw.extend_from_slice(&SEQUENCE.to_le_bytes());
    }
    raw.extend_from_slice(&serialize_outputs(&outputs));
    for stack in &witnesses {
        raw.extend_from_slice(&varint(stack.len()));
        for item in stack {
            raw.extend_from_slice(&varint(item.len()));
            raw.extend_from_slice(item);
        }
    }
    raw.extend_from_slice(&0u32.to_le_bytes());
    Ok((hex::encode(raw), fee))
}

/// Build a P2WPKH output script for the wallet's own change address.
pub fn change_script(params: &UtxoParams, pubkey: &[u8; 33]) -> (String, Vec<u8>) {
    let address = crate::address::p2wpkh_address(pubkey, params.hrp).unwrap_or_default();
    (address, p2wpkh_script(pubkey))
}

pub fn recipient_script(chain: ChainId, address: &str) -> Result<Vec<u8>> {
    crate::address::utxo_output_script(chain, address)
}

/// Return the fee rate for a named speed level.
pub fn fee_rate_for_level(fees: &crate::network::utxo::RecommendedFees, level: &str) -> u64 {
    match level {
        "fast" => fees.fastest,
        "slow" => fees.hour,
        "economy" => fees.economy,
        _ => fees.half_hour,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn txid_and_varint() {
        let txid = "00".repeat(32);
        let parsed = parse_txid(&txid).unwrap();
        assert_eq!(parsed, [0u8; 32]);
        assert_eq!(varint(0xfc), vec![0xfc]);
        assert_eq!(varint(0xfd), vec![0xfd, 0xfd, 0x00]);
    }

    #[test]
    fn selection_accounts_for_fee() {
        let utxos = vec![
            UtxoInput::from_parts(&"11".repeat(32), 0, 100_000, true).unwrap(),
            UtxoInput::from_parts(&"22".repeat(32), 1, 50_000, true).unwrap(),
        ];
        let selection = select_utxos(utxos, 40_000, 2, true).unwrap();
        assert!(!selection.inputs.is_empty());
        assert!(selection.fee >= estimate_vsize(selection.inputs.len(), 2) * 2);
    }

    #[test]
    fn signs_a_p2wpkh_transaction() {
        use k256::SecretKey;
        let key = SecretKey::from_slice(&[0x11; 32]).unwrap();
        let pubkey_point = key.public_key().to_encoded_point(true);
        let mut pubkey = [0u8; 33];
        pubkey.copy_from_slice(pubkey_point.as_bytes());
        let script = p2wpkh_script(&pubkey);
        let inputs = vec![UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap()];
        let (raw, fee) = build_signed_p2wpkh(&key, &inputs, &script, 90_000, None, 2).unwrap();
        assert!(fee >= 100);
        let bytes = hex::decode(&raw).unwrap();
        assert_eq!(&bytes[4..6], &[0x00, 0x01]);
    }

    /// Fixtures recomputed from scratch with libsecp256k1 (coincurve 21) and
    /// BIP143 hashing; see `tools/verify_tx.py`. Byte-identical DER signatures
    /// are expected because both signers use RFC 6979 with low-S normalisation.
    #[test]
    fn p2wpkh_transactions_match_reference_implementation() {
        use k256::SecretKey;
        let key = SecretKey::from_slice(&[0x11; 32]).unwrap();
        let pubkey_point = key.public_key().to_encoded_point(true);
        let mut pubkey = [0u8; 33];
        pubkey.copy_from_slice(pubkey_point.as_bytes());
        assert_eq!(
            hex::encode(pubkey),
            "034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa"
        );
        let script = p2wpkh_script(&pubkey);

        let inputs = vec![UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap()];
        let (raw, fee) = build_signed_p2wpkh(&key, &inputs, &script, 90_000, None, 2).unwrap();
        assert_eq!(fee, 10_000);
        assert_eq!(
            raw,
            "02000000000101abababababababababababababababababababababababababababababababab\
             0000000000fdffffff01905f010000000000160014fc7250a211deddc70ee5a2738de5f0781735\
             1cef02483045022100ad943b33d05052060da21158f92202d60d2575ed604c59dd26004405dfe8\
             165602202c0c7cd4e15824c8eb454fc15839142972e4e05c36a99d14c0541070ca37e754012103\
             4f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aa00000000"
        );

        let inputs = vec![
            UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap(),
            UtxoInput::from_parts(&"cd".repeat(32), 1, 60_000, true).unwrap(),
        ];
        let (raw, fee) =
            build_signed_p2wpkh(&key, &inputs, &script, 120_000, Some((&script, 25_000)), 2)
                .unwrap();
        assert_eq!(fee, 15_000);
        assert_eq!(
            raw,
            "02000000000102abababababababababababababababababababababababababababababababab\
             0000000000fdffffffcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\
             0100000000fdffffff02c0d4010000000000160014fc7250a211deddc70ee5a2738de5f07817351c\
             efa861000000000000160014fc7250a211deddc70ee5a2738de5f07817351cef02483045022100d2\
             f6e125e934b86405ebfa5da2c0c311ae9608d121b4e5a2502790512aa9e62f02204916b32c6dcf91\
             1b9cd5f17b6718bc68d8ca74c10fe56e3ed64071da6f0e6e270121034f355bdcb7cc0af728ef3cce\
             b9615d90684bb5b2ca5f859ab0f0b704075871aa02473044022005d6712cdb9602766a25918af382\
             5aa675c0b0ee3dcc3a1d9ba8c8fff2bc3a140220738cd9ff0d0b0c409b1afc830a5c76c11dbf2902\
             0e54992f6efeaa4c046a4cbc0121034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0\
             f0b704075871aa00000000"
        );
    }
}
