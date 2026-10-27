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
const LOCKTIME: u32 = 0;

/// How a legacy (pre-SegWit) input is signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegacySighash {
    /// The classic pre-BIP-143 digest: Bitcoin and Dogecoin style.
    Classic,
    /// The BIP-143 digest with the `SIGHASH_FORKID` flag: Bitcoin Cash style.
    ForkId,
    /// ZIP-243 (Sapling onward): a BLAKE2b-256 digest over the v4
    /// transaction image, personalised per consensus branch id.
    Zcash,
}

/// `SIGHASH_FORKID | SIGHASH_ALL` — Bitcoin Cash signs with fork id zero.
pub const SIGHASH_FORKID_ALL: u32 = 0x41;

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

/// Serialize outputs for the ZIP-243 hashOutputs preimage: zcashd's
/// `GetOutputsHash` serialises each `CTxOut` in a loop — the vector count
/// that leads the on-disk serialisation never enters the hash. Bitcoin's
/// BIP-143 does include it, which is why this differs from
/// [`serialize_outputs`].
fn zcash_serialize_outputs_for_hash(outputs: &[(Vec<u8>, u64)]) -> Vec<u8> {
    let mut out = Vec::new();
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

/// The classic pay-to-public-key-hash script (25 bytes).
pub fn p2pkh_script(pubkey: &[u8; 33]) -> Vec<u8> {
    let mut script = Vec::with_capacity(25);
    script.extend_from_slice(&[0x76, 0xa9, 0x14]);
    script.extend_from_slice(&hash160(pubkey));
    script.extend_from_slice(&[0x88, 0xac]);
    script
}

// ---------------------------------------------------------------------------
// Zcash transparent (ZIP-243)
//
// Zcash kept the Bitcoin P2PKH script model for its transparent pool, but
// replaced the signature digest: since Sapling every signature is a
// BLAKE2b-256 hash over a transaction image, personalised with
// "ZcashSigHash" plus the little-endian consensus branch id of the epoch
// the transaction will be mined in. The transparent-only transaction the
// wallet builds is a version-4 format:
//
//   header (0x80000004) · version_group_id (0x892f2085) · vin · vout ·
//   locktime · expiry_height · value_balance (0) · spend_desc_count (0) ·
//   output_desc_count (0) · joinsplit_count (0)
//
// Sources: zcashd src/script/interpreter.cpp (SignatureHash, personalisation
// constants), src/primitives/transaction.h (v4 serialisation order),
// src/consensus/upgrades.cpp (branch id table). Verified against the
// official sighash.json test vectors (see tests below).
// ---------------------------------------------------------------------------

/// Transaction version header: fOverwintered bit set, version 4.
const ZCASH_TX_HEADER: u32 = 0x8000_0004;
/// `SAPLING_VERSION_GROUP_ID` from zcashd transaction.h.
const ZCASH_VERSION_GROUP_ID: u32 = 0x892f_2085;
/// Consensus branch ids, `zcashd` consensus/upgrades.cpp. The sighash is
/// personalised with the branch id of the epoch the tx is *mined* in.
const ZCASH_BRANCH_NU6_2: u32 = 0x5437_f330;
/// ZIP-203: 0 disables the expiry check entirely — the safest value for a
/// wallet that does not track the tip height at signing time.
const ZCASH_EXPIRY_HEIGHT: u32 = 0;

/// BLAKE2b-256 with the 16-byte personalization prefix zcashd uses.
/// blake2 0.10 exposes persona only via the MAC type; an empty key gives
/// exactly the plain personalised hash zcashd computes.
fn zcash_personal_hash(data: &[u8], personal: &[u8; 16]) -> [u8; 32] {
    let mut hasher = blake2b_simd::Params::new()
        .hash_length(32)
        .personal(personal)
        .to_state();
    hasher.update(data);
    let mut out = [0u8; 32];
    out.copy_from_slice(hasher.finalize().as_bytes());
    out
}

/// Per-transaction fields the ZIP-243 digest depends on beyond the raw
/// inputs/outputs. Production code uses the constants; the official test
/// vectors parse them out of the raw transaction.
#[derive(Debug, Clone)]
pub struct ZcashSignContext {
    pub header: u32,
    pub version_group_id: u32,
    pub locktime: u32,
    pub expiry_height: u32,
    pub sequences: Vec<u32>,
    pub value_balance: i64,
}

impl ZcashSignContext {
    /// The wallet's own transparent-only transactions: v4 Sapling format,
    /// zero locktime (ZIP-203: expiry 0 disables the check), no shielded
    /// value, every input with the same RBF-ready sequence.
    pub fn wallet(inputs: usize) -> Self {
        Self {
            header: ZCASH_TX_HEADER,
            version_group_id: ZCASH_VERSION_GROUP_ID,
            locktime: LOCKTIME,
            expiry_height: ZCASH_EXPIRY_HEIGHT,
            sequences: vec![SEQUENCE; inputs],
            value_balance: 0,
        }
    }
}

/// hashPrevouts / hashSequence / hashOutputs — every prevout, sequence and
/// output is serialised in order and hashed with its own personalisation.
fn zcash_component_hashes(
    inputs: &[UtxoInput],
    outputs: &[(Vec<u8>, u64)],
    sequences: &[u32],
) -> ([u8; 32], [u8; 32], [u8; 32]) {
    const PREVOUTS: &[u8; 16] = b"ZcashPrevoutHash";
    const SEQUENCE_HASH: &[u8; 16] = b"ZcashSequencHash";
    const OUTPUTS: &[u8; 16] = b"ZcashOutputsHash";

    let mut prevouts = Vec::with_capacity(inputs.len() * 36);
    let mut sequence_bytes = Vec::with_capacity(sequences.len() * 4);
    for input in inputs {
        prevouts.extend_from_slice(&outpoint(input));
    }
    for seq in sequences {
        sequence_bytes.extend_from_slice(&seq.to_le_bytes());
    }
    (
        zcash_personal_hash(&prevouts, PREVOUTS),
        zcash_personal_hash(&sequence_bytes, SEQUENCE_HASH),
        zcash_personal_hash(&zcash_serialize_outputs_for_hash(outputs), OUTPUTS),
    )
}

/// zcashd's `uint256()` default: when a transaction has no joinsplits /
/// shielded spends / shielded outputs, the corresponding PrecomputedTransactionData
/// hash stays an all-zero `uint256` and that zero value — *not* a hash of
/// an empty stream — enters the ZIP-243 preimage. Verified end-to-end
/// against a real Sapling-era P2PKH spend (see
/// `zcash_sighash_verifies_a_real_mainnet_spend`).
const ZCASH_ZERO_HASH: [u8; 32] = [0u8; 32];

/// ZIP-243 signature digest for a transparent input (Sapling sigversion:
/// v4 transactions). `amount` is the value of the input being signed.
pub fn zcash_sighash(
    inputs: &[UtxoInput],
    outputs: &[(Vec<u8>, u64)],
    script_code: &[u8],
    index: usize,
    amount: u64,
    branch_id: u32,
    ctx: &ZcashSignContext,
) -> [u8; 32] {
    let (hash_prevouts, hash_sequence, hash_outputs) =
        zcash_component_hashes(inputs, outputs, &ctx.sequences);
    // Empty shielded bundles contribute all-zero hashes (see ZCASH_ZERO_HASH).
    let hash_joinsplits = ZCASH_ZERO_HASH;
    let hash_shielded_spends = ZCASH_ZERO_HASH;
    let hash_shielded_outputs = ZCASH_ZERO_HASH;

    let mut personal = [0u8; 16];
    personal[..12].copy_from_slice(b"ZcashSigHash");
    personal[12..].copy_from_slice(&branch_id.to_le_bytes());

    let mut preimage = Vec::with_capacity(256);
    // 1. header (version with overwintered bit)
    preimage.extend_from_slice(&ctx.header.to_le_bytes());
    // 2. version group id
    preimage.extend_from_slice(&ctx.version_group_id.to_le_bytes());
    // 3. hashPrevouts
    preimage.extend_from_slice(&hash_prevouts);
    // 4. hashSequence
    preimage.extend_from_slice(&hash_sequence);
    // 5. hashOutputs
    preimage.extend_from_slice(&hash_outputs);
    // 6. hashJoinSplits
    preimage.extend_from_slice(&hash_joinsplits);
    // 7. hashShieldedSpends (Sapling sigversion)
    preimage.extend_from_slice(&hash_shielded_spends);
    // 8. hashShieldedOutputs
    preimage.extend_from_slice(&hash_shielded_outputs);
    // 9. locktime
    preimage.extend_from_slice(&ctx.locktime.to_le_bytes());
    // 10. expiry height
    preimage.extend_from_slice(&ctx.expiry_height.to_le_bytes());
    // 11. sapling value balance
    preimage.extend_from_slice(&ctx.value_balance.to_le_bytes());
    // 12. sighash type — ZIP-243 serialises it as 4 bytes LE. The wallet
    // only ever signs with SIGHASH_ALL; SINGLE/NONE/ANYONECANPAY also swap
    // the component hashes above per zcashd's conditional rules.
    preimage.extend_from_slice(&SIGHASH_ALL.to_le_bytes());
    // 13. the input being signed: outpoint, scriptCode, amount, nSequence
    preimage.extend_from_slice(&outpoint(&inputs[index]));
    preimage.extend_from_slice(&varint(script_code.len()));
    preimage.extend_from_slice(script_code);
    preimage.extend_from_slice(&amount.to_le_bytes());
    preimage.extend_from_slice(&ctx.sequences[index].to_le_bytes());

    zcash_personal_hash(&preimage, &personal)
}

/// Serialize a transparent-only Zcash v4 transaction.
fn zcash_serialize_v4(
    inputs: &[UtxoInput],
    outputs: &[(Vec<u8>, u64)],
    script_sigs: &[Vec<u8>],
) -> Vec<u8> {
    let mut tx = Vec::with_capacity(64 + inputs.len() * 148 + outputs.len() * 34);
    tx.extend_from_slice(&ZCASH_TX_HEADER.to_le_bytes());
    tx.extend_from_slice(&ZCASH_VERSION_GROUP_ID.to_le_bytes());
    tx.extend_from_slice(&varint(inputs.len()));
    for (input, script_sig) in inputs.iter().zip(script_sigs) {
        tx.extend_from_slice(&outpoint(input));
        tx.extend_from_slice(&varint(script_sig.len()));
        tx.extend_from_slice(script_sig);
        tx.extend_from_slice(&SEQUENCE.to_le_bytes());
    }
    tx.extend_from_slice(&serialize_outputs(outputs));
    tx.extend_from_slice(&LOCKTIME.to_le_bytes());
    tx.extend_from_slice(&ZCASH_EXPIRY_HEIGHT.to_le_bytes());
    // valueBalanceSapling (int64, zero for transparent-only)
    tx.extend_from_slice(&0i64.to_le_bytes());
    // vSpendsSapling / vOutputsSapling / vJoinSplit: empty vector counts
    tx.push(0);
    tx.push(0);
    tx.push(0);
    tx
}

/// Transaction size estimate for a Zcash v4 transparent transaction: 2+4+4
/// header bytes, ~148-byte P2PKH inputs, 34-byte outputs, and the 8+1+1+1
/// byte tail (value balance + three empty bundle counts).
pub fn estimate_zcash_vsize(input_count: usize, output_count: usize) -> u64 {
    22 + (input_count as u64) * 148 + (output_count as u64) * 34
}

/// Build and sign a transparent Zcash v4 transaction (ZIP-243 digest).
#[allow(clippy::too_many_arguments)]
pub fn build_signed_zcash(
    key: &SecretKey,
    inputs: &[UtxoInput],
    recipient_script: &[u8],
    recipient_value: u64,
    change: Option<(&[u8], u64)>,
    fee_rate: u64,
    chain: ChainId,
) -> Result<(String, u64)> {
    let dust_limit = chain.dust_limit();
    if inputs.is_empty() {
        return Err(Error::InsufficientBalance);
    }
    let pubkey_point = key.public_key().to_encoded_point(true);
    let mut pubkey = [0u8; 33];
    pubkey.copy_from_slice(pubkey_point.as_bytes());
    let signing = SigningKey::from_slice(&key.to_bytes())
        .map_err(|e| Error::Crypto(format!("invalid signing key: {e}")))?;
    let script_code = p2pkh_script(&pubkey);

    let mut outputs: Vec<(Vec<u8>, u64)> = Vec::new();
    outputs.push((recipient_script.to_vec(), recipient_value));
    if let Some((script, value)) = change {
        if value >= dust_limit {
            outputs.push((script.to_vec(), value));
        }
    }

    let total_in: u64 = inputs.iter().map(|i| i.value).sum();
    let total_out: u64 = outputs.iter().map(|(_, v)| *v).sum();
    if total_in < total_out {
        return Err(Error::InsufficientBalance);
    }
    let fee = total_in - total_out;
    let min_fee = estimate_zcash_vsize(inputs.len(), outputs.len()).saturating_mul(fee_rate.max(1));
    if fee < min_fee {
        return Err(Error::Transaction(format!(
            "fee {fee} is below the required {min_fee}"
        )));
    }

    // Branch id of the epoch the transaction will be mined in. NU6.2 is
    // live since height 3,364,600 (well behind the current tip), and the
    // next upgrade would need a wallet update anyway.
    let branch_id = ZCASH_BRANCH_NU6_2;
    let sighash_type = SIGHASH_ALL;
    let ctx = ZcashSignContext::wallet(inputs.len());

    let mut script_sigs: Vec<Vec<u8>> = Vec::with_capacity(inputs.len());
    for index in 0..inputs.len() {
        let digest = zcash_sighash(
            inputs,
            &outputs,
            &script_code,
            index,
            inputs[index].value,
            branch_id,
            &ctx,
        );

        use k256::ecdsa::signature::hazmat::PrehashSigner;
        let signature: k256::ecdsa::Signature = signing
            .sign_prehash(&digest)
            .map_err(|e| Error::Crypto(format!("zcash signing failed: {e}")))?;
        let der = signature.to_der();
        let mut signature_with_type = der.as_bytes().to_vec();
        signature_with_type.push(sighash_type as u8);

        // scriptSig: <signature> <pubkey>
        let mut script_sig = Vec::with_capacity(signature_with_type.len() + pubkey.len() + 2);
        script_sig.push(signature_with_type.len() as u8);
        script_sig.extend_from_slice(&signature_with_type);
        script_sig.push(pubkey.len() as u8);
        script_sig.extend_from_slice(&pubkey);
        script_sigs.push(script_sig);
    }

    let tx_bytes = zcash_serialize_v4(inputs, &outputs, &script_sigs);
    Ok((hex::encode(&tx_bytes), fee))
}

/// Transaction size estimate for legacy P2PKH spends: one input is about 148
/// bytes, one output about 34 bytes, plus ten bytes of overhead.
pub fn estimate_legacy_vsize(input_count: usize, output_count: usize) -> u64 {
    10 + (input_count as u64) * 148 + (output_count as u64) * 34
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
    dust_limit: u64,
) -> Result<Selection> {
    if target == 0 {
        return Err(Error::InvalidAmount(
            "amount must be greater than zero".into(),
        ));
    }
    utxos.sort_by_key(|a| std::cmp::Reverse(a.value));
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
            if change < dust_limit {
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

pub fn max_sendable(utxos: &[UtxoInput], fee_rate: u64, dust_limit: u64) -> Result<(u64, u64)> {
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
    if change < dust_limit {
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
    chain: ChainId,
) -> Result<(String, u64)> {
    let dust_limit = chain.dust_limit();
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
        if value >= dust_limit {
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

/// Coin selection for legacy P2PKH spends: the size of an input and an output
/// differs from the SegWit case, so the fee is computed with its own estimate.
/// Zcash pays the extra v4 header bytes, hence the chain parameter.
pub fn select_legacy_utxos(
    mut utxos: Vec<UtxoInput>,
    target: u64,
    fee_rate: u64,
    dust_limit: u64,
    chain: ChainId,
) -> Result<Selection> {
    if target == 0 {
        return Err(Error::InvalidAmount(
            "amount must be greater than zero".into(),
        ));
    }
    let estimate = legacy_estimator(chain);
    utxos.sort_by_key(|a| std::cmp::Reverse(a.value));
    let mut selected: Vec<UtxoInput> = Vec::new();
    let mut total = 0u64;
    for utxo in &utxos {
        selected.push(utxo.clone());
        total = total.saturating_add(utxo.value);
        let fee = estimate(selected.len(), 2).saturating_mul(fee_rate.max(1));
        if total >= target.saturating_add(fee) {
            let mut change = total - target - fee;
            let mut fee = fee;
            if change < dust_limit {
                let base_fee = estimate(selected.len(), 1).saturating_mul(fee_rate.max(1));
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

/// The largest amount that can be sent with the given inputs, and its fee.
pub fn max_legacy_sendable(
    utxos: &[UtxoInput],
    fee_rate: u64,
    dust_limit: u64,
    chain: ChainId,
) -> Result<(u64, u64)> {
    if utxos.is_empty() {
        return Err(Error::InsufficientBalance);
    }
    let estimate = legacy_estimator(chain);
    let total: u64 = utxos.iter().map(|u| u.value).sum();
    let fee = estimate(utxos.len(), 2).saturating_mul(fee_rate.max(1));
    if total <= fee {
        return Err(Error::InsufficientBalance);
    }
    let amount = total - fee;
    let change = total - amount - fee;
    if change < dust_limit {
        let fee_one = estimate(utxos.len(), 1).saturating_mul(fee_rate.max(1));
        let amount_one = total.saturating_sub(fee_one);
        Ok((amount_one, total - amount_one))
    } else {
        Ok((amount, fee))
    }
}

/// Per-chain vsize estimator for the legacy selection helpers. Zcash pays
/// for the v4 header (and its trailing bundle counts) up front.
fn legacy_estimator(chain: ChainId) -> fn(usize, usize) -> u64 {
    if chain.legacy_sighash() == LegacySighash::Zcash {
        estimate_zcash_vsize
    } else {
        estimate_legacy_vsize
    }
}

/// Build a P2WPKH output script for the wallet's own change address.
pub fn change_script(params: &UtxoParams, pubkey: &[u8; 33]) -> (String, Vec<u8>) {
    let address = crate::address::p2wpkh_address(pubkey, params.hrp).unwrap_or_default();
    (address, p2wpkh_script(pubkey))
}

/// Build a legacy P2PKH output script for the wallet's own change address.
pub fn legacy_change_script(params: &UtxoParams, pubkey: &[u8; 33]) -> (String, Vec<u8>) {
    let address = crate::address::p2pkh_address(pubkey, params.p2pkh_version);
    (address, p2pkh_script(pubkey))
}

/// The classic pre-BIP-143 digest: every input but the one being signed
/// contributes an empty script.
fn legacy_sighash(
    inputs: &[UtxoInput],
    outputs: &[(Vec<u8>, u64)],
    script_code: &[u8],
    index: usize,
    sighash_type: u32,
) -> [u8; 32] {
    let mut preimage = Vec::with_capacity(256);
    preimage.extend_from_slice(&1i32.to_le_bytes());
    preimage.extend_from_slice(&varint(inputs.len()));
    for (position, input) in inputs.iter().enumerate() {
        preimage.extend_from_slice(&outpoint(input));
        if position == index {
            preimage.extend_from_slice(&varint(script_code.len()));
            preimage.extend_from_slice(script_code);
        } else {
            preimage.extend_from_slice(&varint(0));
        }
        preimage.extend_from_slice(&SEQUENCE.to_le_bytes());
    }
    preimage.extend_from_slice(&serialize_outputs(outputs));
    preimage.extend_from_slice(&LOCKTIME.to_le_bytes());
    preimage.extend_from_slice(&sighash_type.to_le_bytes());
    double_sha256(&preimage)
}

/// The BIP-143 digest with the fork id flag, as Bitcoin Cash uses it.
fn forkid_sighash(
    inputs: &[UtxoInput],
    outputs: &[(Vec<u8>, u64)],
    script_code: &[u8],
    index: usize,
    sighash_type: u32,
) -> [u8; 32] {
    let mut prevouts = Vec::with_capacity(inputs.len() * 36);
    let mut sequences = Vec::with_capacity(inputs.len() * 4);
    for input in inputs {
        prevouts.extend_from_slice(&outpoint(input));
        sequences.extend_from_slice(&SEQUENCE.to_le_bytes());
    }
    let hash_prevouts = double_sha256(&prevouts);
    let hash_sequence = double_sha256(&sequences);
    let hash_outputs = double_sha256(&serialize_outputs(outputs));

    let mut preimage = Vec::with_capacity(160);
    preimage.extend_from_slice(&1i32.to_le_bytes());
    preimage.extend_from_slice(&hash_prevouts);
    preimage.extend_from_slice(&hash_sequence);
    preimage.extend_from_slice(&hash_outputs);
    preimage.extend_from_slice(&outpoint(&inputs[index]));
    preimage.extend_from_slice(&varint(script_code.len()));
    preimage.extend_from_slice(script_code);
    preimage.extend_from_slice(&inputs[index].value.to_le_bytes());
    preimage.extend_from_slice(&SEQUENCE.to_le_bytes());
    preimage.extend_from_slice(&hash_outputs);
    preimage.extend_from_slice(&LOCKTIME.to_le_bytes());
    preimage.extend_from_slice(&sighash_type.to_le_bytes());
    double_sha256(&preimage)
}

/// Build and sign a legacy (pre-SegWit) P2PKH transaction.
///
/// The same key and P2PKH script is assumed for every input (the wallet only
/// spends its own account). The chain decides the digest: `Classic` for
/// Dogecoin, `ForkId` for Bitcoin Cash.
pub fn build_signed_p2pkh(
    key: &SecretKey,
    inputs: &[UtxoInput],
    recipient_script: &[u8],
    recipient_value: u64,
    change: Option<(&[u8], u64)>,
    fee_rate: u64,
    chain: ChainId,
) -> Result<(String, u64)> {
    if chain.legacy_sighash() == LegacySighash::Zcash {
        return build_signed_zcash(
            key,
            inputs,
            recipient_script,
            recipient_value,
            change,
            fee_rate,
            chain,
        );
    }
    let mode = chain.legacy_sighash();
    let dust_limit = chain.dust_limit();
    if inputs.is_empty() {
        return Err(Error::InsufficientBalance);
    }
    let pubkey_point = key.public_key().to_encoded_point(true);
    let mut pubkey = [0u8; 33];
    pubkey.copy_from_slice(pubkey_point.as_bytes());
    let signing = SigningKey::from_slice(&key.to_bytes())
        .map_err(|e| Error::Crypto(format!("invalid signing key: {e}")))?;
    let script_code = p2pkh_script(&pubkey);

    let mut outputs: Vec<(Vec<u8>, u64)> = Vec::new();
    outputs.push((recipient_script.to_vec(), recipient_value));
    if let Some((script, value)) = change {
        if value >= dust_limit {
            outputs.push((script.to_vec(), value));
        }
    }

    let total_in: u64 = inputs.iter().map(|i| i.value).sum();
    let total_out: u64 = outputs.iter().map(|(_, v)| v).sum();
    if total_in < total_out {
        return Err(Error::InsufficientBalance);
    }
    let fee = total_in - total_out;
    let min_fee =
        estimate_legacy_vsize(inputs.len(), outputs.len()).saturating_mul(fee_rate.max(1));
    if fee < min_fee {
        return Err(Error::Transaction(format!(
            "fee {fee} is below the required {min_fee}"
        )));
    }

    let sighash_type = match mode {
        LegacySighash::Classic => SIGHASH_ALL,
        LegacySighash::ForkId => SIGHASH_FORKID_ALL,
        // Routed to build_signed_zcash before this point; unreachable here.
        LegacySighash::Zcash => SIGHASH_ALL,
    };

    let mut script_sigs: Vec<Vec<u8>> = Vec::with_capacity(inputs.len());
    for index in 0..inputs.len() {
        let digest = match mode {
            LegacySighash::Classic => {
                legacy_sighash(inputs, &outputs, &script_code, index, sighash_type)
            }
            LegacySighash::ForkId => {
                forkid_sighash(inputs, &outputs, &script_code, index, sighash_type)
            }
            LegacySighash::Zcash => unreachable!("zcash is routed to build_signed_zcash"),
        };

        use k256::ecdsa::signature::hazmat::PrehashSigner;
        let signature: k256::ecdsa::Signature = signing
            .sign_prehash(&digest)
            .map_err(|e| Error::Crypto(format!("legacy signing failed: {e}")))?;
        let der = signature.to_der();
        let mut signature_with_type = der.as_bytes().to_vec();
        signature_with_type.push(sighash_type as u8);

        // scriptSig: <signature> <pubkey>, both pushed onto the stack.
        let mut script_sig = Vec::with_capacity(signature_with_type.len() + pubkey.len() + 2);
        script_sig.push(signature_with_type.len() as u8);
        script_sig.extend_from_slice(&signature_with_type);
        script_sig.push(pubkey.len() as u8);
        script_sig.extend_from_slice(&pubkey);
        script_sigs.push(script_sig);
    }

    let mut raw = Vec::new();
    raw.extend_from_slice(&1i32.to_le_bytes());
    raw.extend_from_slice(&varint(inputs.len()));
    for (index, input) in inputs.iter().enumerate() {
        raw.extend_from_slice(&outpoint(input));
        raw.extend_from_slice(&varint(script_sigs[index].len()));
        raw.extend_from_slice(&script_sigs[index]);
        raw.extend_from_slice(&SEQUENCE.to_le_bytes());
    }
    raw.extend_from_slice(&serialize_outputs(&outputs));
    raw.extend_from_slice(&LOCKTIME.to_le_bytes());
    Ok((hex::encode(raw), fee))
}

pub fn recipient_script(chain: ChainId, address: &str) -> Result<Vec<u8>> {
    crate::address::utxo_output_script(chain, address)
}

/// Return the fee rate for a named speed level.
/// Fee recommendation for the UTXO chains, in satoshis per virtual byte.
///
/// Lives here rather than in [`crate::network`] so the WebAssembly build (which
/// has no HTTP adapters) can still price a transaction.
#[derive(Debug, Clone, Copy)]
pub struct RecommendedFees {
    pub fastest: u64,
    pub half_hour: u64,
    pub hour: u64,
    pub economy: u64,
    pub minimum: u64,
}

pub fn fee_rate_for_level(fees: &RecommendedFees, level: &str) -> u64 {
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
    fn zcash_sighash_all_matches_a_real_mainnet_spend() {
        // Real mainnet transaction 3882ebcb1753b92166bb0ba31e55689733febe9f4d33afea627723e3e8b53095
        // (block 600001, Sapling epoch): one transparent P2PKH input signed
        // with SIGHASH_ALL, locktime 599990, expiry 600021, three outputs.
        // This drives the *public* `zcash_sighash` — the same entry point the
        // wallet signs through — with a non-zero expiry and multiple outputs.
        use k256::ecdsa::signature::hazmat::PrehashVerifier;
        use k256::ecdsa::{Signature, VerifyingKey};

        let inputs = [UtxoInput::from_parts(
            "b7f585ad239b715bfbbd1b4f7b7c2949b92d06151a30cf40c2a826922d2f34d9",
            0,
            1_182_382_016_963,
            true,
        )
        .unwrap()];
        let outputs: Vec<(Vec<u8>, u64)> = vec![
            (
                hex::decode("76a914f137bcd5c3256629211502638033bebde245c11d88ac").unwrap(),
                1_158_395_491_033,
            ),
            (
                hex::decode("76a9144bb3fc016b73934d94afaf967bfcdf8daa4de8f588ac").unwrap(),
                4_600_000_000,
            ),
            (
                hex::decode("76a914e082600e2a1f29e95887ac56a76fa3c0ce1e250788ac").unwrap(),
                19_386_523_348,
            ),
        ];
        let script_code =
            hex::decode("76a914940c091c62fcc12a8701ad6a8e828c0b1c6fc11288ac").unwrap();
        const SAPLING_BRANCH: u32 = 0x76b8_09bb;

        let ctx = ZcashSignContext {
            header: 0x8000_0004,
            version_group_id: 0x892f_2085,
            locktime: 599_990,
            expiry_height: 600_021,
            sequences: vec![0xffff_fffe],
            value_balance: 0,
        };
        let digest = zcash_sighash(
            &inputs,
            &outputs,
            &script_code,
            0,
            inputs[0].value,
            SAPLING_BRANCH,
            &ctx,
        );
        let pubkey = VerifyingKey::from_sec1_bytes(
            &hex::decode("03da83c07b64d33b510a41c54d5a4f860b8a366a0e1a7028ba8be57ec9bfee3e38")
                .unwrap(),
        )
        .unwrap();
        let sig = Signature::from_der(&hex::decode(
            "3044022060df094f587874af9490daa429ef1a3845a33e0e867c9e115d4b37e13921b60702206d765e13b6d2e6089752dec28a085abbef2b9763c739d0fffa3341964953ac9f",
        ).unwrap())
        .unwrap();
        pubkey
            .verify_prehash(&digest, &sig)
            .expect("SIGHASH_ALL digest must verify against the mainnet signature");
    }

    #[test]
    fn zcash_sighash_single_matches_a_real_mainnet_spend() {
        // Real mainnet transaction a9444d3678eb…d0d5 (block 600000, Sapling
        // epoch): two transparent P2PKH inputs signed with SIGHASH_SINGLE by
        // pubkey 02f71a…4533. The public `zcash_sighash` only implements
        // SIGHASH_ALL (the wallet's sole mode), so this recomputes the digest
        // with the SINGLE overrides zcashd applies — zeroed sequence hash and
        // the single CTxOut at the signed index — reusing the same primitives.
        use k256::ecdsa::signature::hazmat::PrehashVerifier;
        use k256::ecdsa::{Signature, VerifyingKey};

        let inputs = [
            UtxoInput::from_parts(
                "d4e944af7819a3bd6aaf0a111ae719dd35f2eca41ceb013678ec0b1a60252889",
                343,
                12_562_982,
                true,
            )
            .unwrap(),
            UtxoInput::from_parts(
                "8a6f840beb2962e90fe71458170bd0b59db0c7a4800ec1c1c50fb22bd2b2746c",
                338,
                11_876_773,
                true,
            )
            .unwrap(),
        ];
        let outputs: Vec<(Vec<u8>, u64)> = vec![
            (
                hex::decode("76a914b9d6c810beca0a1a54e6a53c109778c0cea8439788ac").unwrap(),
                24_000_000,
            ),
            (
                hex::decode("76a914d745f351e8eef2190d7d753b1ff9cfd8aa8b254988ac").unwrap(),
                437_885,
            ),
        ];
        let script_code =
            hex::decode("76a9148243ccf55859706f7b84e25c12bce2d93ad6777088ac").unwrap();
        const SAPLING_BRANCH: u32 = 0x76b8_09bb;
        const SIGHASH_SINGLE: u32 = 0x03;

        let digest = |index: usize| -> [u8; 32] {
            let prevouts = inputs
                .iter()
                .flat_map(|i| outpoint(i).to_vec())
                .collect::<Vec<u8>>();
            let h_prevouts = zcash_personal_hash(&prevouts, b"ZcashPrevoutHash");
            let h_sequence = ZCASH_ZERO_HASH;
            // SIGHASH_SINGLE hashes the CTxOut at the signed index only —
            // a bare CTxOut serialisation, without the vector count prefix.
            let (script, value) = &outputs[index];
            let mut single = Vec::with_capacity(9 + script.len());
            single.extend_from_slice(&value.to_le_bytes());
            single.extend_from_slice(&varint(script.len()));
            single.extend_from_slice(script);
            let h_outputs = zcash_personal_hash(&single, b"ZcashOutputsHash");

            let mut personal = [0u8; 16];
            personal[..12].copy_from_slice(b"ZcashSigHash");
            personal[12..].copy_from_slice(&SAPLING_BRANCH.to_le_bytes());

            let mut pre = Vec::with_capacity(256);
            pre.extend_from_slice(&0x8000_0004u32.to_le_bytes());
            pre.extend_from_slice(&0x892f_2085u32.to_le_bytes());
            pre.extend_from_slice(&h_prevouts);
            pre.extend_from_slice(&h_sequence);
            pre.extend_from_slice(&h_outputs);
            pre.extend_from_slice(&ZCASH_ZERO_HASH); // joinsplits
            pre.extend_from_slice(&ZCASH_ZERO_HASH); // shielded spends
            pre.extend_from_slice(&ZCASH_ZERO_HASH); // shielded outputs
            pre.extend_from_slice(&0u32.to_le_bytes()); // locktime
            pre.extend_from_slice(&0u32.to_le_bytes()); // expiry
            pre.extend_from_slice(&0i64.to_le_bytes()); // value balance
            pre.extend_from_slice(&SIGHASH_SINGLE.to_le_bytes());
            pre.extend_from_slice(&outpoint(&inputs[index]));
            pre.extend_from_slice(&varint(script_code.len()));
            pre.extend_from_slice(&script_code);
            pre.extend_from_slice(&inputs[index].value.to_le_bytes());
            pre.extend_from_slice(&0xffff_ffffu32.to_le_bytes());
            zcash_personal_hash(&pre, &personal)
        };

        let pubkey = VerifyingKey::from_sec1_bytes(
            &hex::decode("02f71aba63920ff1444ee5db28ae68a65123ae5b6dc96d81979e0b890fd8bb4533")
                .unwrap(),
        )
        .unwrap();
        let sigs = [
            "3045022100a41573dbddfb7afe1aa5050ec3d6acff660932b8091f8bcab6ac78f1e201ff250220271aa0c183f167472d4204eeaee4453b6692da60e817e0b9084e9aefdc408758",
            "3045022100e8564b04c99d7e780157fbf67f483c832eba129dfcc93eec3c2587486dab6e1802202aceb21ec2efe5bd1926647754996de4bdee453db9435dbe26ef2e232852abf5",
        ];
        for (index, sig_hex) in sigs.iter().enumerate() {
            let sig = Signature::from_der(&hex::decode(sig_hex).unwrap()).unwrap();
            pubkey
                .verify_prehash(&digest(index), &sig)
                .unwrap_or_else(|e| panic!("input {index} failed to verify: {e}"));
        }
    }

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
        let selection = select_utxos(utxos, 40_000, 2, true, DUST_LIMIT).unwrap();
        assert!(!selection.inputs.is_empty());
        assert!(selection.fee >= estimate_vsize(selection.inputs.len(), 2) * 2);
    }

    #[test]
    fn dogecoin_dust_is_one_whole_coin() {
        // Dogecoin relays nothing below one DOGE, so a change output between
        // 546 and 100_000_000 satoshis would be unspendable.
        assert_eq!(ChainId::Dogecoin.dust_limit(), 100_000_000);
        assert_eq!(ChainId::Bitcoin.dust_limit(), 546);
        assert_eq!(ChainId::BitcoinCash.dust_limit(), 546);

        let utxos = vec![UtxoInput::from_parts(&"11".repeat(32), 0, 120_000_000, true).unwrap()];
        // A leftover of 0.2 DOGE is below the Dogecoin dust limit, so it has
        // to be absorbed into the fee instead of becoming a change output.
        let selection = select_legacy_utxos(
            utxos,
            100_000_000,
            1_000,
            ChainId::Dogecoin.dust_limit(),
            ChainId::Dogecoin,
        )
        .unwrap();
        assert_eq!(selection.change, 0);
        assert_eq!(selection.fee, 20_000_000);
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
        let (raw, fee) =
            build_signed_p2wpkh(&key, &inputs, &script, 90_000, None, 2, ChainId::Bitcoin).unwrap();
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
        let (raw, fee) =
            build_signed_p2wpkh(&key, &inputs, &script, 90_000, None, 2, ChainId::Bitcoin).unwrap();
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
        let (raw, fee) = build_signed_p2wpkh(
            &key,
            &inputs,
            &script,
            120_000,
            Some((&script, 25_000)),
            2,
            ChainId::Bitcoin,
        )
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

    /// Legacy P2PKH spends, recomputed with libsecp256k1 (coincurve 21) in
    /// `tools/verify_tx_legacy.py`: Dogecoin signs with the classic
    /// pre-BIP143 digest, Bitcoin Cash with BIP143 plus SIGHASH_FORKID.
    /// Dogecoin also relays nothing below one DOGE, so a smaller change
    /// output is absorbed into the fee.
    #[test]
    fn legacy_p2pkh_transactions_match_reference_implementation() {
        use k256::SecretKey;
        let key = SecretKey::from_slice(&[0x11; 32]).unwrap();
        let pubkey_point = key.public_key().to_encoded_point(true);
        let mut pubkey = [0u8; 33];
        pubkey.copy_from_slice(pubkey_point.as_bytes());
        let script = p2pkh_script(&pubkey);
        let single = vec![UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap()];
        let pair = vec![
            UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap(),
            UtxoInput::from_parts(&"cd".repeat(32), 1, 60_000, true).unwrap(),
        ];

        let (raw, fee) =
            build_signed_p2pkh(&key, &single, &script, 90_000, None, 2, ChainId::Dogecoin).unwrap();
        assert_eq!(fee, 10000);
        assert_eq!(
            raw,
            "0100000001abababababababababababababababababababababababababababababababab000000006a47\
            304402201e5ac4d899e88dc911b9877149ae84ec5b179cde28023cee555d19a261023c9a02201da1c2e510\
            b0cdcb134b53881fb25d7d6f99a83a73a46fe2303b4b61513600a90121034f355bdcb7cc0af728ef3cceb9\
            615d90684bb5b2ca5f859ab0f0b704075871aafdffffff01905f0100000000001976a914fc7250a211dedd\
            c70ee5a2738de5f07817351cef88ac00000000"
        );

        let (raw, fee) = build_signed_p2pkh(
            &key,
            &pair,
            &script,
            120_000,
            Some((&script, 25_000)),
            2,
            ChainId::Dogecoin,
        )
        .unwrap();
        assert_eq!(fee, 40000);
        assert_eq!(
            raw,
            "0100000002abababababababababababababababababababababababababababababababab000000006a47\
            304402204b2368941eefc1fd4f79e9abce76d144e0a773e735f7fbb0c79a48563c55dfa80220373c1275fc\
            2c7625de76c8b1f1191a84eaff80f5913a3080ccd732b9bca775ac0121034f355bdcb7cc0af728ef3cceb9\
            615d90684bb5b2ca5f859ab0f0b704075871aafdffffffcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\
            cdcdcdcdcdcdcdcdcdcdcdcd010000006a473044022055d795970a5b9cff8bdfa5330647bec4a9acd401d0\
            ab7e447aa11a143c90ae24022024d9ad9c8d0aef211d50b7e5477b263fc9a279ab1993addcdf1343eec86e\
            cb7e0121034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aafdffffff01c0\
            d40100000000001976a914fc7250a211deddc70ee5a2738de5f07817351cef88ac00000000"
        );

        let (raw, fee) = build_signed_p2pkh(
            &key,
            &single,
            &script,
            90_000,
            None,
            2,
            ChainId::BitcoinCash,
        )
        .unwrap();
        assert_eq!(fee, 10000);
        assert_eq!(
            raw,
            "0100000001abababababababababababababababababababababababababababababababab000000006a47\
            304402203b475c2a3d3c827b16e6a9dbca01411796cb0ceca0c6f32356a0361cc14f49990220365630fb11\
            be46cf73b3fa0a18ae99878aa8ea8ac6c7943323f595b97452521d4121034f355bdcb7cc0af728ef3cceb9\
            615d90684bb5b2ca5f859ab0f0b704075871aafdffffff01905f0100000000001976a914fc7250a211dedd\
            c70ee5a2738de5f07817351cef88ac00000000"
        );

        let (raw, fee) = build_signed_p2pkh(
            &key,
            &pair,
            &script,
            120_000,
            Some((&script, 25_000)),
            2,
            ChainId::BitcoinCash,
        )
        .unwrap();
        assert_eq!(fee, 15000);
        assert_eq!(
            raw,
            "0100000002abababababababababababababababababababababababababababababababab000000006a47\
            304402206f52fa9177e781e86a40fe9e3dbfd60423c44f241c1d4a9bdbd03f22a681ea4602201ee97e102f\
            aee3c1d1c480309c219f2e58df933a0c86d8cae51af116bb2dc8654121034f355bdcb7cc0af728ef3cceb9\
            615d90684bb5b2ca5f859ab0f0b704075871aafdffffffcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\
            cdcdcdcdcdcdcdcdcdcdcdcd010000006b483045022100deac753f8943c8f75c90efd43f34875ec059e080\
            e0b505f9d9ac8f744265d8c102202db31468fc30861e8fca9a2a43f05edcbf239cb326a175c694cd21c53f\
            fb8a4c4121034f355bdcb7cc0af728ef3cceb9615d90684bb5b2ca5f859ab0f0b704075871aafdffffff02\
            c0d40100000000001976a914fc7250a211deddc70ee5a2738de5f07817351cef88aca86100000000000019\
            76a914fc7250a211deddc70ee5a2738de5f07817351cef88ac00000000"
        );
    }

    /// The full Zcash path: `build_signed_zcash` produces a v4 transaction
    /// whose transparent signatures verify against the ZIP-243 digest —
    /// parsed straight from the serialised bytes, so the test also pins the
    /// v4 wire format.
    #[test]
    fn builds_a_verifiable_zcash_v4_transaction() {
        use k256::ecdsa::signature::hazmat::PrehashVerifier;
        use k256::ecdsa::{Signature, VerifyingKey};
        use k256::SecretKey;

        let key = SecretKey::from_slice(&[0x11; 32]).unwrap();
        let pubkey_point = key.public_key().to_encoded_point(true);
        let mut pubkey = [0u8; 33];
        pubkey.copy_from_slice(pubkey_point.as_bytes());
        let script = p2pkh_script(&pubkey);

        let inputs = vec![
            UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap(),
            UtxoInput::from_parts(&"cd".repeat(32), 1, 60_000, true).unwrap(),
        ];
        let (raw, fee) = build_signed_zcash(
            &key,
            &inputs,
            &script,
            120_000,
            Some((&script, 25_000)),
            2,
            ChainId::Zcash,
        )
        .unwrap();
        assert_eq!(fee, 15_000);

        // --- Parse the serialised transaction back (v4 format) ---
        let tx = hex::decode(&raw).unwrap();
        assert_eq!(&tx[0..4], &0x8000_0004u32.to_le_bytes()); // header
        assert_eq!(&tx[4..8], &0x892f_2085u32.to_le_bytes()); // version group id
        assert_eq!(tx[8], 2); // vin count
        let mut script_sigs: Vec<Vec<u8>> = Vec::new();
        let mut pos = 9usize;
        for _ in 0..2 {
            pos += 32; // prevout txid
            pos += 4; // prevout vout
            let script_len = tx[pos] as usize;
            let script_sig = tx[pos + 1..pos + 1 + script_len].to_vec();
            pos += 1 + script_len;
            pos += 4; // sequence
            script_sigs.push(script_sig);
        }
        assert_eq!(tx[pos], 2); // vout count
        pos += 1;
        let mut outputs: Vec<(Vec<u8>, u64)> = Vec::new();
        for _ in 0..2 {
            let value = u64::from_le_bytes(tx[pos..pos + 8].try_into().unwrap());
            pos += 8;
            let script_len = tx[pos] as usize;
            let script = tx[pos + 1..pos + 1 + script_len].to_vec();
            pos += 1 + script_len;
            outputs.push((script, value));
        }
        // locktime, expiry, valueBalance, three empty bundle counts
        let locktime = u32::from_le_bytes(tx[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let expiry = u32::from_le_bytes(tx[pos..pos + 4].try_into().unwrap());
        pos += 4;
        let value_balance = i64::from_le_bytes(tx[pos..pos + 8].try_into().unwrap());
        pos += 8;
        assert_eq!(&tx[pos..pos + 3], &[0, 0, 0]); // empty spends/outputs/joinsplits
        assert_eq!(pos + 3, tx.len());

        // --- Recompute the digest from parsed fields and verify ---
        let ctx = ZcashSignContext {
            header: 0x8000_0004,
            version_group_id: 0x892f_2085,
            locktime,
            expiry_height: expiry,
            sequences: vec![SEQUENCE; 2],
            value_balance,
        };
        let verifying = VerifyingKey::from_sec1_bytes(&pubkey).unwrap();
        for (index, script_sig) in script_sigs.iter().enumerate() {
            // scriptSig = <push len><DER+hashtype><push len><pubkey>
            let sig_len = script_sig[0] as usize;
            let sig_with_type = &script_sig[1..1 + sig_len];
            let der = &sig_with_type[..sig_with_type.len() - 1];
            assert_eq!(*sig_with_type.last().unwrap(), SIGHASH_ALL as u8);
            let digest = zcash_sighash(
                &inputs,
                &outputs,
                &p2pkh_script(&pubkey),
                index,
                inputs[index].value,
                ZCASH_BRANCH_NU6_2,
                &ctx,
            );
            let signature = Signature::from_der(der).unwrap();
            verifying
                .verify_prehash(&digest, &signature)
                .unwrap_or_else(|e| panic!("input {index}: {e}"));
        }
    }
}
