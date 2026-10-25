/**
 * Real Rust core, compiled to WebAssembly.
 *
 * The demo build (`?mock=1`) has no Tauri backend, so it used to invent
 * addresses and QR codes. This module loads the very same `lycoris-core` the
 * desktop app uses, which means the demo shows real BIP-39 / BIP-32 / SLIP-0010
 * derivation, real Keccak/scrypt brain-wallet addresses, the real Monero
 * base58 encoding, real address validation and the real rounded QR renderer.
 *
 * The wasm is a plain `cdylib` (no wasm-bindgen shim):
 *
 *   1. copy the UTF-8 request into `lycoris_alloc(len)`;
 *   2. `lycoris_call(op, ptr, len)` returns `[u32 LE length][JSON]`;
 *   3. read the bytes, then `lycoris_result_free(ptr)`.
 *
 * A handful of inert wasm-bindgen intrinsics are imported because `k256`
 * pulls `rand_core`, whose `std` feature enables `getrandom`. The demo never
 * draws entropy or opens a vault, so they are stubbed with no-ops and the
 * module can be instantiated without a JavaScript shim.
 */

const OPS = {
  preview: 1,
  qrSvg: 2,
  validateAddress: 3,
  privateKey: 4,
};

const CORE_URL = new URL("./vendor/lycoris-core.wasm", import.meta.url);

let pending = null;

function inertImports(module) {
  const imports = {};
  for (const entry of WebAssembly.Module.imports(module)) {
    imports[entry.module] ??= {};
    if (entry.kind === "function") {
      imports[entry.module][entry.name] = () => {
        if (entry.name.includes("throw")) {
          throw new Error("lycoris-core: wasm runtime trap");
        }
        return 0;
      };
      continue;
    }
    throw new Error(
      `lycoris-core: unexpected ${entry.kind} import ${entry.module}.${entry.name}`,
    );
  }
  return imports;
}

function read(instance, pointer) {
  if (!pointer) throw new Error("lycoris-core: out of memory");
  const buffer = instance.exports.memory.buffer;
  const length = new DataView(buffer).getUint32(pointer, true);
  const bytes = new Uint8Array(buffer, pointer + 4, length).slice();
  instance.exports.lycoris_result_free(pointer);
  return JSON.parse(new TextDecoder().decode(bytes));
}

function invoke(instance, op, payload) {
  const bytes = new TextEncoder().encode(JSON.stringify(payload ?? {}));
  const input = instance.exports.lycoris_alloc(bytes.length);
  if (!input) throw new Error("lycoris-core: out of memory");
  new Uint8Array(instance.exports.memory.buffer, input, bytes.length).set(bytes);
  const output = instance.exports.lycoris_call(op, input, bytes.length);
  instance.exports.lycoris_free(input, bytes.length);
  const value = read(instance, output);
  if (value && typeof value === "object" && value.error) {
    const error = new Error(value.error.message ?? "core error");
    error.code = value.error.code ?? "ERROR";
    throw error;
  }
  return value;
}

/**
 * Load the core once. Resolves to the same object for every caller.
 * @returns {Promise<{
 *   preview: (secret: object) => object,
 *   qrSvg: (text: string, dark?: string, light?: string) => string,
 *   validateAddress: (chain: string, address: string) => boolean,
 *   privateKey: (secret: object, chain: string) => string,
 * }>}
 */
export function loadCore() {
  if (!pending) {
    pending = (async () => {
      // `no-cache` revalidates with the server (304 when unchanged), so a
      // rebuilt core is picked up without re-downloading 1.2 MB every load.
      const response = await fetch(CORE_URL, { cache: "no-cache" });
      if (!response.ok) {
        throw new Error(`lycoris-core.wasm: HTTP ${response.status}`);
      }
      const module = await WebAssembly.compile(await response.arrayBuffer());
      const instance = await WebAssembly.instantiate(module, inertImports(module));
      return {
        preview: (secret) => invoke(instance, OPS.preview, secret),
        qrSvg: (text, dark, light) => invoke(instance, OPS.qrSvg, { text, dark, light }),
        validateAddress: (chain, address) =>
          invoke(instance, OPS.validateAddress, { chain, address }),
        privateKey: (secret, chain) => invoke(instance, OPS.privateKey, { secret, chain }),
      };
    })();
  }
  return pending;
}
