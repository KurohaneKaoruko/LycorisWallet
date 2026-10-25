#!/usr/bin/env node
/**
 * Verify the WebAssembly bridge the demo build uses.
 *
 * The demo (`ui-react/?mock=1`) is not a mock any more: it loads `lycoris-core`
 * compiled to wasm, so addresses, fingerprints, address validation and QR codes
 * are produced by exactly the same Rust code as the desktop app. This script
 * pins that contract:
 *
 *   - BIP-39/BIP-32 derivation must match the addresses the Rust tests pin;
 *   - brain-wallet derivation must match the Rust Monero vector;
 *   - address validation must accept good addresses and reject tampered ones;
 *   - the QR renderer output must decode with zbar (written to
 *     `out/mock-qr-fixtures.txt` for `tools/verify_qr_svg.py`).
 *
 * Usage: node tools/verify_core_wasm.mjs
 */

import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const wasmPath = path.join(root, "ui-react", "src", "mock", "vendor", "lycoris-core.wasm");
const outDir = path.join(root, "out");

const OP_PREVIEW = 1;
const OP_QR_SVG = 2;
const OP_VALIDATE = 3;

function inertImports(module) {
  const imports = {};
  for (const entry of WebAssembly.Module.imports(module)) {
    imports[entry.module] ??= {};
    if (entry.kind === "function") {
      imports[entry.module][entry.name] = () => 0;
      continue;
    }
    throw new Error(`unexpected ${entry.kind} import ${entry.module}.${entry.name}`);
  }
  return imports;
}

const module = await WebAssembly.compile(fs.readFileSync(wasmPath));
const instance = await WebAssembly.instantiate(module, inertImports(module));
const exports = instance.exports;

function call(op, payload) {
  const bytes = new TextEncoder().encode(JSON.stringify(payload ?? {}));
  const input = exports.lycoris_alloc(bytes.length);
  new Uint8Array(exports.memory.buffer, input, bytes.length).set(bytes);
  const output = exports.lycoris_call(op, input, bytes.length);
  exports.lycoris_free(input, bytes.length);
  const view = new DataView(exports.memory.buffer);
  const length = view.getUint32(output, true);
  const text = new TextDecoder().decode(
    new Uint8Array(exports.memory.buffer, output + 4, length),
  );
  exports.lycoris_result_free(output);
  return JSON.parse(text);
}

let failures = 0;

function expect(label, actual, expected) {
  if (actual === expected) {
    console.log(`  ok   ${label}`);
    return;
  }
  failures += 1;
  console.log(`  FAIL ${label}\n       got      ${actual}\n       expected ${expected}`);
}

const mnemonic =
  "legal winner thank year wave sausage worth useful legal winner thank yellow";
const bip39 = call(OP_PREVIEW, { mode: "mnemonic", mnemonic });
const accounts = Object.fromEntries(bip39.accounts.map((account) => [account.chain, account]));

console.log(`wasm core: ${(fs.statSync(wasmPath).size / 1024).toFixed(0)} KB`);
console.log(`fingerprint ${bip39.fingerprint}`);

const vectors = {
  ethereum: "0x58A57ed9d8d624cBD12e2C467D34787555bB1b25",
  "ethereum-classic": "0x80857C0c11b30003Cf74F99Ed4D68DA41C7224aa",
  dogecoin: "DMM78DyiwEPzWmykiQQUFcPcUv13YPe2ZK",
  bitcoincash: "bitcoincash:qpdtccrxx78kcuc65mceurfwg60gmmqu9cwpjdt25n",
};
for (const [chain, expected] of Object.entries(vectors)) {
  expect(`${chain} address`, accounts[chain]?.address, expected);
}
// All EVM chains share one key in mnemonic mode (same BIP-44 coin type).
expect("bsc shares ethereum", accounts.bsc?.address, vectors.ethereum);
expect("polygon shares ethereum", accounts.polygon?.address, vectors.ethereum);

const brain = call(OP_PREVIEW, {
  mode: "passphrase",
  phrase: "a quiet garden at dusk",
  derivation: "isolated",
});
const monero = brain.accounts.find((account) => account.chain === "monero");
expect(
  "monero brain-wallet address",
  monero?.address,
  "48zQvBdmMxj9ByEJWD3uPAVdSKJEnJEsMS8ppnVdJXuh5GjcU9wRcuBck7NTZidFd44bFJmpumZRRi5yufFhd3dP7mHe5Eb",
);

const valid = [
  ["bitcoin", accounts.bitcoin.address],
  ["dogecoin", accounts.dogecoin.address],
  ["bitcoincash", accounts.bitcoincash.address],
  ["monero", monero.address],
];
for (const [chain, address] of valid) {
  expect(`${chain} validates`, call(OP_VALIDATE, { chain, address }), true);
}
const tampered = accounts.bitcoin.address.slice(0, -1) + (accounts.bitcoin.address.endsWith("t") ? "q" : "t");
expect("tampered bitcoin rejected", call(OP_VALIDATE, { chain: "bitcoin", address: tampered }), false);
expect(
  "ethereum address rejected on bitcoin",
  call(OP_VALIDATE, { chain: "bitcoin", address: "0x58A57ed9d8c624cBD12e2C467D34787555bB1b25" }),
  false,
);

// Fixtures for the independent zbar pass. The app renders a light symbol in
// both themes, so check exactly that palette.
const fixtures = [
  ["mock_monero", monero.address],
  ["mock_bitcoin", accounts.bitcoin.address],
  ["mock_dogecoin", accounts.dogecoin.address],
];
const lines = [];
for (const [name, payload] of fixtures) {
  const svg = call(OP_QR_SVG, { text: payload, dark: "#1b1a19", light: "#f7f1e8" });
  if (!svg.startsWith("<?xml")) {
    failures += 1;
    console.log(`  FAIL ${name} QR did not render`);
    continue;
  }
  lines.push(`qrsvg_${name}=${payload}|${svg}`);
  console.log(`  ok   ${name} QR rendered (${svg.length} bytes)`);
}
fs.mkdirSync(outDir, { recursive: true });
fs.writeFileSync(path.join(outDir, "mock-qr-fixtures.txt"), lines.join("\n"), "utf8");
console.log(`wrote out/mock-qr-fixtures.txt`);

if (failures > 0) {
  console.log(`\n${failures} failure(s)`);
  process.exit(1);
}
console.log("\nwasm core matches the Rust vectors");
