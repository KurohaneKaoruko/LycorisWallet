#!/usr/bin/env python3
"""Independently verify the legacy P2PKH transaction builder.

Re-derives the raw transactions emitted by
`cargo run -p lycoris-core --example tx_fixtures` with coincurve (libsecp256k1)
and plain hashlib, using the classic pre-BIP-143 digest (Dogecoin) and the
BIP-143 + SIGHASH_FORKID digest (Bitcoin Cash), then compares the bytes.

Usage:
    python tools/verify_tx_legacy.py [fixtures.txt]
"""

import hashlib
import re
import subprocess
import sys
from pathlib import Path

import coincurve

SEQUENCE = 0xFFFFFFFD
LOCKTIME = 0
SIGHASH_ALL = 1
SIGHASH_FORKID_ALL = 0x41


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


def p2pkh_code(pubkey: bytes) -> bytes:
    return bytes([0x76, 0xA9, 0x14]) + hash160(pubkey) + bytes([0x88, 0xAC])


def serialize_outputs(outputs):
    out = varint(len(outputs))
    for script, value in outputs:
        out += value.to_bytes(8, "little") + varint(len(script)) + script
    return out


def classic_sighash(inputs, outputs, code, index, sighash_type):
    preimage = (1).to_bytes(4, "little")
    preimage += varint(len(inputs))
    for position, (txid, vout, _value) in enumerate(inputs):
        preimage += txid[::-1] + vout.to_bytes(4, "little")
        if position == index:
            preimage += varint(len(code)) + code
        else:
            preimage += varint(0)
        preimage += SEQUENCE.to_bytes(4, "little")
    preimage += serialize_outputs(outputs)
    preimage += LOCKTIME.to_bytes(4, "little")
    preimage += sighash_type.to_bytes(4, "little")
    return double_sha256(preimage)


def forkid_sighash(inputs, outputs, code, index, sighash_type):
    prevouts = b"".join(txid[::-1] + vout.to_bytes(4, "little") for txid, vout, _ in inputs)
    sequences = SEQUENCE.to_bytes(4, "little") * len(inputs)
    hash_prevouts = double_sha256(prevouts)
    hash_sequence = double_sha256(sequences)
    hash_outputs = double_sha256(serialize_outputs(outputs))
    txid, vout, value = inputs[index]
    preimage = (1).to_bytes(4, "little")
    preimage += hash_prevouts + hash_sequence + hash_outputs
    preimage += txid[::-1] + vout.to_bytes(4, "little")
    preimage += varint(len(code)) + code
    preimage += value.to_bytes(8, "little")
    preimage += SEQUENCE.to_bytes(4, "little")
    preimage += hash_outputs
    preimage += LOCKTIME.to_bytes(4, "little")
    preimage += sighash_type.to_bytes(4, "little")
    return double_sha256(preimage)


def build_legacy(inputs, recipient_script, recipient_value, change, forkid: bool, dust_limit: int):
    key = coincurve.PrivateKey(bytes([0x11] * 32))
    pubkey = key.public_key.format(compressed=True)
    code = p2pkh_code(pubkey)
    outputs = [(recipient_script, recipient_value)]
    if change is not None:
        outputs.append(change)

    # A change output below the chain's dust limit can never be spent again,
    # so it is absorbed into the fee instead.
    if change is not None:
        leftover = change[1] - 0
        total_in = sum(value for _, _, value in inputs)
        fee = total_in - recipient_value - leftover
        if leftover < dust_limit:
            outputs = [(recipient_script, recipient_value)]

    sighash_type = SIGHASH_FORKID_ALL if forkid else SIGHASH_ALL
    script_sigs = []
    for index in range(len(inputs)):
        if forkid:
            digest = forkid_sighash(inputs, outputs, code, index, sighash_type)
        else:
            digest = classic_sighash(inputs, outputs, code, index, sighash_type)
        sig = key.sign(digest, hasher=None)  # RFC 6979, low-S
        assert key.public_key.verify(sig, digest, hasher=None)
        with_type = sig + bytes([sighash_type])
        script_sigs.append(
            varint(len(with_type)) + with_type + varint(len(pubkey)) + pubkey
        )

    raw = (1).to_bytes(4, "little")
    raw += varint(len(inputs))
    for index, (txid, vout, _value) in enumerate(inputs):
        raw += txid[::-1] + vout.to_bytes(4, "little")
        raw += varint(len(script_sigs[index])) + script_sigs[index]
        raw += SEQUENCE.to_bytes(4, "little")
    raw += serialize_outputs(outputs)
    raw += LOCKTIME.to_bytes(4, "little")
    return raw.hex(), pubkey.hex()


