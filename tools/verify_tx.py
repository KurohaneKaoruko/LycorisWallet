#!/usr/bin/env python3
"""Independently verify Lycoris Wallet transaction fixtures.

The script re-derives every raw transaction emitted by
`cargo run -p lycoris-core --example tx_fixtures` using unrelated Python
libraries (eth-account for EVM, coincurve + hashlib for Bitcoin) and compares
the bytes. It also verifies the Bitcoin signatures with libsecp256k1.

Usage:
    python tools/verify_tx.py [fixtures.txt]
"""

import hashlib
import re
import subprocess
import sys
from pathlib import Path

import coincurve
from eth_account import Account

# ---------------------------------------------------------------- EVM (eth-account)


def evm_1559():
    acct = Account.from_key("0x" + "46" * 32)
    tx = {
        "chainId": 1,
        "nonce": 9,
        "maxPriorityFeePerGas": 1_000_000_000,
        "maxFeePerGas": 1_000_000_000,
        "gas": 21_000,
        "to": "0x3535353535353535353535353535353535353535",
        "value": 1_000_000_000_000_000_000,
        "data": b"",
    }
    signed = acct.sign_transaction(tx)
    return "0x" + signed.raw_transaction.hex(), "0x" + signed.hash.hex()


def evm_legacy():
    acct = Account.from_key("0x" + "46" * 32)
    tx = {
        "chainId": 56,
        "nonce": 0,
        "gasPrice": 5_000_000_000,
        "gas": 21_000,
        "to": "0x3535353535353535353535353535353535353535",
        "value": 123_456_789_000_000_000,
        "data": b"",
    }
    signed = acct.sign_transaction(tx)
    return "0x" + signed.raw_transaction.hex(), "0x" + signed.hash.hex()


def evm_1559_data():
    acct = Account.from_key("0x" + "46" * 32)
    tx = {
        "chainId": 137,
        "nonce": 3,
        "maxPriorityFeePerGas": 30_000_000_000,
        "maxFeePerGas": 200_000_000_000,
        "gas": 60_000,
        "to": "0x3535353535353535353535353535353535353535",
        "value": 0,
        "data": bytes.fromhex("a9059cbb"),
    }
    signed = acct.sign_transaction(tx)
    return "0x" + signed.raw_transaction.hex(), "0x" + signed.hash.hex()


# ------------------------------------------------- Bitcoin (manual BIP143 + coincurve)

SIGHASH_ALL = 1
SEQUENCE = 0xFFFFFFFD


def sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def double_sha256(data: bytes) -> bytes:
    return sha256(sha256(data))


def hash160(data: bytes) -> bytes:
    ripe = hashlib.new("ripemd160")
    ripe.update(sha256(data))
    return ripe.digest()


def varint(n: int) -> bytes:
    if n < 0xFD:
        return bytes([n])
    if n <= 0xFFFF:
        return b"\xfd" + n.to_bytes(2, "little")
    if n <= 0xFFFFFFFF:
        return b"\xfe" + n.to_bytes(4, "little")
    return b"\xff" + n.to_bytes(8, "little")


def script_code(pubkey: bytes) -> bytes:
    return bytes([0x19, 0x76, 0xA9, 0x14]) + hash160(pubkey) + bytes([0x88, 0xAC])


def bip143_sighash(inputs, outputs, pubkey):
    """Compute the BIP143 sighash for every input of a P2WPKH transaction."""
    prevouts = b"".join(txid[::-1] + vout.to_bytes(4, "little") for txid, vout, _ in inputs)
    sequences = SEQUENCE.to_bytes(4, "little") * len(inputs)
    outs = varint(len(outputs)) + b"".join(
        value.to_bytes(8, "little") + varint(len(script)) + script
        for script, value in outputs
    )
    hash_prevouts = double_sha256(prevouts)
    hash_sequence = double_sha256(sequences)
    hash_outputs = double_sha256(outs)
    code = script_code(pubkey)
    digests = []
    for txid, vout, value in inputs:
        preimage = (
            2 .to_bytes(4, "little")
            + hash_prevouts
            + hash_sequence
            + txid[::-1]
            + vout.to_bytes(4, "little")
            + varint(len(code))
            + code
            + value.to_bytes(8, "little")
            + SEQUENCE.to_bytes(4, "little")
            + hash_outputs
            + (0).to_bytes(4, "little")
            + SIGHASH_ALL.to_bytes(4, "little")
        )
        digests.append(double_sha256(preimage))
    return digests


def der_low_s(signature: bytes) -> bytes:
    return signature


def build_btc(inputs, recipient_script, recipient_value, change):
    key = coincurve.PrivateKey(bytes([0x11] * 32))
    pubkey = key.public_key.format(compressed=True)
    outputs = [(recipient_script, recipient_value)]
    if change is not None:
        outputs.append(change)
    digests = bip143_sighash(inputs, outputs, pubkey)
    witnesses = []
    for digest in digests:
        sig = key.sign(digest, hasher=None)  # RFC 6979, low-S, no extra hashing
        assert key.public_key.verify(sig, digest, hasher=None)
        witnesses.append(varint(2) + varint(len(sig) + 1) + sig + bytes([SIGHASH_ALL]) + varint(33) + pubkey)

    raw = 2 .to_bytes(4, "little")
    raw += b"\x00\x01"
    raw += varint(len(inputs))
    for txid, vout, _ in inputs:
        raw += txid[::-1] + vout.to_bytes(4, "little") + b"\x00" + SEQUENCE.to_bytes(4, "little")
    raw += varint(len(outputs))
    for script, value in outputs:
        raw += value.to_bytes(8, "little") + varint(len(script)) + script
    raw += b"".join(witnesses)
    raw += (0).to_bytes(4, "little")
    return raw.hex(), pubkey.hex(), digests


