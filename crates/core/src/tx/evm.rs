//! EVM transaction encoding and signing (EIP-1559 type 2 and EIP-155 legacy).

use crate::address::keccak256;
use crate::error::{Error, Result};
use k256::ecdsa::SigningKey;
use k256::SecretKey;

#[derive(Debug, Clone, Copy)]
pub enum EvmFee {
    Eip1559 {
        max_fee_per_gas: u128,
        max_priority_fee_per_gas: u128,
    },
    Legacy {
        gas_price: u128,
    },
}

pub struct SignedEvmTransaction {
    pub raw_hex: String,
    pub hash: String,
}

pub fn minimal_be(value: u128) -> Vec<u8> {
    if value == 0 {
        return Vec::new();
    }
    let bytes = value.to_be_bytes();
    let start = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
    bytes[start..].to_vec()
}

pub fn rlp_bytes(data: &[u8]) -> Vec<u8> {
    if data.len() == 1 && data[0] < 0x80 {
        return vec![data[0]];
    }
    if data.len() < 56 {
        let mut out = Vec::with_capacity(data.len() + 1);
        out.push(0x80 + data.len() as u8);
        out.extend_from_slice(data);
        out
    } else {
        let len = minimal_be(data.len() as u128);
        let mut out = Vec::with_capacity(data.len() + len.len() + 2);
        out.push(0xb7 + len.len() as u8);
        out.extend_from_slice(&len);
        out.extend_from_slice(data);
        out
    }
}

pub fn rlp_u128(value: u128) -> Vec<u8> {
    rlp_bytes(&minimal_be(value))
}

pub fn rlp_list(items: &[Vec<u8>]) -> Vec<u8> {
    let payload: Vec<u8> = items.concat();
    if payload.len() < 56 {
        let mut out = Vec::with_capacity(payload.len() + 1);
        out.push(0xc0 + payload.len() as u8);
        out.extend_from_slice(&payload);
        out
    } else {
        let len = minimal_be(payload.len() as u128);
        let mut out = Vec::with_capacity(payload.len() + len.len() + 2);
        out.push(0xf7 + len.len() as u8);
        out.extend_from_slice(&len);
        out.extend_from_slice(&payload);
        out
    }
}

struct Signed {
    recovery_id: u8,
    r: [u8; 32],
    s: [u8; 32],
}

fn sign_prehash(key: &SecretKey, preimage: &[u8]) -> Result<Signed> {
    let signing = SigningKey::from_slice(&key.to_bytes())
        .map_err(|e| Error::Crypto(format!("invalid signing key: {e}")))?;
    let digest = keccak256(preimage);
    let (signature, recovery_id) = signing
        .sign_prehash_recoverable(&digest)
        .map_err(|e| Error::Crypto(format!("ECDSA signing failed: {e}")))?;
    let bytes = signature.to_bytes();
    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&bytes[..32]);
    s.copy_from_slice(&bytes[32..]);
    Ok(Signed {
        recovery_id: recovery_id.to_byte(),
        r,
        s,
    })
}

/// Everything needed to serialise one EVM transaction. `data` is the calldata
/// (empty for a plain value transfer).
#[derive(Debug, Clone, Copy)]
pub struct EvmCall<'a> {
    pub chain_id: u64,
    pub nonce: u64,
    pub gas_limit: u64,
    pub to: [u8; 20],
    pub value: u128,
    pub data: &'a [u8],
    pub fee: EvmFee,
}