# Dogecoin relays nothing below one DOGE, Bitcoin Cash below 546 satoshis.
DOGE_DUST = 100_000_000
BCH_DUST = 546


def doge_single():
    key_pub = coincurve.PrivateKey(bytes([0x11] * 32)).public_key.format(compressed=True)
    script = p2pkh_code(key_pub)
    inputs = [(bytes.fromhex("ab" * 32), 0, 100_000)]
    return build_legacy(inputs, script, 90_000, None, forkid=False, dust_limit=DOGE_DUST)


def doge_two_inputs():
    key_pub = coincurve.PrivateKey(bytes([0x11] * 32)).public_key.format(compressed=True)
    script = p2pkh_code(key_pub)
    inputs = [
        (bytes.fromhex("ab" * 32), 0, 100_000),
        (bytes.fromhex("cd" * 32), 1, 60_000),
    ]
    return build_legacy(
        inputs, script, 120_000, (script, 25_000), forkid=False, dust_limit=DOGE_DUST
    )


def bch_single():
    key_pub = coincurve.PrivateKey(bytes([0x11] * 32)).public_key.format(compressed=True)
    script = p2pkh_code(key_pub)
    inputs = [(bytes.fromhex("ab" * 32), 0, 100_000)]
    return build_legacy(inputs, script, 90_000, None, forkid=True, dust_limit=BCH_DUST)


def bch_two_inputs():
    key_pub = coincurve.PrivateKey(bytes([0x11] * 32)).public_key.format(compressed=True)
    script = p2pkh_code(key_pub)
    inputs = [
        (bytes.fromhex("ab" * 32), 0, 100_000),
        (bytes.fromhex("cd" * 32), 1, 60_000),
    ]
    return build_legacy(
        inputs, script, 120_000, (script, 25_000), forkid=True, dust_limit=BCH_DUST
    )


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
        text = Path(sys.argv[1]).read_text(encoding="utf-8")
    else:
        result = subprocess.run(
            [
                "cargo",
                "run",
                "-q",
                "-p",
                "lycoris-core",
                "--example",
                "tx_fixtures",
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        text = result.stdout
    fixtures = parse_fixtures(text)
    failures = []

    print("Dogecoin (classic pre-BIP143 sighash, 1 DOGE dust limit)")
    raw, pubkey = doge_single()
    check("doge_pubkey", pubkey, fixtures.get("doge_pubkey", ""), failures)
    check("doge_single_raw", raw, fixtures.get("doge_single_raw", ""), failures)
    check("doge_single_fee", "10000", fixtures.get("doge_single_fee", ""), failures)

    raw, _ = doge_two_inputs()
    check("doge_two_inputs_raw", raw, fixtures.get("doge_two_inputs_raw", ""), failures)
    # The 25_000 satoshi change is below the Dogecoin dust limit, so it is
    # absorbed into the fee instead of becoming an output.
    check("doge_two_inputs_fee", "40000", fixtures.get("doge_two_inputs_fee", ""), failures)

    print("Bitcoin Cash (BIP143 + SIGHASH_FORKID)")
    raw, _ = bch_single()
    check("bch_single_raw", raw, fixtures.get("bch_single_raw", ""), failures)
    check("bch_single_fee", "10000", fixtures.get("bch_single_fee", ""), failures)

    raw, _ = bch_two_inputs()
    check("bch_two_inputs_raw", raw, fixtures.get("bch_two_inputs_raw", ""), failures)
    check("bch_two_inputs_fee", "15000", fixtures.get("bch_two_inputs_fee", ""), failures)

    if failures:
        print(f"\n{len(failures)} mismatches: {', '.join(failures)}")
        return 1
    print("\nall legacy fixtures match")
    return 0


if __name__ == "__main__":
    sys.exit(main())
