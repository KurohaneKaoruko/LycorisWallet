//! Prints raw transactions for cross-checking against independent libraries.
//!
//! Run with `cargo run -p lycoris-core --example tx_fixtures`.
//! The expected values are checked by `tools/verify_tx.py` and pinned as
//! regression tests in `tx/evm.rs`, `tx/utxo.rs` and `tx/solana.rs`.

use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::SecretKey;
use lycoris_core::chains::ChainId;
use lycoris_core::tx::evm::{sign_transaction, EvmCall, EvmFee};
use lycoris_core::tx::utxo::{
    build_signed_p2pkh, build_signed_p2wpkh, p2pkh_script, p2wpkh_script, UtxoInput,
};

fn pubkey_bytes(key: &SecretKey) -> [u8; 33] {
    let point = key.public_key().to_encoded_point(true);
    let mut out = [0u8; 33];
    out.copy_from_slice(point.as_bytes());
    out
}

fn main() {
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
    println!("evm_1559_raw={}", tx.raw_hex);
    println!("evm_1559_hash={}", tx.hash);

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
    println!("evm_legacy_raw={}", tx.raw_hex);
    println!("evm_legacy_hash={}", tx.hash);

    let tx = sign_transaction(
        &key,
        &EvmCall {
            chain_id: 137,
            nonce: 3,
            gas_limit: 60_000,
            to,
            value: 0,
            data: &[0xa9, 0x05, 0x9c, 0xbb], // transfer(address,uint256) selector fragment
            fee: EvmFee::Eip1559 {
                max_fee_per_gas: 200_000_000_000,
                max_priority_fee_per_gas: 30_000_000_000,
            },
        },
    )
    .unwrap();
    println!("evm_1559_data_raw={}", tx.raw_hex);
    println!("evm_1559_data_hash={}", tx.hash);

    // Bitcoin: two inputs, recipient plus change.
    let btc_key = SecretKey::from_slice(&[0x11; 32]).unwrap();
    let pubkey = pubkey_bytes(&btc_key);
    let script = p2wpkh_script(&pubkey);
    let inputs = vec![
        UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap(),
        UtxoInput::from_parts(&"cd".repeat(32), 1, 60_000, true).unwrap(),
    ];
    let (raw, fee) = build_signed_p2wpkh(
        &btc_key,
        &inputs,
        &script,
        120_000,
        Some((&script, 25_000)),
        2,
        ChainId::Bitcoin,
    )
    .unwrap();
    println!("btc_two_inputs_raw={raw}");
    println!("btc_two_inputs_fee={fee}");
    println!("btc_pubkey={}", hex::encode(pubkey));

    let single = vec![UtxoInput::from_parts(&"ab".repeat(32), 0, 100_000, true).unwrap()];
    let (raw, fee) = build_signed_p2wpkh(
        &btc_key,
        &single,
        &script,
        90_000,
        None,
        2,
        ChainId::Bitcoin,
    )
    .unwrap();
    println!("btc_single_raw={raw}");
    println!("btc_single_fee={fee}");

    // Dogecoin (classic sighash) and Bitcoin Cash (SIGHASH_FORKID) use the
    // same P2PKH script as Bitcoin's legacy addresses.
    let legacy_script = p2pkh_script(&pubkey);
    for (label, chain) in [("doge", ChainId::Dogecoin), ("bch", ChainId::BitcoinCash)] {
        let (raw, fee) =
            build_signed_p2pkh(&btc_key, &single, &legacy_script, 90_000, None, 2, chain).unwrap();
        println!("{label}_single_raw={raw}");
        println!("{label}_single_fee={fee}");

        let (raw, fee) = build_signed_p2pkh(
            &btc_key,
            &inputs,
            &legacy_script,
            120_000,
            Some((&legacy_script, 25_000)),
            2,
            chain,
        )
        .unwrap();
        println!("{label}_two_inputs_raw={raw}");
        println!("{label}_two_inputs_fee={fee}");
    }
    println!("doge_pubkey={}", hex::encode(pubkey));

    // Solana: seed 7, recipient 9, blockhash 3 (base58 of 32 x 0x03).
    let blockhash = bs58::encode([3u8; 32]).into_string();
    let built =
        lycoris_core::tx::solana::build_transfer(&[7u8; 32], &[9u8; 32], 1_000_000, &blockhash)
            .unwrap();
    println!("sol_message_base64={}", built.message_base64);
    println!("sol_signature_base64={}", built.signature_base64);
    println!("sol_transaction_base64={}", built.transaction_base64);
}