pub fn sign_transaction(key: &SecretKey, call: &EvmCall<'_>) -> Result<SignedEvmTransaction> {
    let EvmCall {
        chain_id,
        nonce,
        gas_limit,
        to,
        value,
        data,
        fee,
    } = *call;
    let to_bytes = to.to_vec();
    match fee {
        EvmFee::Eip1559 {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } => {
            let fields = vec![
                rlp_u128(chain_id as u128),
                rlp_u128(nonce as u128),
                rlp_u128(max_priority_fee_per_gas),
                rlp_u128(max_fee_per_gas),
                rlp_u128(gas_limit as u128),
                rlp_bytes(&to_bytes),
                rlp_u128(value),
                rlp_bytes(data),
                rlp_list(&[]),
            ];
            let payload = rlp_list(&fields);
            let mut preimage = Vec::with_capacity(payload.len() + 1);
            preimage.push(0x02);
            preimage.extend_from_slice(&payload);
            // EIP-2718 / EIP-1559: sign keccak256(0x02 || rlp(payload)).
            let signed = sign_prehash(key, &preimage)?;
            let mut signed_fields = fields;
            signed_fields.push(rlp_u128(signed.recovery_id as u128));
            signed_fields.push(rlp_bytes(&signed.r));
            signed_fields.push(rlp_bytes(&signed.s));
            let signed_payload = rlp_list(&signed_fields);
            let mut raw = Vec::with_capacity(signed_payload.len() + 1);
            raw.push(0x02);
            raw.extend_from_slice(&signed_payload);
            let hash = keccak256(&raw);
            Ok(SignedEvmTransaction {
                raw_hex: format!("0x{}", hex::encode(&raw)),
                hash: format!("0x{}", hex::encode(hash)),
            })
        }
        EvmFee::Legacy { gas_price } => {
            let signing_fields = vec![
                rlp_u128(nonce as u128),
                rlp_u128(gas_price),
                rlp_u128(gas_limit as u128),
                rlp_bytes(&to_bytes),
                rlp_u128(value),
                rlp_bytes(data),
                rlp_u128(chain_id as u128),
                rlp_bytes(&[]),
                rlp_bytes(&[]),
            ];
            let preimage = rlp_list(&signing_fields);
            let signed = sign_prehash(key, &preimage)?;
            let v = signed.recovery_id as u128 + 35 + (chain_id as u128) * 2;
            let signed_fields = vec![
                rlp_u128(nonce as u128),
                rlp_u128(gas_price),
                rlp_u128(gas_limit as u128),
                rlp_bytes(&to_bytes),
                rlp_u128(value),
                rlp_bytes(data),
                rlp_u128(v),
                rlp_bytes(&signed.r),
                rlp_bytes(&signed.s),
            ];
            let raw = rlp_list(&signed_fields);
            let hash = keccak256(&raw);
            Ok(SignedEvmTransaction {
                raw_hex: format!("0x{}", hex::encode(&raw)),
                hash: format!("0x{}", hex::encode(hash)),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rlp_known_encoding() {
        assert_eq!(hex::encode(rlp_bytes(b"dog")), "83646f67");
        assert_eq!(hex::encode(rlp_bytes(&[])), "80");
        assert_eq!(hex::encode(rlp_u128(0)), "80");
        assert_eq!(hex::encode(rlp_u128(15)), "0f");
        assert_eq!(hex::encode(rlp_u128(1024)), "820400");
        assert_eq!(
            hex::encode(rlp_list(&[rlp_bytes(b"cat"), rlp_bytes(b"dog"),])),
            "c88363617483646f67"
        );
    }

    /// Fixtures produced by `eth-account` 0.13 (RFC 6979 deterministic ECDSA)
    /// and re-checked with `tools/verify_tx.py`.
    #[test]
    fn signed_transactions_match_eth_account() {
        let key = SecretKey::from_slice(&[0x46; 32]).unwrap();
        let to = [0x35u8; 20];

        let tx = sign_transaction(
            &key,
            &EvmCall {
                chain_id: 1,
                nonce: 9,
                gas_limit: 21_000,
                to,
                value: 1_000_000_000_000_000_000,
                data: &[],
                fee: EvmFee::Eip1559 {
                    max_fee_per_gas: 1_000_000_000,
                    max_priority_fee_per_gas: 1_000_000_000,
                },
            },
        )
        .unwrap();
        assert_eq!(
            tx.raw_hex,
            "0x02f8720109843b9aca00843b9aca00825208943535353535353535353535353535353535353535\
             880de0b6b3a764000080c001a0010155c4c5eaa5eaf0dd3e53c0f98a9072d57f282f5754e70842\
             f6df73865ca2a01c8ad638ee2c3545e4f98754db0758697669abea92c14350d72d6d6a7bd6910b"
        );
        assert_eq!(
            tx.hash,
            "0x8f763a04fd0808438d76f8ad8e62d19e3f3c3169aa8d818f0722db41f8b56af4"
        );

        let tx = sign_transaction(
            &key,
            &EvmCall {
                chain_id: 56,
                nonce: 0,
                gas_limit: 21_000,
                to,
                value: 123_456_789_000_000_000,
                data: &[],
                fee: EvmFee::Legacy {
                    gas_price: 5_000_000_000,
                },
            },
        )
        .unwrap();
        assert_eq!(
            tx.raw_hex,
            "0xf86d8085012a05f2008252089435353535353535353535353535353535353535358801b69b4ba5\
             749200808194a02632fd25733f67f812e850c8c7c13a2dd53e6f07dbaf81c23b4cd353c68e3e41\
             a0450c502d66887c2438789baf06d0f608daae2e871825428e419d89b6c4fea841"
        );
        assert_eq!(
            tx.hash,
            "0x2a920334d3971570126fa7a68df7fd30da9a04757187188ced3e9caf4e7b9b15"
        );

        let tx = sign_transaction(
            &key,
            &EvmCall {
                chain_id: 137,
                nonce: 3,
                gas_limit: 60_000,
                to,
                value: 0,
                data: &[0xa9, 0x05, 0x9c, 0xbb],
                fee: EvmFee::Eip1559 {
                    max_fee_per_gas: 200_000_000_000,
                    max_priority_fee_per_gas: 30_000_000_000,
                },
            },
        )
        .unwrap();
        assert_eq!(
            tx.raw_hex,
            "0x02f8718189038506fc23ac00852e90edd00082ea609435353535353535353535353535353535353\
             535358084a9059cbbc080a0f4630073d52f049df398812870f26c5468d4758e36219a9c50aac90\
             028a944e2a038e66d80b2e1c5e60d3ac4240c813733063934e19a1c30fdf217119863803b12"
        );
        assert_eq!(
            tx.hash,
            "0xb2d497ff5c3751527a07f75ea21766893068255d193e35578d7e2de9fbe92db6"
        );
    }
}