def btc_two_inputs():
    key_pub = coincurve.PrivateKey(bytes([0x11] * 32)).public_key.format(compressed=True)
    script = b"\x00\x14" + hash160(key_pub)
    inputs = [
        (bytes.fromhex("ab" * 32), 0, 100_000),
        (bytes.fromhex("cd" * 32), 1, 60_000),
    ]
    return build_btc(inputs, script, 120_000, (script, 25_000))


def btc_single():
    key_pub = coincurve.PrivateKey(bytes([0x11] * 32)).public_key.format(compressed=True)
    script = b"\x00\x14" + hash160(key_pub)
    inputs = [(bytes.fromhex("ab" * 32), 0, 100_000)]
    return build_btc(inputs, script, 90_000, None)


# ------------------------------------------------------------- Solana (solders)


def solana_transfer():
    from solders.hash import Hash
    from solders.instruction import AccountMeta, Instruction
    from solders.keypair import Keypair
    from solders.message import Message
    from solders.pubkey import Pubkey
    from solders.transaction import Transaction

    kp = Keypair.from_seed(bytes([7] * 32))
    to = Pubkey.from_bytes(bytes([9] * 32))
    system_program = Pubkey.from_bytes(bytes(32))
    blockhash = Hash.from_string(str(Pubkey.from_bytes(bytes([3] * 32))))

    data = (2).to_bytes(4, "little") + (1_000_000).to_bytes(8, "little")
    ix = Instruction(
        program_id=system_program,
        accounts=[AccountMeta(kp.pubkey(), True, True), AccountMeta(to, False, True)],
        data=data,
    )
    message = Message.new_with_blockhash([ix], kp.pubkey(), blockhash)
    transaction = Transaction.new_signed_with_payer([ix], kp.pubkey(), [kp], blockhash)
    assert bytes(transaction.message) == bytes(message)

    message_b64 = __import__("base64").b64encode(bytes(message)).decode()
    signature = bytes(transaction.signatures[0])
    return (
        message_b64,
        __import__("base64").b64encode(signature).decode(),
        __import__("base64").b64encode(bytes(transaction)).decode(),
    )


# ---------------------------------------------------------------------- comparison


def parse_fixtures(text):
    fixtures = {}
    for line in text.splitlines():
        match = re.match(r"^([a-z0-9_]+)=(.*)$", line.strip())
        if match:
            fixtures[match.group(1)] = match.group(2)
    return fixtures


def check(name, expected, actual, failures):
    if expected == actual:
        print(f"  ok   {name}")
    else:
        failures.append(name)
        print(f"  FAIL {name}\n       expected {expected}\n       actual   {actual}")


def main():
    if len(sys.argv) > 1:
        text = Path(sys.argv[1]).read_text()
    else:
        result = subprocess.run(
            ["cargo", "run", "-q", "-p", "lycoris-core", "--example", "tx_fixtures"],
            capture_output=True,
            text=True,
            check=True,
        )
        text = result.stdout
    fixtures = parse_fixtures(text)
    failures = []

    print("EVM fixtures (eth-account)")
    raw, tx_hash = evm_1559()
    check("evm_1559_raw", raw, fixtures.get("evm_1559_raw", ""), failures)
    check("evm_1559_hash", tx_hash, fixtures.get("evm_1559_hash", ""), failures)

    raw, tx_hash = evm_legacy()
    check("evm_legacy_raw", raw, fixtures.get("evm_legacy_raw", ""), failures)
    check("evm_legacy_hash", tx_hash, fixtures.get("evm_legacy_hash", ""), failures)

    raw, tx_hash = evm_1559_data()
    check("evm_1559_data_raw", raw, fixtures.get("evm_1559_data_raw", ""), failures)
    check("evm_1559_data_hash", tx_hash, fixtures.get("evm_1559_data_hash", ""), failures)

    print("Bitcoin fixtures (coincurve / BIP143)")
    raw, pubkey, _ = btc_two_inputs()
    check("btc_pubkey", pubkey, fixtures.get("btc_pubkey", ""), failures)
    check("btc_two_inputs_raw", raw, fixtures.get("btc_two_inputs_raw", ""), failures)
    check("btc_two_inputs_fee", "15000", fixtures.get("btc_two_inputs_fee", ""), failures)

    raw, _, _ = btc_single()
    check("btc_single_raw", raw, fixtures.get("btc_single_raw", ""), failures)
    check("btc_single_fee", "10000", fixtures.get("btc_single_fee", ""), failures)

    print("Solana fixtures (solders)")
    message_b64, signature_b64, transaction_b64 = solana_transfer()
    check("sol_message_base64", message_b64, fixtures.get("sol_message_base64", ""), failures)
    check(
        "sol_signature_base64",
        signature_b64,
        fixtures.get("sol_signature_base64", ""),
        failures,
    )
    check(
        "sol_transaction_base64",
        transaction_b64,
        fixtures.get("sol_transaction_base64", ""),
        failures,
    )

    if failures:
        print(f"\n{len(failures)} mismatches: {', '.join(failures)}")
        return 1
    print("\nall fixtures match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
