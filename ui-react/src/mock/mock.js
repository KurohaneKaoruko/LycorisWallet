/** Demo backend for the static build.
 *
 * Used when the interface runs in a plain browser (`?mock=1` or no Tauri
 * host). The command surface, formats and error codes mirror the Rust backend,
 * and whenever the WebAssembly core is available the addresses, fingerprints,
 * address validation and QR codes are **real**: they come from the same
 * `lycoris-core` the desktop app uses. Only balances, prices and broadcast
 * results are demo data — the UI shows a 演示模式 badge so nobody mistakes the
 * balances for a funded wallet. */

import { BIP39_WORDS } from "./bip39-words.js?v=23";
import { loadCore } from "./core.js?v=23";

// The desktop build has a Rust backend already; only the browser demo needs
// the wasm copy (1.2 MB, skipped entirely when Tauri is present).
const wantsCore =
  new URLSearchParams(location.search).has("mock") || !globalThis.__TAURI__;
let core = null;
if (wantsCore) {
  try {
    core = await loadCore();
  } catch (error) {
    console.warn("[lycoris] wasm core unavailable, using demo addresses", error);
  }
}

/** Derivation is scrypt-heavy; cache per secret so a refresh does not re-derive. */
const PREVIEW_CACHE = new Map();

function corePreview(secret) {
  if (!core) return null;
  const key = JSON.stringify(secret);
  const cached = PREVIEW_CACHE.get(key);
  if (cached) return cached;
  const preview = core.preview(secret);
  if (PREVIEW_CACHE.size >= 24) PREVIEW_CACHE.clear();
  PREVIEW_CACHE.set(key, preview);
  return preview;
}

/** Smallest-unit integer → trimmed decimal string, like Rust's format_amount. */
function trimDecimal(units, decimals) {
  const negative = units.startsWith("-");
  const digits = negative ? units.slice(1) : units;
  const padded = digits.padStart(decimals + 1, "0");
  const whole = padded.slice(0, padded.length - decimals);
  let frac = decimals > 0 ? padded.slice(padded.length - decimals) : "";
  frac = frac.replace(/0+$/, "");
  const text = frac ? `${whole}.${frac}` : whole;
  return negative ? `-${text}` : text;
}

const CHAINS = [
  { id: "bitcoin", name: "Bitcoin", symbol: "BTC", decimals: 8, accent: "#F7931A", kind: "utxo" },
  { id: "litecoin", name: "Litecoin", symbol: "LTC", decimals: 8, accent: "#345D9D", kind: "utxo" },
  { id: "dogecoin", name: "Dogecoin", symbol: "DOGE", decimals: 8, accent: "#C2A633", kind: "utxo" },
  { id: "bitcoincash", name: "Bitcoin Cash", symbol: "BCH", decimals: 8, accent: "#8DC351", kind: "utxo" },
  { id: "ethereum", name: "Ethereum", symbol: "ETH", decimals: 18, accent: "#627EEA", kind: "evm" },
  { id: "ethereum-classic", name: "Ethereum Classic", symbol: "ETC", decimals: 18, accent: "#3AB83A", kind: "evm" },
  { id: "bsc", name: "BNB Chain", symbol: "BNB", decimals: 18, accent: "#F3BA2F", kind: "evm" },
  { id: "polygon", name: "Polygon", symbol: "POL", decimals: 18, accent: "#8247E5", kind: "evm" },
  { id: "solana", name: "Solana", symbol: "SOL", decimals: 9, accent: "#14F195", kind: "ed25519" },
  { id: "monero", name: "Monero", symbol: "XMR", decimals: 12, accent: "#FF6600", kind: "cryptonote" },
  { id: "bitcoinsv", name: "Bitcoin SV", symbol: "BSV", decimals: 8, accent: "#EAB300", kind: "utxo" },
  { id: "namecoin", name: "Namecoin", symbol: "NMC", decimals: 8, accent: "#186C9D", kind: "utxo" },
  { id: "litecoincash", name: "Litecoin Cash", symbol: "LCC", decimals: 8, accent: "#BEBEBE", kind: "utxo" },
  { id: "feathercoin", name: "Feathercoin", symbol: "FTC", decimals: 8, accent: "#C6E4EE", kind: "utxo" },
  { id: "viacoin", name: "Viacoin", symbol: "VIA", decimals: 8, accent: "#404E82", kind: "utxo" },
  { id: "potcoin", name: "Potcoin", symbol: "POT", decimals: 8, accent: "#3F8C48", kind: "utxo" },
  { id: "egulden", name: "e-Gulden", symbol: "EFL", decimals: 8, accent: "#F4812F", kind: "utxo" },
  { id: "dash", name: "Dash", symbol: "DASH", decimals: 8, accent: "#008DE4", kind: "utxo" },
  { id: "groestlcoin", name: "Groestlcoin", symbol: "GRS", decimals: 8, accent: "#36855A", kind: "utxo" },
  { id: "vertcoin", name: "Vertcoin", symbol: "VTC", decimals: 8, accent: "#5BB5A0", kind: "utxo" },
  { id: "ravencoin", name: "Ravencoin", symbol: "RVN", decimals: 8, accent: "#B7B7B7", kind: "utxo" },
  { id: "monacoin", name: "Monacoin", symbol: "MONA", decimals: 8, accent: "#B78C33", kind: "utxo" },
  { id: "fujicoin", name: "Fujicoin", symbol: "FJC", decimals: 8, accent: "#B7472A", kind: "utxo" },
];

const PRICES = {
  bitcoin: { usd: 96320.5, cny: 692450.0 },
  litecoin: { usd: 118.42, cny: 851.3 },
  dogecoin: { usd: 0.1284, cny: 0.923 },
  bitcoincash: { usd: 486.7, cny: 3500.2 },
  ethereum: { usd: 3182.7, cny: 22880.4 },
  "ethereum-classic": { usd: 24.18, cny: 173.8 },
  bsc: { usd: 642.18, cny: 4616.9 },
  polygon: { usd: 0.5124, cny: 3.683 },
  solana: { usd: 187.63, cny: 1348.9 },
  monero: { usd: 176.4, cny: 1269.2 },
  bitcoinsv: { usd: 62.4, cny: 448.9 },
  namecoin: { usd: 0.918, cny: 6.6 },
  litecoincash: { usd: 0.0083, cny: 0.0597 },
  feathercoin: { usd: 0.0156, cny: 0.112 },
  viacoin: { usd: 0.0671, cny: 0.483 },
  potcoin: { usd: 0.00152, cny: 0.0109 },
  egulden: { usd: 0.0241, cny: 0.173 },
  dash: { usd: 31.86, cny: 229.0 },
  groestlcoin: { usd: 0.382, cny: 2.75 },
  vertcoin: { usd: 0.0492, cny: 0.354 },
  ravencoin: { usd: 0.0217, cny: 0.156 },
  monacoin: { usd: 0.384, cny: 2.76 },
  fujicoin: { usd: 0.0129, cny: 0.0928 },
};

const BALANCES = {
  bitcoin: 42_186_400n,
  litecoin: 152_300_000n,
  dogecoin: 88_400_000_000n,
  bitcoincash: 1_260_000_000n,
  ethereum: 512_400_000_000_000_000n,
  "ethereum-classic": 96_200_000_000_000_000_000n,
  bsc: 1_240_000_000_000_000_000n,
  polygon: 486_000_000_000_000_000_000n,
  solana: 8_420_000_000n,
  monero: 3_420_000_000_000n,
  bitcoinsv: 2_418_000_000n,
  namecoin: 8_640_000_000n,
  litecoincash: 126_400_000_000n,
  feathercoin: 246_800_000_000n,
  viacoin: 4_120_000_000n,
  potcoin: 968_200_000_000n,
  egulden: 74_100_000_000n,
  dash: 5_320_000_000n,
  groestlcoin: 68_400_000_000n,
  vertcoin: 31_260_000_000n,
  ravencoin: 1_842_600_000_000n,
  monacoin: 7_640_000_000n,
  fujicoin: 124_800_000_000n,
};

const ALGOS = {
  bitcoin: ["SHA-256", "FIPS 180-4 · 32 字节摘要"],
  litecoin: ["scrypt", "N=16384, r=8, p=1 · 32 字节摘要"],
  dogecoin: ["scrypt", "N=16384, r=8, p=1 · 32 字节摘要"],
  bitcoincash: ["SHA-256", "FIPS 180-4 · 32 字节摘要"],
  ethereum: ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  "ethereum-classic": ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  bsc: ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  polygon: ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  solana: ["SHA-256", "FIPS 180-4 · 32 字节摘要"],
  monero: ["Keccak-256", "CryptoNote 原生哈希 · 32 字节摘要"],
  bitcoinsv: ["SHA-256", "FIPS 180-4 · 32 字节摘要"],
  namecoin: ["SHA-256", "FIPS 180-4 · 32 字节摘要"],
  litecoincash: ["SHA-256", "FIPS 180-4 · 32 字节摘要"],
  feathercoin: ["scrypt", "N=16384, r=8, p=1 · 32 字节摘要"],
  viacoin: ["scrypt", "N=16384, r=8, p=1 · 32 字节摘要"],
  potcoin: ["scrypt", "N=16384, r=8, p=1 · 32 字节摘要"],
  egulden: ["scrypt", "N=16384, r=8, p=1 · 32 字节摘要"],
  dash: ["X11", "Dash PoW 哈希 · 32 字节摘要"],
  groestlcoin: ["Groestl", "Groestl-512 → 前 32 字节"],
  vertcoin: ["Lyra2RE", "Lyra2RE(v2) 哈希链"],
  ravencoin: ["KawPow", "Ravencoin PoW 哈希 · progpow 变体"],
  monacoin: ["Lyra2RE", "Lyra2RE(v2) 哈希链"],
  fujicoin: ["Lyra2RE", "Lyra2RE(v2) 哈希链"],
};

const PATHS = {
  bitcoin: "m/84'/0'/0'/0/0",
  litecoin: "m/84'/2'/0'/0/0",
  dogecoin: "m/44'/3'/0'/0/0",
  bitcoincash: "m/44'/145'/0'/0/0",
  ethereum: "m/44'/60'/0'/0/0",
  "ethereum-classic": "m/44'/61'/0'/0/0",
  bsc: "m/44'/60'/0'/0/0",
  polygon: "m/44'/60'/0'/0/0",
  solana: "m/44'/501'/0'/0'",
  monero: "m/44'/128'/0'/0/0",
  bitcoinsv: "m/44'/236'/0'/0/0",
  namecoin: "m/44'/7'/0'/0/0",
  litecoincash: "m/44'/192'/0'/0/0",
  feathercoin: "m/44'/8'/0'/0/0",
  viacoin: "m/44'/14'/0'/0/0",
  potcoin: "m/44'/81'/0'/0/0",
  egulden: "m/44'/78'/0'/0/0",
  dash: "m/44'/5'/0'/0/0",
  groestlcoin: "m/44'/17'/0'/0/0",
  vertcoin: "m/44'/28'/0'/0/0",
  ravencoin: "m/44'/175'/0'/0/0",
  monacoin: "m/84'/22'/0'/0/0",
  fujicoin: "m/84'/75'/0'/0/0",
};

/* BIP-44 coin types: chains with the same coin type share one key in
   mnemonic mode (Ethereum / BNB Chain / Polygon all use 60). */
const COIN_TYPES = {
  bitcoin: 0,
  litecoin: 2,
  dogecoin: 3,
  bitcoincash: 145,
  ethereum: 60,
  "ethereum-classic": 61,
  bsc: 60,
  polygon: 60,
  solana: 501,
  monero: 128,
  bitcoinsv: 236,
  namecoin: 7,
  litecoincash: 192,
  feathercoin: 8,
  viacoin: 14,
  potcoin: 81,
  egulden: 78,
  dash: 5,
  groestlcoin: 17,
  vertcoin: 28,
  ravencoin: 175,
  monacoin: 22,
  fujicoin: 75,
};

const TX_EXPLORERS = {  bitcoin: "https://mempool.space/tx/",
  litecoin: "https://litecoinspace.org/tx/",
  dogecoin: "https://doge.trezor.io/tx/",
  bitcoincash: "https://bch.trezor.io/tx/",
  ethereum: "https://etherscan.io/tx/",
  "ethereum-classic": "https://blockscout.com/etc/mainnet/tx/",
  bsc: "https://bscscan.com/tx/",
  polygon: "https://polygonscan.com/tx/",
  solana: "https://solscan.io/tx/",
  monero: "https://xmrchain.net/tx/",
  bitcoinsv: "https://whatsonchain.com/tx/",
  namecoin: "https://nmc.blockbook.network/tx/",
  litecoincash: "https://explorer.litecoinca.sh/tx/",
  feathercoin: "https://ftc.blockbook.network/tx/",
  viacoin: "https://via.blockbook.network/tx/",
  potcoin: "https://chainz.cryptoid.info/pot/tx.dws?",
  egulden: "https://efl.blockbook.network/tx/",
  dash: "https://chainz.cryptoid.info/dash/tx.dws?",
  groestlcoin: "https://grs.blockbook.network/tx/",
  vertcoin: "https://vtc.blockbook.network/tx/",
  ravencoin: "https://blockbook.ravencoin.org/tx/",
  monacoin: "https://blockbook.electrum-mona.org/tx/",
  fujicoin: "https://explorer.fujicoin.org/tx/",
};

/* ------------------------------------------------------------------- random */

function fnv1a(text) {
  let hash = 0x811c9dc5;
  for (let index = 0; index < text.length; index += 1) {
    hash ^= text.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return hash >>> 0;
}

function mulberry32(seed) {
  let value = seed >>> 0;
  return () => {
    value = (value + 0x6d2b79f5) >>> 0;
    let t = value;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

function pick(rand, alphabet, length) {
  let out = "";
  for (let index = 0; index < length; index += 1) {
    out += alphabet[Math.floor(rand() * alphabet.length)];
  }
  return out;
}

const BASE58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
const BASE32 = "qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const HEX = "0123456789abcdef";

/**
 * Stream that stands for the chain's private key.
 *
 * - Mnemonic: the chain's BIP-44 coin type decides the key, so Ethereum /
 *   BNB Chain / Polygon share one address while Ethereum Classic (61) does not.
 * - Isolated: the chain name is mixed into the hashed message, so every chain
 *   gets its own key.
 * - Strict: the phrase is hashed alone, so chains sharing a native hash (all
 *   the Keccak-256 EVM chains) share one key — and therefore one address,
 *   exactly like the Rust implementation does.
 */
function keyStream(chain, seedText, mode) {
  if (mode === "mnemonic") {
    return mulberry32(fnv1a(`M:${COIN_TYPES[chain] ?? chain}:${seedText}`));
  }
  if (mode === "strict") {
    const algo = ALGOS[chain]?.[0] ?? chain;
    return mulberry32(fnv1a(`H:${algo}:${seedText}`));
  }
  return mulberry32(fnv1a(`C:${chain}:${seedText}`));
}

function fakeAddress(chain, seedText, mode) {
  const rand = keyStream(chain, seedText, mode);
  switch (chain) {
    case "bitcoin":
      return `bc1q${pick(rand, BASE32, 38)}`;
    case "litecoin":
      return `ltc1q${pick(rand, BASE32, 38)}`;
    case "dogecoin":
      return `D${pick(rand, BASE58, 33)}`;
    case "bitcoincash":
      return `bitcoincash:q${pick(rand, BASE32, 34)}`;
    case "solana":
      return pick(rand, BASE58, 44);
    case "monero":
      // 95-char primary address (4/8 prefix), structure-only fake.
      return `4${pick(rand, BASE58, 94)}`;
    // 2024 追加的十条链:bech32 三条 + P2PKH 七条(结构演示,校验和随机)。
    case "viacoin":
      return `via1q${pick(rand, BASE32, 38)}`;
    case "groestlcoin":
      return `grs1q${pick(rand, BASE32, 38)}`;
    case "vertcoin":
      return `vtc1q${pick(rand, BASE32, 38)}`;
    case "bitcoinsv":
      return `1${pick(rand, BASE58, 33)}`;
    case "namecoin":
      return `N${pick(rand, BASE58, 33)}`;
    case "litecoincash":
      return `C${pick(rand, BASE58, 33)}`;
    case "feathercoin":
      return `6${pick(rand, BASE58, 33)}`;
    case "potcoin":
      return `P${pick(rand, BASE58, 33)}`;
    case "egulden":
      return `L${pick(rand, BASE58, 33)}`;
    case "dash":
      return `X${pick(rand, BASE58, 33)}`;
    case "ravencoin":
      return `R${pick(rand, BASE58, 33)}`;
    case "monacoin":
      return `mona1q${pick(rand, BASE32, 38)}`;
    case "fujicoin":
      return `fc1q${pick(rand, BASE32, 38)}`;
    default:
      return `0x${pick(rand, HEX, 40)}`;
  }
}

function fingerprintOf(accounts) {
  const source = accounts.map((account) => account.address).join("|");
  const rand = mulberry32(fnv1a(source));
  const alphabet = "0123456789ABCDEF";
  const parts = [];
  for (let group = 0; group < 4; group += 1) {
    parts.push(pick(rand, alphabet, 4));
  }
  return parts.join("-");
}

function unitsToText(value, decimals) {
  const negative = value < 0n;
  const abs = negative ? -value : value;
  const scale = 10n ** BigInt(decimals);
  const integer = abs / scale;
  let fraction = (abs % scale).toString().padStart(decimals, "0");
  fraction = fraction.replace(/0+$/, "");
  return `${negative ? "-" : ""}${integer}${fraction ? `.${fraction}` : ""}`;
}

function parseUnits(text, decimals) {
  const value = String(text).trim();
  if (!/^\d*(\.\d*)?$/.test(value) || value === "" || value === ".") {
    throw { code: "INVALID_AMOUNT", message: "金额格式不正确" };
  }
  const [integer, fraction = ""] = value.split(".");
  if (fraction.length > decimals) {
    throw { code: "INVALID_AMOUNT", message: `最多支持 ${decimals} 位小数` };
  }
  const padded = fraction.padEnd(decimals, "0");
  return BigInt(integer || "0") * 10n ** BigInt(decimals) + BigInt(padded || "0");
}

/* -------------------------------------------------------------------- store */

function makeId() {
  return crypto.randomUUID
    ? crypto.randomUUID()
    : `w${Date.now()}${Math.floor(Math.random() * 1e6)}`;
}

function nowSeconds() {
  return Math.floor(Date.now() / 1000);
}

/* 演示模式的口令：任何状态都能用它解锁，避免测试数据把人锁在门外。 */
const DEMO_PASSWORD = "lycoris-demo";

const store = {
  version: 2,
  exists: false,
  locked: true,
  password: DEMO_PASSWORD,
  hint: `演示口令：${DEMO_PASSWORD}`,
  wallets: [],
  activeId: null,
  settings: {
    fiat: "usd",
    theme: "system",
    hideBalances: false,
    autoLockMinutes: 5,
    priceService: true,
    visibleChains: [],
    blockchairApiKey: "",
    moneroRpcUrl: "http://127.0.0.1:18083/json_rpc",
    moneroRpcUser: "",
    moneroRpcPassword: "",
    moneroRestoreHeight: 0,
  },
  contacts: [],
};

try {
  const saved = JSON.parse(localStorage.getItem("lycoris.mock.v1") ?? "null");
  if (saved) {
    Object.assign(store, saved);
    if (!Array.isArray(store.wallets) || store.wallets.length === 0) {
      // Migrate the single-wallet demo state written by earlier builds.
      if (saved.secret) {
        const id = makeId();
        store.wallets = [
          {
            id,
            label: saved.label ?? "演示钱包",
            secret: saved.secret,
            createdAt: saved.createdAt ?? nowSeconds(),
            activity: saved.activity ?? [],
          },
        ];
        store.activeId = id;
      } else {
        store.wallets = [];
        store.activeId = null;
      }
    }
    delete store.secret;
    delete store.label;
    delete store.activity;
    delete store.createdAt;
  }
  store.locked = true;
} catch {
  /* ignore malformed demo state */
}

function persist() {
  try {
    localStorage.setItem("lycoris.mock.v1", JSON.stringify({ ...store, locked: true }));
  } catch {
    /* storage may be unavailable */
  }
}

function activeWallet() {
  return store.wallets.find((wallet) => wallet.id === store.activeId) ?? store.wallets[0] ?? null;
}

function uniqueLabel(base) {
  const clean = String(base ?? "").trim() || "钱包";
  if (!store.wallets.some((wallet) => wallet.label === clean)) return clean;
  for (let index = 2; index < 1000; index += 1) {
    const candidate = `${clean} ${index}`;
    if (!store.wallets.some((wallet) => wallet.label === candidate)) return candidate;
  }
  return clean;
}

/* ----------------------------------------------------------------- accounts */

function accountsFor(secret) {
  // Prefer the real core: addresses, paths, hash descriptions and the
  // explorer links all come from the same code the desktop app runs.
  // The shipped wasm may predate the 2026 chain additions — its (real)
  // accounts are kept and the missing chains are filled in with demo
  // addresses so every chain stays visible in 演示模式.
  const preview = corePreview(secret);
  const seedText = secret.mode === "mnemonic" ? secret.mnemonic : `${secret.phrase}:${secret.derivation}`;
  const demo = (chain) => {
    const [algorithm, detail] = ALGOS[chain.id];
    return {
      chain: chain.id,
      name: chain.name,
      symbol: chain.symbol,
      address: fakeAddress(chain.id, seedText, secret.mode === "mnemonic" ? "mnemonic" : secret.derivation),
      path: secret.mode === "mnemonic" ? PATHS[chain.id] : "—",
      derivation:
        secret.mode === "mnemonic"
          ? "BIP-32 / SLIP-0010"
          : secret.derivation === "strict"
            ? "H(phrase)"
            : "H_chain(隔离)",
      hashAlgorithm: algorithm,
      hashDetail: detail,
      addressFormat:
        chain.id === "solana"
          ? "base58"
          : chain.id === "monero"
            ? "Monero base58"
            : chain.kind === "evm"
              ? "EIP-55"
              : chain.kind === "utxo" &&
                  ["viacoin", "groestlcoin", "vertcoin", "bitcoin", "litecoin", "monacoin", "fujicoin"].includes(chain.id)
                ? "bech32"
                : "base58",
      publicKey: `0x${pick(keyStream(chain.id, seedText, secret.mode === "mnemonic" ? "mnemonic" : secret.derivation), HEX, 64)}`,
      accent: chain.accent,
      explorerUrl: "#",
      sendSupported: true,
      decimals: chain.decimals,
    };
  };
  if (!preview) return CHAINS.map(demo);
  const known = new Set(preview.accounts.map((account) => account.chain));
  return [...preview.accounts, ...CHAINS.filter((c) => !known.has(c.id)).map(demo)];
}

function walletView() {
  const wallet = activeWallet();
  if (store.locked || !wallet) {
    throw { code: "LOCKED", message: "钱包已锁定" };
  }
  const selectedSpec = (wallet.batchAccounts ?? []).find(
    (spec) => spec.id === wallet.activeBatch,
  );
  // Demo mode cannot re-derive for real; perturb the seed so the derived
  // account shows a different address. The perturbation applies only to the
  // spec's own chain — other chains keep the base derivation.
  let accounts;
  if (selectedSpec) {
    const derived = perturbSecret(wallet.secret, selectedSpec.index);
    accounts = accountsFor(wallet.secret).map((account) =>
      account.chain === selectedSpec.chain
        ? accountsFor(derived).find((a) => a.chain === selectedSpec.chain)
        : account,
    );
  } else {
    accounts = accountsFor(wallet.secret);
  }
  const preview = corePreview(wallet.secret);
  const isMnemonic = wallet.secret.mode === "mnemonic";
  const activeFingerprint = selectedSpec
    ? fingerprintOf(accounts)
    : preview?.fingerprint ?? fingerprintOf(accounts);
  return {
    id: wallet.id,
    mode: wallet.secret.mode,
    derivation:
      wallet.secret.mode === "mnemonic" || wallet.secret.mode === "aggregate"
        ? null
        : wallet.secret.derivation ?? "isolated",
    label: wallet.label,
    fingerprint: activeFingerprint,
    createdAt: wallet.createdAt ?? nowSeconds(),
    formula: selectedSpec
      ? `批量派生 · ${selectedSpec.strategy} · #${selectedSpec.index}`
      : preview?.formula ??
      (isMnemonic
        ? "BIP-39 助记词 → 512 位种子 → BIP-32 / SLIP-0010 路径"
        : wallet.secret.mode === "private_key"
          ? "imported secp256k1 / ed25519 / monero keys → direct"
          : wallet.secret.mode === "aggregate"
            ? `聚合 ${wallet.secret.entries?.length ?? 0} 把独立私钥 · 一钥一址`
            : wallet.secret.derivation === "strict"
              ? "H_chain(phrase) → 32 字节 → 私钥"
              : 'H_chain("lycoris/v1/<chain>\\0" + phrase) → 32 字节 → 私钥'),
    keyRing:
      wallet.secret.mode === "private_key"
        ? {
            secp256k1: Boolean(wallet.secret.keys?.secp256k1),
            ed25519: Boolean(wallet.secret.keys?.ed25519),
            monero: Boolean(wallet.secret.keys?.monero),
          }
        : null,
    aggregateEntries:
      wallet.secret.mode === "aggregate"
        ? (wallet.secret.entries ?? []).map((entry) => ({
            id: entry.id,
            chain: entry.chain,
            label: entry.label ?? null,
          }))
        : null,
    accounts,
    settings: store.settings,
    contacts: store.contacts,
    activity: wallet.activity ?? [],
    wallets: store.wallets.map((item) => {
      const itemPreview = corePreview(item.secret);
      return {
        id: item.id,
        label: item.label,
        mode: item.secret.mode,
        derivation:
          item.secret.mode === "mnemonic" ? null : item.secret.derivation ?? "isolated",
        fingerprint:
          itemPreview?.fingerprint ?? fingerprintOf(accountsFor(item.secret)),
        createdAt: item.createdAt ?? nowSeconds(),
        activityCount: (item.activity ?? []).length,
        active: item.id === wallet.id,
      };
    }),
    batchAccounts: (wallet.batchAccounts ?? []).map((spec) => ({
      id: spec.id,
      strategy: spec.strategy,
      chain: spec.chain,
      index: spec.index,
      label: spec.label || `#${spec.index}`,
      fingerprint: (spec.fingerprint ?? "").toUpperCase(),
      active: spec.id === wallet.activeBatch,
    })),
    activeBatch: wallet.activeBatch ?? null,
  };
}

/** Demo-only: change the seed text so a derived account shows new addresses.
 *  For mnemonics the BIP-39 checksum must stay valid — the wasm core rejects
 *  invalid ones — so we flip one entropy bit derived from the index and
 *  re-encode through the wordlist with a freshly computed checksum. The SHA-256
 *  here is synchronous (compact implementation) because every call site is
 *  synchronous; 16/32-byte digests are trivial work. */
function sha256Bytes(data) {
  const K = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
  ];
  const bitLength = data.length * 8;
  const padded = new Uint8Array((((data.length + 8) >> 6) + 1) * 64);
  padded.set(data);
  padded[data.length] = 0x80;
  const view = new DataView(padded.buffer);
  view.setUint32(padded.length - 4, bitLength >>> 0);
  view.setUint32(padded.length - 8, Math.floor(bitLength / 0x100000000));
  const H = new Uint32Array([
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
  ]);
  const w = new Uint32Array(64);
  const rotr = (value, bits) => (value >>> bits) | (value << (32 - bits));
  for (let offset = 0; offset < padded.length; offset += 64) {
    for (let index = 0; index < 16; index += 1) {
      w[index] = view.getUint32(offset + index * 4);
    }
    for (let index = 16; index < 64; index += 1) {
      const s0 = rotr(w[index - 15], 7) ^ rotr(w[index - 15], 18) ^ (w[index - 15] >>> 3);
      const s1 = rotr(w[index - 2], 17) ^ rotr(w[index - 2], 19) ^ (w[index - 2] >>> 10);
      w[index] = (w[index - 16] + s0 + w[index - 7] + s1) >>> 0;
    }
    let [a, b, c, d, e, f, g, h] = H;
    for (let index = 0; index < 64; index += 1) {
      const S1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
      const ch = (e & f) ^ (~e & g);
      const temp1 = (h + S1 + ch + K[index] + w[index]) >>> 0;
      const S0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
      const maj = (a & b) ^ (a & c) ^ (b & c);
      const temp2 = (S0 + maj) >>> 0;
      h = g; g = f; f = e;
      e = (d + temp1) >>> 0;
      d = c; c = b; b = a;
      a = (temp1 + temp2) >>> 0;
    }
    H[0] = (H[0] + a) >>> 0; H[1] = (H[1] + b) >>> 0; H[2] = (H[2] + c) >>> 0; H[3] = (H[3] + d) >>> 0;
    H[4] = (H[4] + e) >>> 0; H[5] = (H[5] + f) >>> 0; H[6] = (H[6] + g) >>> 0; H[7] = (H[7] + h) >>> 0;
  }
  const out = new Uint8Array(32);
  const outView = new DataView(out.buffer);
  for (let index = 0; index < 8; index += 1) outView.setUint32(index * 4, H[index]);
  return out;
}

/** 演示存储的私钥归一化:输出统一为钥匙环形状
 *  `{ keys: { secp256k1?: hex, ed25519?: hex, monero?: hex } }`。
 *  secp 槽收 WIF(base58check)或 hex;ed25519/monero 槽只收 hex。
 *  校验和不对直接原样返回,留给 wasm 后端报错。 */
function normaliseStoredSecret(secret) {
  if (secret?.mode !== "private_key") return secret;
  const incoming = secret.keys && typeof secret.keys === "object" ? secret.keys : { secp256k1: secret.key };
  const ring = {};
  if (incoming.secp256k1) ring.secp256k1 = normaliseKeyHex(incoming.secp256k1) ?? incoming.secp256k1;
  if (incoming.ed25519) ring.ed25519 = normaliseKeyHex(incoming.ed25519) ?? incoming.ed25519;
  if (incoming.monero) ring.monero = normaliseKeyHex(incoming.monero) ?? incoming.monero;
  // 剔除旧形状的 key 字段:serde 的 alias 与 keys 并存会报 duplicate field。
  const { key: _legacyKey, ...rest } = secret;
  return { ...rest, keys: ring };
}

/** 聚合钱包的存储归一化:每条记录按链把钥匙转成规范化 hex;id 缺失时
 *  就地生成。校验不过的记录保留原样,留给 wasm 后端报错。 */
function normaliseAggregateSecret(secret) {
  if (secret?.mode !== "aggregate") return secret;
  // 创建入口是 {chain, key} 单钥形状;转成一条记录再走统一归一。
  const raw = Array.isArray(secret.entries)
    ? secret.entries
    : [{ chain: secret.chain, key: secret.key, label: secret.label }];
  const entries = raw.filter((entry) => String(entry?.key ?? "").trim());
  return {
    ...secret,
    entries: entries.map((entry) => {
      const chain = String(entry.chain ?? "");
      const key = entry.key ?? "";
      // 各链归一:UTXO 收 WIF/hex,EVM/Monero 收 hex,Solana 收 hex/
      // base58 keypair。JS 端只做「能确定的」归一,其余原样留给内核。
      let hex = normaliseKeyHex(key);
      if (!hex && chain === "solana") {
        // 64 字节 base58 keypair → 后 32 字节。
        const decoded = base58Decode(key);
        if (decoded && decoded.length === 64) {
          hex = [...decoded.slice(32)].map((b) => b.toString(16).padStart(2, "0")).join("");
        } else if (decoded && decoded.length === 32) {
          hex = [...decoded].map((b) => b.toString(16).padStart(2, "0")).join("");
        }
      }
      return {
        id: entry.id ?? makeId(),
        chain,
        key: hex ?? key,
        ...(entry.label ? { label: entry.label } : {}),
      };
    }),
  };
}

/** base58 → 字节(Solana keypair 用);非法字符返回 null。 */
function base58Decode(input) {  const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let num = 0n;
  for (const ch of String(input ?? "").trim()) {
    const value = B58.indexOf(ch);
    if (value < 0) return null;
    num = num * 58n + BigInt(value);
  }
  let hex = num.toString(16);
  if (hex.length % 2) hex = `0${hex}`;
  const zeros = [...String(input).trim()].filter((ch) => ch === "1").length;
  const bytes = new Uint8Array(zeros + hex.length / 2);
  for (let i = 0; i < hex.length / 2; i += 1) {
    bytes[zeros + i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  return bytes;
}

/** Solana 私钥(keypair base58 / 种子 base58)→ 32 字节种子 hex。 */
function solanaKeyToHex(input) {
  const bytes = base58Decode(input);
  if (!bytes) return null;
  if (bytes.length === 64) {
    return [...bytes.slice(32)].map((b) => b.toString(16).padStart(2, "0")).join("");
  }
  if (bytes.length === 32) {
    return [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
  }
  return null;
}

/** WIF/hex → 64 位小写 hex;解析不了返回 null。 */
function normaliseKeyHex(input) {
  const text = String(input ?? "").trim();
  if (/^(0[xX])?[0-9a-fA-F]{64}$/.test(text)) {
    return text.replace(/^0[xX]/, "").toLowerCase();
  }
  return wifToHex(text);
}

/** WIF(base58check)→ 32 字节 hex;载荷 33 字节(version+key)或 34 字节
 *  (version+key+压缩标记,末尾 0x01 去掉)。 */
function wifToHex(input) {
  const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
  let num = 0n;
  for (const ch of input) {
    const value = B58.indexOf(ch);
    if (value < 0) return null;
    num = num * 58n + BigInt(value);
  }
  let hex = num.toString(16);
  if (hex.length % 2) hex = `0${hex}`;
  const zeros = [...input].filter((ch) => ch === "1").length;
  let bytes = new Uint8Array(zeros + hex.length / 2);
  for (let i = 0; i < hex.length / 2; i += 1) {
    bytes[zeros + i] = parseInt(hex.slice(i * 2, i * 2 + 2), 16);
  }
  if (bytes.length < 5) return null;
  const payload = bytes.subarray(0, bytes.length - 4);
  const checksum = sha256Bytes(sha256Bytes(payload));
  for (let i = 0; i < 4; i += 1) {
    if (checksum[i] !== bytes[bytes.length - 4 + i]) return null;
  }
  if (payload.length !== 33 && payload.length !== 34) return null;
  // 33 字节 = version+key;34 字节 = version+key+压缩标记(末尾 0x01 要去掉)。
  const keyBytes = payload.length === 34 ? payload.subarray(1, 33) : payload.subarray(1);
  return [...keyBytes].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function perturbSecret(secret, index) {
  if (secret.mode === "aggregate") {
    // 聚合钱包没有派生,永远不会走批量路径;原样返回兜底。
    return secret;
  }
  if (secret.mode === "mnemonic") {
    const words = String(secret.mnemonic ?? "").trim().split(/\s+/);
    const count = words.length === 24 ? 24 : 12;
    const indices = words.slice(0, count).map((word) => BIP39_WORDS.indexOf(word));
    if (indices.some((value) => value < 0)) return secret;
    const totalBits = count * 11;
    const bits = new Uint8Array(totalBits);
    for (let position = 0; position < count; position += 1) {
      for (let bit = 0; bit < 11; bit += 1) {
        bits[position * 11 + bit] = (indices[position] >> (10 - bit)) & 1;
      }
    }
    const checksumBits = count === 24 ? 8 : 4;
    const entropyBits = totalBits - checksumBits;
    // Flip one entropy bit per index step; with 16 entropy bytes the
    // (byte, bit) pair stays distinct for indices 0..127.
    const byteIndex = index % (entropyBits / 8);
    const bitIndex = (index * 5) % 8;
    bits[byteIndex * 8 + bitIndex] ^= 1;
    const entropy = new Uint8Array(entropyBits / 8);
    for (let position = 0; position < entropyBits; position += 1) {
      entropy[position >> 3] = (entropy[position >> 3] << 1) | bits[position];
    }
    const checksum = sha256Bytes(entropy);
    for (let position = 0; position < checksumBits; position += 1) {
      bits[entropyBits + position] = (checksum[position >> 3] >> (7 - (position & 7))) & 1;
    }
    const next = [];
    for (let position = 0; position < count; position += 1) {
      let value = 0;
      for (let bit = 0; bit < 11; bit += 1) value = (value << 1) | bits[position * 11 + bit];
      next.push(BIP39_WORDS[value]);
    }
    return { ...secret, mnemonic: next.join(" ") };
  }
  if (secret.mode === "private_key") {
    // 私钥钱包的演示扰动:按索引翻转 secp 槽密钥 hex 的一个比特,保证
    // 派生地址与基础账户不同(仅演示;桌面端走真实的加盐哈希)。
    const key = String(secret.keys?.secp256k1 ?? secret.key ?? "");
    if (!/^[0-9a-fA-F]{64}$/.test(key)) return secret;
    const byteIndex = Number(index) % 32;
    const bitIndex = (Number(index) * 5) % 8;
    const flipped = 0x01 << bitIndex;
    const bytes = key.match(/.{2}/g).map((h) => parseInt(h, 16));
    bytes[byteIndex] ^= flipped;
    const hex = bytes.map((b) => b.toString(16).padStart(2, "0")).join("");
    if (secret.keys) {
      return { ...secret, keys: { ...secret.keys, secp256k1: hex } };
    }
    return { ...secret, key: hex };
  }
  return { ...secret, phrase: `${secret.phrase ?? ""} #${index}` };
}

function portfolio() {
  const now = Math.floor(Date.now() / 1000);
  const accounts = accountsFor(activeWallet().secret);
  const assets = accounts.map((account) => {
    const chain = CHAINS.find((item) => item.id === account.chain);
    const price = PRICES[account.chain];
    const amount = unitsToText(BALANCES[account.chain], chain.decimals);
    const human = Number(BALANCES[account.chain]) / 10 ** chain.decimals;
    return {
      chain: account.chain,
      name: account.name,
      symbol: account.symbol,
      decimals: chain.decimals,
      accent: account.accent,
      address: account.address,
      amount,
      priceUsd: price.usd,
      priceCny: price.cny,
      valueUsd: human * price.usd,
      valueCny: human * price.cny,
      explorerUrl: "#",
      error: null,
    };
  });
  const totalUsd = assets.reduce((sum, asset) => sum + (asset.valueUsd ?? 0), 0);
  const totalCny = assets.reduce((sum, asset) => sum + (asset.valueCny ?? 0), 0);
  return { assets, totalUsd, totalCny, fetchedAt: now, warnings: [] };
}

function levelLabel(level) {
  return level === "fast" ? "快速" : level === "economy" ? "经济" : "标准";
}

/** The smallest unit of a chain, as its community calls it. */
function baseUnit(chainId) {
  if (chainId === "litecoin") return "里特";
  if (chainId === "dogecoin") return "柴";
  if (chainId === "monero") return "piconero";
  return "聪";
}

function quoteFor(request) {
  const chain = CHAINS.find((item) => item.id === request.chain);
  if (!chain) throw { code: "UNSUPPORTED_CHAIN", message: "不支持的链" };
  const to = String(request.to ?? "").trim();
  // Real checksum / network validation from the Rust core when available.
  const addressOk = core
    ? core.validateAddress(chain.id, to)
    : to.length >= 12;
  if (!addressOk) {
    throw { code: "INVALID_ADDRESS", message: "地址格式不正确（校验和或网络不符）" };
  }
  const balance = BALANCES[chain.id];
  const decimals = chain.decimals;
  const price = PRICES[chain.id].usd;
  // Legacy (pre-SegWit) chains price per byte, SegWit chains per vbyte.
  const legacy = ["dogecoin", "bitcoincash"].includes(chain.id);
  const feeBase = legacy
    ? chain.id === "dogecoin"
      ? 1_000n
      : 2n
    : chain.kind === "utxo"
      ? 2_400n
      : chain.id === "solana"
        ? 5_000n
        : 210_000_000_000_000n;

  const levels = ["fast", "normal", "economy"].map((level, index) => {
    if (chain.kind === "cryptonote") {
      // Same numbers as the Rust side: a typical 1-in/2-out CLSAG transaction
      // weighs ~1.6 KB, the dynamic fee floor is 20,000 piconero/byte, and
      // Monero multiplies it by 20× / 4× / 1× for priority 3 / 2 / 1.
      const multiplier = [20n, 4n, 1n][index];
      const fee = 1_600n * 20_000n * multiplier;
      const human = Number(fee) / 10 ** decimals;
      return {
        level,
        label: levelLabel(level),
        fee: unitsToText(fee, decimals),
        feeFiat: `≈ $${(human * price).toFixed(2)}`,
        eta: index === 2 ? "约 20 分钟" : "约 2 分钟",
        note: `优先级 ${[3, 2, 1][index]} · 环签名 16 · 约 1.6 KB`,
        selected: level === "normal",
      };
    }
    // 费率与 Rust 端一致：legacy 链是基准的 1.5 / 1 / 0.75 倍（聪/字节），
    // SegWit 链是 1.4 / 1 / 0.7 倍（聪/vB）。
    let rate = legacy
      ? index === 0
        ? (feeBase * 3n) / 2n
        : index === 1
          ? feeBase
          : (feeBase * 3n) / 4n
      : index === 0
        ? (feeBase * 14n) / 10n
        : index === 1
          ? feeBase
          : (feeBase * 7n) / 10n;
    // legacy 交易按字节计费，这里用一笔典型交易（约 226 字节）估算费用。
    const fee = legacy ? rate * 226n : rate;
    const feeText = unitsToText(fee, decimals);
    const human = Number(fee) / 10 ** decimals;
    return {
      level,
      label: levelLabel(level),
      fee: feeText,
      feeFiat: `≈ $${(human * price).toFixed(2)}`,
      eta: chain.kind === "utxo" ? (index === 0 ? "约 10 分钟" : index === 1 ? "约 30 分钟" : "约 1 小时") : "约 15 秒",
      note: chain.kind === "utxo"
        ? legacy
          ? `${rate} ${baseUnit(chain.id)}/字节`
          : `${rate} ${baseUnit(chain.id)}/vB`
        : `${[6.2, 4.4, 2.8][index].toFixed(1)} gwei`,
      selected: level === "normal",
    };
  });

  const normalFee = parseUnits(levels[1].fee, decimals);
  const max = balance > normalFee ? balance - normalFee : 0n;
  let amount = max;
  if (request.amount && request.amount.trim() && request.amount.trim() !== "max") {
    amount = parseUnits(request.amount, decimals);
  }
  const canSend = balance >= amount + normalFee && amount > 0n;

  return {
    chain: chain.id,
    symbol: chain.symbol,
    decimals,
    balance: unitsToText(balance, decimals),
    maxAmount: unitsToText(max, decimals),
    amount: unitsToText(amount, decimals),
    to: request.to,
    feeOptions: levels,
    canSend,
    warning: canSend ? null : "可用余额不足以支付金额与手续费",
    note:
      chain.kind === "utxo"
        ? chain.id === "dogecoin"
          ? "Legacy P2PKH（传统 sighash）"
          : chain.id === "bitcoincash"
            ? "Legacy P2PKH · SIGHASH-FORKID"
            : "原生 SegWit（Bech32）"
        : chain.kind === "evm"
          ? "EIP-1559 · 21,000 gas"
          : chain.kind === "cryptonote"
            ? "CryptoNote · RingCT（本地 monero-wallet-rpc 签名）"
            : "Ed25519 系统转账",
  };
}

function pseudoQr(text, dark = "#141416", light = "#ffffff") {
  const size = 29;
  const rand = mulberry32(fnv1a(text));
  const modules = Array.from({ length: size }, () => new Array(size).fill(0));
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      modules[y][x] = rand() > 0.52 ? 1 : 0;
    }
  }
  // 三个定位图案：外圈 + 中心方块
  const drawFinder = (ox, oy) => {
    for (let y = 0; y < 7; y += 1) {
      for (let x = 0; x < 7; x += 1) {
        const border = x === 0 || y === 0 || x === 6 || y === 6;
        const core = x >= 2 && x <= 4 && y >= 2 && y <= 4;
        modules[oy + y][ox + x] = border || core ? 1 : 0;
      }
    }
  };
  drawFinder(0, 0);
  drawFinder(size - 7, 0);
  drawFinder(0, size - 7);

  // 定位图案：圆角方框（描边）+ 中心实心圆角方块，整体内缩半格留出呼吸感
  const quiet = 4;
  const scale = 8;
  const inset = 0.5;
  const total = (size + quiet * 2) * scale;
  const cell = (x, y) => [(x + quiet) * scale, (y + quiet) * scale];
  const shapes = [];

  // 定位图案：圆角方框（描边）+ 中心实心圆角方块
  const ring = (ox, oy, sizeModules, radius) => {
    const [px, py] = cell(ox, oy);
    const half = scale / 2;
    const gap = inset * scale;
    const extent = sizeModules * scale - scale - gap * 2;
    shapes.push(
      `<rect x="${px + half + gap}" y="${py + half + gap}" width="${extent}" height="${extent}" rx="${Math.max(
        0,
        radius - half,
      )}" fill="none" stroke="${dark}" stroke-width="${scale}"/>`,
    );
  };
  const block = (ox, oy, sizeModules, radius, gap = 0) => {
    const [px, py] = cell(ox, oy);
    const extent = sizeModules * scale - gap * 2;
    shapes.push(
      `<rect x="${px + gap}" y="${py + gap}" width="${extent}" height="${extent}" rx="${radius}"/>`,
    );
  };

  const finders = [
    [0, 0],
    [size - 7, 0],
    [0, size - 7],
  ];
  for (const [ox, oy] of finders) {
    ring(ox, oy, 7, 2.6 * scale);
    block(ox + 2, oy + 2, 3, 0.9 * scale, inset * scale);
  }

  // 数据区：每个深色模块一个独立的圆角小块
  const inFinder = (x, y) =>
    (x < 7 && y < 7) || (x >= size - 7 && y < 7) || (x < 7 && y >= size - 7);
  const tile = Math.round(scale * 0.88);
  const offset = Math.round((scale - tile) / 2);
  const radius = Math.round(tile * 0.25);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      if (!modules[y][x] || inFinder(x, y)) continue;
      const [px, py] = cell(x, y);
      shapes.push(
        `<rect x="${px + offset}" y="${py + offset}" width="${tile}" height="${tile}" rx="${radius}"/>`,
      );
    }
  }

  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${total} ${total}" shape-rendering="geometricPrecision"><rect width="${total}" height="${total}" fill="${light}"/><g fill="${dark}">${shapes.join("")}</g></svg>`;
}

/** BIP-39 generator (entropy → SHA-256 checksum → 11-bit words). */
async function generateMnemonic(wordCount) {
  const count = wordCount === 24 ? 24 : 12;
  const entropy = new Uint8Array(count === 24 ? 32 : 16);
  crypto.getRandomValues(entropy);
  const checksum = new Uint8Array(await crypto.subtle.digest("SHA-256", entropy));

  const bits = [];
  for (const byte of entropy) {
    for (let shift = 7; shift >= 0; shift -= 1) bits.push((byte >> shift) & 1);
  }
  // 12 words carry a 4-bit checksum, 24 words an 8-bit one.
  const checksumBits = count * 11 - entropy.length * 8;
  for (let index = 0; index < checksumBits; index += 1) {
    bits.push((checksum[index >> 3] >> (7 - (index & 7))) & 1);
  }

  const words = [];
  for (let index = 0; index < count; index += 1) {
    let value = 0;
    for (let bit = 0; bit < 11; bit += 1) {
      value = (value << 1) | bits[index * 11 + bit];
    }
    words.push(BIP39_WORDS[value]);
  }
  return words.join(" ");
}

/* ------------------------------------------------------------------ dispatch */

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

export async function mockCall(command, args = {}) {
  await sleep(90 + Math.random() * 160);

  switch (command) {
    case "vault_status":
      return {
        exists: store.exists,
        locked: store.locked,
        hasHint: Boolean(store.hint),
        hint: store.hint,
        label: store.locked ? null : activeWallet()?.label ?? null,
      };

    case "create_vault": {
      const request = args.request ?? {};
      if (store.exists) throw { code: "VAULT_EXISTS", message: "本地已存在钱包文件" };
      if ((request.password ?? "").length < 8) {
        throw { code: "WEAK_PASSWORD", message: "密码至少需要 8 个字符" };
      }
      const secret = normaliseStoredSecret(normaliseAggregateSecret(request.secret ?? {}));
      // Real BIP-39 checksum / derivation check before the wallet is stored.
      if (core) corePreview(secret);
      const id = makeId();
      store.wallets = [
        {
          id,
          label:
            request.label ??
            (secret.mode === "mnemonic"
              ? "助记词钱包"
              : secret.mode === "private_key"
                ? "私钥钱包"
                : "口令钱包"),
          secret: { ...secret },
          createdAt: nowSeconds(),
          activity: [],
        },
      ];
      store.activeId = id;
      store.exists = true;
      store.locked = false;
      store.password = request.password;
      // 提示必须与真实口令一致，否则演示里会出现「按提示输入却报密码错误」。
      store.hint = request.hint ?? `演示口令：${request.password}`;
      store.contacts = [];
      persist();
      return walletView();
    }

    case "unlock_vault": {
      if (!store.exists) throw { code: "VAULT_NOT_FOUND", message: "找不到本地钱包文件" };
      // 演示口令在任何状态下都能解锁（数据只是内存假数据）。
      if (args.password !== store.password && args.password !== DEMO_PASSWORD) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      store.locked = false;
      return walletView();
    }

    case "lock_vault":
      store.locked = true;
      return null;

    case "wallet":
      return walletView();

    case "add_wallet": {
      const request = args.request ?? {};
      const secret = normaliseStoredSecret(normaliseAggregateSecret(request.secret ?? {}));
      if (core) corePreview(secret);
      if (secret.mode === "mnemonic") {
        const words = String(secret.mnemonic ?? "").trim().split(/\s+/).filter(Boolean);
        if (words.length !== 12 && words.length !== 24) {
          throw { code: "INVALID_MNEMONIC", message: "助记词需要 12 或 24 个单词" };
        }
      } else if (secret.mode !== "private_key" && secret.mode !== "aggregate" && !String(secret.phrase ?? "").trim()) {
        throw { code: "INVALID_PASSPHRASE", message: "口令不能为空" };
      }
      const id = makeId();
      const label = uniqueLabel(
        request.name ??
          (secret.mode === "mnemonic"
            ? "助记词钱包"
            : secret.mode === "private_key"
              ? "私钥钱包"
              : secret.mode === "aggregate"
                ? "聚合钱包"
                : "口令钱包"),
      );
      store.wallets.push({
        id,
        label,
        secret: { ...secret },
        createdAt: nowSeconds(),
        activity: [],
      });
      store.activeId = id;
      persist();
      return walletView();
    }

    case "switch_wallet": {
      const wallet = store.wallets.find((item) => item.id === args.id);
      if (!wallet) throw { code: "WALLET_NOT_FOUND", message: "找不到该钱包" };
      store.activeId = wallet.id;
      persist();
      return walletView();
    }

    case "preview_batch": {
      const batch = args.batch ?? {};
      const wallet = activeWallet();
      if (store.locked || !wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      const count = Math.min(Math.max(Number(batch?.count ?? 1), 1), 3);
      const start = Math.max(Number(batch?.start ?? 0), 0);
      const strategy = batch?.strategy ?? "salted";
      const chain = batch?.chain;
      if (!chain || !CHAINS.some((item) => item.id === chain)) {
        throw { code: "INVALID_BATCH", message: "缺少有效的币种（chain）" };
      }
      const previews = [];
      for (let offset = 0; offset < count; offset += 1) {
        const index = start + offset;
        // The mock perturbs the secret so different indices look different;
        // the preview only covers the requested chain.
        const branched = perturbSecret(wallet.secret, index);
        const chainAccount = accountsFor(branched).find((a) => a.chain === chain);
        const preview = corePreview(branched)
          ? {
              mode: wallet.secret.mode,
              derivation:
                wallet.secret.mode === "mnemonic" ? null : wallet.secret.derivation ?? "isolated",
              accounts: [chainAccount],
              fingerprint: fingerprintOf([chainAccount]),
              formula: "",
            }
          : {
              mode: wallet.secret.mode,
              derivation:
                wallet.secret.mode === "mnemonic" ? null : wallet.secret.derivation ?? "isolated",
              accounts: [chainAccount],
              fingerprint: fingerprintOf([chainAccount]),
              formula: "",
            };
        previews.push({ ...preview, formula: `批量派生 · ${strategy} · #${index}` });
      }
      return previews;
    }

    case "add_wallets_batch": {
      const request = args.request ?? {};
      const batch = request.batch ?? {};
      const wallet = activeWallet();
      if (store.locked || !wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      // 路径派生只属于助记词钱包;口令钱包没有 BIP-32 树可走。
      if (batch.strategy === "path" && wallet.secret.mode !== "mnemonic") {
        throw { code: "INVALID_BATCH", message: "路径派生只适用于助记词钱包" };
      }
      const count = Math.max(Number(batch.count ?? 1), 1);
      const start = Math.max(Number(batch.start ?? 0), 0);
      const max = batch.strategy === "iterated" ? 50 : 100;
      if (count > max || start + count > max) {
        throw { code: "INVALID_BATCH", message: `账户数量或序号超出范围（1..=${max}）` };
      }
      // 迭代哈希基于基础私钥,0 号会与基础账户重合。
      if (batch.strategy === "iterated" && start < 1) {
        throw { code: "INVALID_BATCH", message: "迭代哈希从序号 1 开始（0 号即基础账户）" };
      }
      const chain = batch.chain;
      if (!chain || !CHAINS.some((item) => item.id === chain)) {
        throw { code: "INVALID_BATCH", message: "缺少有效的币种（chain）" };
      }
      wallet.batchAccounts ??= [];
      for (let offset = 0; offset < count; offset += 1) {
        const index = start + offset;
        const id = `${batch.strategy}:${chain}:${index}`;
        if (wallet.batchAccounts.some((spec) => spec.id === id)) continue;
        // 单链派生:指纹只取该链账户,与其他链无关。
        const demoAccount = accountsFor(perturbSecret(wallet.secret, index)).find(
          (account) => account.chain === chain,
        );
        wallet.batchAccounts.push({
          id,
          strategy: batch.strategy,
          chain,
          index,
          label: request.name ? `${request.name} #${index}` : `#${index}`,
          fingerprint: fingerprintOf([demoAccount]).toUpperCase(),
        });
      }
      if (wallet.batchAccounts.length > 0) {
        wallet.activeBatch = wallet.batchAccounts[wallet.batchAccounts.length - 1].id;
      }
      persist();
      return walletView();
    }

    case "select_batch_account": {
      const wallet = activeWallet();
      if (store.locked || !wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      if (args.id == null) {
        wallet.activeBatch = null;
      } else {
        // 聚合钱包:id 指向钥匙记录本身,不是派生 spec。
        if (wallet.secret.mode === "aggregate") {
          const entry = (wallet.secret.entries ?? []).find((entry) => entry.id === args.id);
          if (!entry) throw { code: "BATCH_ACCOUNT_NOT_FOUND", message: "找不到该钥匙记录" };
        } else {
          const spec = (wallet.batchAccounts ?? []).find((spec) => spec.id === args.id);
          if (!spec) throw { code: "BATCH_ACCOUNT_NOT_FOUND", message: "找不到该派生账户" };
        }
        wallet.activeBatch = args.id;
      }
      persist();
      return walletView();
    }

    case "chain_addresses": {
      const wallet = activeWallet();
      if (store.locked || !wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      const chain = args.chain;
      // 聚合钱包:该链上的每把钥匙一行,id 指向钥匙记录;第一把默认高亮。
      if (wallet.secret.mode === "aggregate") {
        const entries = wallet.secret.entries ?? [];
        // 全链账户顺序 = entries 顺序(core 直出);先筛本链再按链内序号对位。
        const chainEntries = entries
          .map((entry, index) => ({ entry, index }))
          .filter(({ entry }) => entry.chain === chain);
        const all = accountsFor(wallet.secret).filter((a) => a.chain === chain);
        const chainName = CHAINS.find((item) => item.id === chain)?.name ?? chain;
        if (!chainEntries.length) {
          throw { code: "UNSUPPORTED_CHAIN", message: `聚合钱包没有 ${chainName} 的钥匙` };
        }
        return chainEntries.map(({ entry }, localIndex) => {
          const label = entry.label ?? `${chainName} 钥匙 #${localIndex + 1}`;
          return {
            id: entry.id,
            label,
            address: all[localIndex]?.address ?? "",
            path: "imported · direct",
            active: wallet.activeBatch ? wallet.activeBatch === entry.id : localIndex === 0,
          };
        });
      }
      const base = accountsFor(wallet.secret).find((a) => a.chain === chain);
      const rows = [
        {
          id: null,
          label: "基础账户",
          address: base.address,
          path: base.path,
          active: !wallet.activeBatch,
        },
      ];
      // 只列出属于该链的派生账户;其他链的派生不出现在这里。
      for (const spec of wallet.batchAccounts ?? []) {
        if (spec.chain !== chain) continue;
        const account = accountsFor(perturbSecret(wallet.secret, spec.index)).find(
          (a) => a.chain === chain,
        );
        rows.push({
          id: spec.id,
          label: spec.label || `#${spec.index}`,
          address: account.address,
          path: account.path,
          active: wallet.activeBatch === spec.id,
        });
      }
      return rows;
    }

    case "account_detail": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (store.locked || !wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      const chain = args.chain;
      // 聚合钱包:id 指向钥匙记录;导出的就是导入的那把(按链格式)。
      if (wallet.secret.mode === "aggregate") {
        const entries = wallet.secret.entries ?? [];
        const entry = args.id
          ? entries.find((item) => item.id === args.id)
          : entries.find((item) => item.chain === chain);
        if (!entry) throw { code: "BATCH_ACCOUNT_NOT_FOUND", message: "找不到该钥匙记录" };
        if (entry.chain !== chain) {
          throw { code: "UNSUPPORTED_CHAIN", message: "该钥匙不属于此币种" };
        }
        const chainEntries = entries.filter((item) => item.chain === chain);
        const localIndex = chainEntries.indexOf(entry);
        const chainName = CHAINS.find((item) => item.id === chain)?.name ?? chain;
        const label = entry.label ?? `${chainName} 钥匙 #${localIndex + 1}`;
        const account = accountsFor(wallet.secret).filter((a) => a.chain === chain)[localIndex];
        const value = core
          ? core.privateKey(wallet.secret, chain)
          : wallet.secret.entries.find((item) => item.id === entry.id)?.key ?? "";
        const kindLabel =
          chain === "solana"
            ? "ed25519 种子"
            : chain === "monero"
              ? "monero spend key"
              : "原始私钥";
        const format =
          chain === "solana"
            ? "Base58 seed"
            : chain === "monero"
              ? "Hex (spend)"
              : chain === "bitcoin" || chain === "litecoin"
                ? "WIF"
                : "Hex (0x…)";
        return {
          chain,
          chainName: account?.name ?? chainName,
          symbol: account?.symbol ?? "",
          id: entry.id,
          label,
          address: account?.address ?? "",
          addressFormat: account?.addressFormat ?? "",
          path: "imported · direct",
          derivation: "聚合导入 · 直接使用",
          hashAlgorithm: account?.hashAlgorithm ?? "",
          publicKey: account?.publicKey ?? "",
          privateKeyFormat: format,
          privateKey: value,
          sourceKind: "聚合导入",
          sourcePreview: `${chainName}(${account?.address ?? ""})`,
          steps: [
            "取该条目导入的原始私钥字节",
            `字节即该链 ${kindLabel}(无任何派生)`,
          ],
          formula: "imported key → address",
          warning:
            "任何持有该私钥的人都能控制此地址的全部资产。请勿截图、勿粘贴到网页，用完请清理剪贴板。",
        };
      }
      const spec = args.id
        ? (wallet.batchAccounts ?? []).find((item) => item.id === args.id)
        : null;
      if (args.id && !spec) throw { code: "BATCH_ACCOUNT_NOT_FOUND", message: "找不到该派生账户" };
      // 派生账户只属于一条链,跨链查询直接拒绝。
      if (spec && spec.chain !== chain) {
        throw { code: "UNSUPPORTED_CHAIN", message: "该派生账户不属于此币种" };
      }
      // 单链派生:只有目标链用扰动密钥,其他链保持基础派生。
      const secret = spec ? perturbSecret(wallet.secret, spec.index) : wallet.secret;
      const account = accountsFor(secret).find((a) => a.chain === chain);
      let value;
      if (core) {
        value = core.privateKey(secret, chain);
      } else {
        const rand = mulberry32(fnv1a(`${chain}:${args.id ?? "base"}:demo-key`));
        value =
          chain === "bitcoin" || chain === "litecoin"
            ? `${pick(rand, BASE58, 5)}…演示WIF…${pick(rand, BASE58, 5)}`
            : chain === "solana"
              ? pick(rand, BASE58, 64)
              : chain === "monero"
                ? pick(rand, HEX, 128)
                : `0x${pick(rand, HEX, 64)}`;
      }
      const format =
        chain === "solana"
          ? "Base58 keypair"
          : chain === "bitcoin" || chain === "litecoin"
            ? "WIF"
            : chain === "monero"
              ? "Hex (spend‖view)"
              : "Hex (0x…)";
      // 来源说明：让用户能说清私钥从哪来、怎么算（与桌面端 account_detail 一致）。
      const isMnemonic = wallet.secret.mode === "mnemonic";
      const trimmed = isMnemonic ? "" : String(wallet.secret.phrase ?? "").trim();
      const redact = (text) => {
        const chars = [...text.trim()];
        return chars.length <= 4
          ? "•".repeat(chars.length)
          : `${chars.slice(0, 2).join("")}…${chars.slice(-2).join("")}（共 ${chars.length} 字符）`;
      };
      const preview = isMnemonic
        ? "BIP-39 助记词（见 设置 → 导出助记词）"
        : trimmed;
      const basePreview = "基础账户私钥（base）";
      const domain = `lycoris/v1/${chain}`;
      const hashName =
        chain === "litecoin" || chain === "dogecoin"
          ? "scrypt"
          : chain === "monero"
            ? "Keccak-256"
            : "SHA-256";
      const isolated = !isMnemonic && wallet.secret.derivation !== "strict";
      const index = spec?.index ?? 0;
      const strategyKey = spec?.strategy ?? "base";
      let sourceKind;
      let sourcePreview = preview;
      let steps;
      let formula;
      if (isMnemonic) {
        sourceKind = "助记词";
        if (spec && strategyKey === "salted") {
          steps = [
            "取助记词原文（12/24 个单词）",
            `拼消息："${domain}\\0" + 助记词 + "\\0#${index}"`,
            `digest = ${hashName}(消息)`,
            "digest 即该链 32 字节私钥种子（离开标准 BIP-32 树）",
          ];
          formula = `${hashName}("${domain}\\0" ‖ 助记词 ‖ "\\0#${index}")`;
        } else if (spec && strategyKey === "iterated") {
          sourceKind = "基础私钥";
          sourcePreview = basePreview;
          steps = [
            `沿基础路径 ${account.path} 派生基础私钥 base`,
            `连续哈希 base 自己 ${index} 次 → digest`,
            "digest 即该链 32 字节私钥种子",
          ];
          formula = `${hashName}^${index}(base)`;
        } else {
          steps = [
            "助记词 → PBKDF2-HMAC-SHA512 → 512 位种子",
            `沿路径 ${account.path} 做BIP-32/SLIP-0010 子钥派生`,
            "派生出的私钥字节即该链私钥",
          ];
          formula = `BIP-32(${account.path})`;
        }
      } else if (!spec) {
        sourceKind = "口令";
        steps = [
          `取口令（NFKD 规范化）：${redact(trimmed)}`,
          isolated ? `拼消息："${domain}\\0" + 口令` : "拼消息：口令原文",
          `digest = ${hashName}(消息)`,
          "digest 即该链 32 字节私钥种子",
        ];
        formula = isolated ? `${hashName}("${domain}\\0" ‖ 口令)` : `${hashName}(口令)`;
      } else if (strategyKey === "iterated") {
        sourceKind = "基础私钥";
        sourcePreview = basePreview;
        steps = [
          `基础账户私钥 base = ${hashName}(口令)`,
          `连续哈希 base 自己 ${index} 次 → digest`,
          "digest 即该链 32 字节私钥种子",
        ];
        formula = `${hashName}^${index}(base)`;
      } else {
        sourceKind = "口令";
        steps = [
          `取口令（NFKD 规范化）：${redact(trimmed)}`,
          isolated ? `拼消息："${domain}\\0" + 口令` : "拼消息：口令原文",
          `追加账户序号：消息 + "\\0#${index}"`,
          `digest = ${hashName}(消息)`,
          "digest 即该链 32 字节私钥种子",
        ];
        formula = isolated
          ? `${hashName}("${domain}\\0" ‖ 口令 ‖ "\\0#${index}")`
          : `${hashName}(口令 ‖ "\\0#${index}")`;
      }
      return {
        chain,
        chainName: account.name,
        symbol: account.symbol,
        id: spec?.id ?? null,
        label: spec ? spec.label || `#${spec.index}` : "基础账户",
        address: account.address,
        addressFormat: account.addressFormat,
        path: account.path,
        derivation: account.derivation,
        hashAlgorithm: account.hashAlgorithm,
        publicKey: account.publicKey,
        privateKeyFormat: format,
        privateKey: value,
        sourceKind,
        sourcePreview,
        steps,
        formula,
        warning: core
          ? "任何持有该私钥的人都能控制此地址的全部资产。请勿截图、勿粘贴到网页，用完请清理剪贴板。"
          : "演示模式展示的私钥是随机字符串，不控制任何资产。",
      };
    }

    case "rename_batch_account": {
      const wallet = activeWallet();
      if (store.locked || !wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      const spec = (wallet.batchAccounts ?? []).find((spec) => spec.id === args.id);
      if (!spec) throw { code: "BATCH_ACCOUNT_NOT_FOUND", message: "找不到该派生账户" };
      const label = String(args.label ?? "").trim();
      if (!label) throw { code: "INVALID_LABEL", message: "账户名称不能为空" };
      spec.label = label;
      persist();
      return walletView();
    }

    case "remove_batch_account": {
      const wallet = activeWallet();
      if (store.locked || !wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      const accounts = wallet.batchAccounts ?? [];
      const before = accounts.length;
      wallet.batchAccounts = accounts.filter((spec) => spec.id !== args.id);
      if (wallet.batchAccounts.length === before) {
        throw { code: "BATCH_ACCOUNT_NOT_FOUND", message: "找不到该派生账户" };
      }
      if (wallet.activeBatch === args.id) {
        wallet.activeBatch = wallet.batchAccounts[0]?.id ?? null;
      }
      persist();
      return walletView();
    }

    case "generate_mnemonic":
      // Real BIP-39: 128/256 bits of entropy plus a SHA-256 checksum. The
      // wasm core accepts the result, so the demo never creates an invalid wallet.
      return generateMnemonic(args.wordCount === 24 ? 24 : 12);

    case "preview_derivation": {
      let secret = args.secret ?? {};
      // 聚合预览请求是 {mode,chain,key} 单钥形状;转成带 entries 的存储形状
      // 才能被 wasm 内核按 WalletSecret 解析(Tauri 侧由 SecretRequest 完成)。
      if (secret.mode === "aggregate" && !Array.isArray(secret.entries)) {
        const hex =
          normaliseKeyHex(secret.key ?? "") ??
          (secret.chain === "solana" ? solanaKeyToHex(secret.key ?? "") : null);
        if (!hex) {
          throw { code: "INVALID_PRIVATE_KEY", message: "无法识别的私钥格式" };
        }
        secret = {
          mode: "aggregate",
          entries: [{ id: "preview", chain: secret.chain, key: hex }],
        };
      }
      // Real preview (accounts, fingerprint and formula) straight from the core.
      const preview = corePreview(secret);
      if (preview) return preview;
      if (secret.mode === "mnemonic") {
        const words = String(secret.mnemonic ?? "").trim().split(/\s+/).filter(Boolean);
        if (words.length !== 12 && words.length !== 24) {
          throw { code: "INVALID_MNEMONIC", message: "助记词需要 12 或 24 个单词" };
        }
      } else if (!String(secret.phrase ?? "").trim()) {
        throw { code: "INVALID_PASSPHRASE", message: "口令不能为空" };
      }
      const accounts = accountsFor(secret);
      return {
        mode: secret.mode,
        derivation: secret.mode === "mnemonic" ? null : secret.derivation ?? "isolated",
        accounts,
        // Fallback path only: the wasm core is unavailable.
        fingerprint: fingerprintOf(accounts),
        formula:
          secret.mode === "mnemonic"
            ? "BIP-39 助记词 → 512 位种子 → BIP-32 / SLIP-0010 路径"
            : secret.derivation === "strict"
              ? "H_chain(phrase) → 32 字节 → 私钥"
              : 'H_chain("lycoris/v1/<chain>\\0" + phrase) → 32 字节 → 私钥',
      };
    }

    case "reveal_mnemonic": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (wallet?.secret?.mode !== "mnemonic") {
        throw { code: "ERROR", message: "当前钱包没有助记词" };
      }
      return wallet.secret.mnemonic;
    }

    case "update_key_slot": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (!wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      if (wallet.secret.mode !== "private_key") {
        throw { code: "ERROR", message: "只有私钥钱包才能管理钥匙环" };
      }
      const slot = args.slot;
      if (!["secp256k1", "ed25519", "monero"].includes(slot)) {
        throw { code: "INVALID_SLOT", message: "未知的钥匙槽" };
      }
      const hex = normaliseKeyHex(args.key ?? "");
      if (!hex) {
        throw { code: "INVALID_PRIVATE_KEY", message: "无法识别的私钥格式:需要 WIF(base58)或 64 位 hex" };
      }
      if (/^0{64}$/.test(hex)) {
        throw { code: "INVALID_PRIVATE_KEY", message: "零私钥非法" };
      }
      if (!wallet.secret.keys) wallet.secret.keys = {};
      wallet.secret.keys[slot] = hex;
      persist();
      return walletView();
    }

    case "remove_key_slot": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (!wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      if (wallet.secret.mode !== "private_key") {
        throw { code: "ERROR", message: "只有私钥钱包才能管理钥匙环" };
      }
      const slot = args.slot;
      if (wallet.secret.keys) delete wallet.secret.keys[slot];
      if (!wallet.secret.keys?.secp256k1 && !wallet.secret.keys?.ed25519 && !wallet.secret.keys?.monero) {
        throw { code: "INVALID_REQUEST", message: "至少要保留一把私钥" };
      }
      persist();
      return walletView();
    }

    case "add_aggregate_key": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (!wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      if (wallet.secret.mode !== "aggregate") {
        throw { code: "ERROR", message: "只有聚合钱包才能添加聚合钥匙" };
      }
      const chain = String(args.chain ?? "");
      if (!CHAINS.some((item) => item.id === chain)) {
        throw { code: "INVALID_CHAIN", message: "未知的链" };
      }
      const hex = normaliseKeyHex(args.key ?? "") ?? (chain === "solana" ? solanaKeyToHex(args.key ?? "") : null);
      if (!hex) {
        throw { code: "INVALID_PRIVATE_KEY", message: "无法识别的私钥格式" };
      }
      if (/^0{64}$/.test(hex)) {
        throw { code: "INVALID_PRIVATE_KEY", message: "零私钥非法" };
      }
      if (!wallet.secret.entries) wallet.secret.entries = [];
      wallet.secret.entries.push({
        id: makeId(),
        chain,
        key: hex,
        ...(args.label?.trim() ? { label: args.label.trim() } : {}),
      });
      if (core) corePreview(wallet.secret);
      persist();
      return walletView();
    }

    case "remove_aggregate_key": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (!wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      if (wallet.secret.mode !== "aggregate") {
        throw { code: "ERROR", message: "只有聚合钱包才能管理聚合钥匙" };
      }
      const entries = wallet.secret.entries ?? [];
      if (entries.length <= 1) {
        throw { code: "INVALID_REQUEST", message: "聚合钱包至少要保留一把私钥" };
      }
      const before = entries.length;
      wallet.secret.entries = entries.filter((entry) => entry.id !== args.id);
      if (wallet.secret.entries.length === before) {
        throw { code: "NOT_FOUND", message: "找不到钥匙记录" };
      }
      persist();
      return walletView();
    }

    case "rename_aggregate_key": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (!wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      if (wallet.secret.mode !== "aggregate") {
        throw { code: "ERROR", message: "只有聚合钱包才能管理聚合钥匙" };
      }
      const entry = (wallet.secret.entries ?? []).find((item) => item.id === args.id);
      if (!entry) throw { code: "NOT_FOUND", message: "找不到钥匙记录" };
      if (args.label?.trim()) entry.label = args.label.trim();
      else delete entry.label;
      persist();
      return walletView();
    }

    case "export_private_key": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const wallet = activeWallet();
      if (!wallet) throw { code: "LOCKED", message: "钱包已锁定" };
      const chain = args.chain;
      let value;
      if (core) {
        // Real key material, derived by the same core as the desktop app.
        value = core.privateKey(wallet.secret, chain);
      } else {
        const rand = mulberry32(fnv1a(`${chain}:demo-key`));
        value =
          chain === "bitcoin" || chain === "litecoin"
            ? `${pick(rand, BASE58, 5)}…演示WIF…${pick(rand, BASE58, 5)}`
            : chain === "solana"
              ? pick(rand, BASE58, 64)
              : `0x${pick(rand, HEX, 64)}`;
      }
      const format =
        chain === "solana"
          ? "Base58 keypair"
          : chain === "bitcoin" || chain === "litecoin"
            ? "WIF"
            : chain === "monero"
              ? "Hex (spend‖view)"
              : "Hex (0x…)";
      return {
        chain,
        format,
        value,
        warning: core
          ? "私钥由真内核从当前钱包派生。演示钱包只保存在浏览器本地，请勿向这些地址转入真实资产。"
          : "演示模式导出的私钥是随机字符串，不控制任何资产。",
      };
    }

    case "change_password":
      if (args.current !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "当前密码错误" };
      }
      if ((args.newPassword ?? "").length < 8) {
        throw { code: "WEAK_PASSWORD", message: "新密码至少需要 8 个字符" };
      }
      store.password = args.newPassword;
      persist();
      return null;

    case "rename_wallet": {
      const wallet = store.wallets.find((item) => item.id === args.id);
      if (!wallet) throw { code: "WALLET_NOT_FOUND", message: "找不到该钱包" };
      const label = String(args.label ?? "").trim();
      if (!label) throw { code: "INVALID_LABEL", message: "钱包名称不能为空" };
      wallet.label = label;
      persist();
      return walletView();
    }

    case "delete_wallet": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const index = store.wallets.findIndex((item) => item.id === args.id);
      if (index < 0) throw { code: "WALLET_NOT_FOUND", message: "找不到该钱包" };
      if (store.wallets.length <= 1) {
        throw { code: "LAST_WALLET", message: "至少保留一个钱包" };
      }
      store.wallets.splice(index, 1);
      if (store.activeId === args.id) store.activeId = store.wallets[0].id;
      persist();
      return walletView();
    }

    case "refresh_portfolio":
      return portfolio();

    case "refresh_activity": {
      // Demo sync: pending sends confirm, and a fresh receive drops in for
      // chains the demo wallet tracks — enough to showcase the flow.
      const wallet = activeWallet();
      wallet.activity = wallet.activity ?? [];
      let confirmedItems = 0;
      for (const item of wallet.activity) {
        if (item.status === "pending") {
          item.status = "confirmed";
          confirmedItems += 1;
        }
      }
      let newItems = 0;
      if (wallet.activity.length < 30) {
        const chain = CHAINS.find((item) => item.id === "bitcoin") ?? CHAINS[0];
        const decimals = chain.decimals ?? 8;
        // 0.0002 in smallest-unit notation, trimmed like the Rust side does.
        const amount = trimDecimal((2 * 10 ** (decimals - 4)).toString(), decimals);
        wallet.activity.unshift({
          id: crypto.randomUUID ? crypto.randomUUID() : String(Date.now()),
          chain: chain.id,
          direction: "receive",
          amount,
          symbol: chain.symbol,
          counterparty: "bc1q…demo-incoming",
          txid: Array.from({ length: 64 }, () =>
            "0123456789abcdef"[Math.floor(Math.random() * 16)],
          ).join(""),
          status: Math.random() > 0.3 ? "confirmed" : "pending",
          fee: "",
          timestamp: Math.floor(Date.now() / 1000) - Math.floor(Math.random() * 3600),
          explorerUrl: "https://mempool.space/tx/demo",
        });
        newItems = 1;
      }
      wallet.activity = wallet.activity.slice(0, 500);
      persist();
      return {
        newItems,
        confirmedItems,
        failedChains: [],
      };
    }

    case "quote_send":
      return quoteFor(args.request ?? {});

    case "send_transaction": {
      const request = args.request ?? {};
      const chain = CHAINS.find((item) => item.id === request.chain);
      if (!chain) throw { code: "UNSUPPORTED_CHAIN", message: "不支持的链" };
      const quote = quoteFor(request);
      if (!quote.canSend) {
        throw { code: "INSUFFICIENT_BALANCE", message: "余额不足以支付金额与手续费" };
      }
      const fee = quote.feeOptions.find((option) => option.level === (request.level ?? "normal")) ?? quote.feeOptions[1];
      const seed = mulberry32(fnv1a(`${request.chain}:${Date.now()}`));
      // Solana signatures are 88-character base58 strings; everything else
      // here uses a 32-byte hex transaction id.
      const txid = chain.id === "solana" ? pick(seed, BASE58, 88) : pick(seed, HEX, 64);
      const explorerUrl = `${TX_EXPLORERS[chain.id] ?? ""}${txid}`;
      const result = {
        chain: chain.id,
        symbol: chain.symbol,
        txid,
        explorerUrl,
        amount: quote.amount,
        fee: fee.fee,
        to: request.to,
        timestamp: Math.floor(Date.now() / 1000),
      };
      const wallet = activeWallet();
      wallet.activity = wallet.activity ?? [];
      wallet.activity.unshift({
        id: crypto.randomUUID ? crypto.randomUUID() : String(Date.now()),
        chain: chain.id,
        direction: "send",
        amount: result.amount,
        symbol: chain.symbol,
        counterparty: request.to,
        txid,
        status: "pending",
        fee: result.fee,
        timestamp: result.timestamp,
        explorerUrl,
      });
      persist();
      return result;
    }

    case "save_contact": {
      const draft = args.contact ?? {};
      if (!String(draft.name ?? "").trim()) {
        throw { code: "INVALID_CONTACT", message: "联系人名称不能为空" };
      }
      const contactAddress = String(draft.address ?? "").trim();
      const validContact = core
        ? core.validateAddress(draft.chain, contactAddress)
        : contactAddress.length >= 12;
      if (!validContact) {
        throw { code: "INVALID_ADDRESS", message: "地址格式不正确（校验和或网络不符）" };
      }
      const id = draft.id || (crypto.randomUUID ? crypto.randomUUID() : String(Date.now()));
      const existing = store.contacts.find((item) => item.id === id);
      const contact = {
        id,
        name: draft.name.trim(),
        chain: draft.chain,
        address: draft.address.trim(),
        note: String(draft.note ?? "").trim(),
        createdAt: existing?.createdAt ?? Math.floor(Date.now() / 1000),
      };
      store.contacts = existing
        ? store.contacts.map((item) => (item.id === id ? contact : item))
        : [...store.contacts, contact];
      persist();
      return store.contacts;
    }

    case "delete_contact":
      store.contacts = store.contacts.filter((item) => item.id !== args.id);
      persist();
      return store.contacts;

    case "save_settings":
      store.settings = { ...store.settings, ...(args.settings ?? {}) };
      persist();
      return store.settings;

    case "qr_svg": {
      // Real rounded QR from the core; the pseudo renderer is only a fallback
      // for environments where the wasm failed to load.
      if (core) {
        try {
          return core.qrSvg(args.text ?? "", args.dark, args.light);
        } catch (error) {
          console.warn("[lycoris] real QR renderer failed", error);
        }
      }
      return pseudoQr(args.text ?? "", args.dark, args.light);
    }

    case "monero_status":
      return {
        connected: false,
        version: null,
        height: null,
        address: null,
        walletMatches: false,
        message:
          "演示模式没有本地 monero-wallet-rpc；桌面版会连接设置里的地址。",
      };

    case "open_external":
      globalThis.open?.(args.url, "_blank", "noopener");
      return null;

    case "delete_vault":
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      Object.assign(store, {
        exists: false,
        locked: true,
        wallets: [],
        activeId: null,
        contacts: [],
      });
      persist();
      return null;

    case "reset_vault":
      // 忘记密码时的兜底：不需要密码，但前端必须显式确认。
      if (!args.confirm) {
        throw { code: "CONFIRM_REQUIRED", message: "删除钱包文件需要用户明确确认" };
      }
      Object.assign(store, {
        exists: false,
        locked: true,
        wallets: [],
        activeId: null,
        contacts: [],
        password: DEMO_PASSWORD,
        hint: `演示口令：${DEMO_PASSWORD}`,
      });
      persist();
      return null;

    case "tool_inspect_address": {
      const text = String(args.address ?? "").trim();
      if (!text) throw { code: "INVALID_ADDRESS", message: "地址是空的" };
      const matches = [];
      const notes = [];
      const lower = text.toLowerCase();
      if (/^0x[0-9a-f]{40}$/i.test(text) || /^[0-9a-f]{40}$/i.test(text)) {
        const checksummed = text.startsWith("0x") ? text : `0x${text}`;
        for (const id of ["ethereum", "ethereum-classic", "bsc", "polygon"]) {
          matches.push({
            chain: id,
            chainName: CHAINS.find((c) => c.id === id)?.name ?? id,
            symbol: CHAINS.find((c) => c.id === id)?.symbol ?? "",
            addressKind: "evm",
            normalised: checksummed,
            checksumOk: null,
          });
        }
      } else {
        for (const chain of CHAINS.filter((c) => c.kind === "utxo")) {
          if (chain.id === "bitcoincash") continue;
          const prefix = { bitcoin: "bc1", litecoin: "ltc1", viacoin: "via1", groestlcoin: "grs1", vertcoin: "vtc1", monacoin: "mona1", fujicoin: "fc1" }[chain.id];
          const legacy = {
            dogecoin: "D",
            bitcoinsv: "1",
            namecoin: "N",
            litecoincash: "C",
            feathercoin: "6",
            potcoin: "P",
            egulden: "L",
            dash: "X",
          }[chain.id];
          const ok = (prefix && lower.startsWith(prefix)) || (legacy && text.startsWith(legacy) && text.length >= 26 && text.length <= 35);
          if (ok) {
            matches.push({
              chain: chain.id,
              chainName: chain.name,
              symbol: chain.symbol,
              addressKind: prefix ? "p2wpkh (bech32)" : "p2pkh (legacy)",
              normalised: text,
              checksumOk: null,
            });
          }
        }
      }
      if (matches.length === 0) notes.push("演示模式只做前缀启发式识别;桌面版会做完整 base58/bech32 校验。");
      return { matches, notes };
    }

    case "tool_convert_units": {
      const chain = CHAINS.find((c) => c.id === args.chain);
      if (!chain) throw { code: "UNSUPPORTED_CHAIN", message: `不支持的链:${args.chain}` };
      const value = String(args.value ?? "").trim();
      if (!value) throw { code: "INVALID_AMOUNT", message: "金额是空的" };
      if (args.fromSmallest) {
        if (!/^\d+$/.test(value)) throw { code: "INVALID_AMOUNT", message: "最小单位必须是非负整数" };
        const raw = BigInt(value);
        const divisor = 10n ** BigInt(chain.decimals);
        const whole = raw / divisor;
        const frac = (raw % divisor).toString().padStart(chain.decimals, "0").replace(/0+$/, "");
        return {
          chain: chain.id,
          symbol: chain.symbol,
          decimals: chain.decimals,
          smallest: value,
          decimal: frac ? `${whole}.${frac}` : `${whole}`,
        };
      }
      const match = /^(\d+)(?:\.(\d+))?$/.exec(value);
      if (!match) throw { code: "INVALID_AMOUNT", message: "金额格式不正确" };
      if ((match[2] ?? "").length > chain.decimals) {
        throw { code: "INVALID_AMOUNT", message: `最多 ${chain.decimals} 位小数` };
      }
      const frac = (match[2] ?? "").padEnd(chain.decimals, "0");
      return {
        chain: chain.id,
        symbol: chain.symbol,
        decimals: chain.decimals,
        smallest: `${BigInt(match[1]) * 10n ** BigInt(chain.decimals) + BigInt(frac || "0")}`,
        decimal: value,
      };
    }

    case "tool_check_mnemonic": {
      const text = String(args.mnemonic ?? "").trim();
      if (!text) throw { code: "INVALID_MNEMONIC", message: "助记词是空的" };
      const words = text.split(/\s+/);
      const valid = words.length === 12 || words.length === 24;
      return {
        valid,
        wordCount: words.length,
        verdict: valid
          ? "演示模式不做校验和;桌面版会完整校验 BIP-39。"
          : `BIP-39 只接受 12 / 24 词,当前 ${words.length} 词。`,
        words: words.map((word, index) => ({ word, index, valid: true })),
      };
    }

    case "tool_hash": {
      const text = String(args.input ?? "");
      const bytes = new TextEncoder().encode(text);
      const algorithm = String(args.algorithm ?? "sha256");
      // 演示模式使用 WebCrypto(仅 SHA 系列);Keccak/RIPEMD 提示走桌面版。
      const names = { sha256: "SHA-256", sha512: "SHA-512", "double-sha256": "SHA-256d", keccak256: "Keccak-256", hash160: "HASH160", ripemd160: "RIPEMD-160" };
      if (!["sha256", "sha512"].includes(algorithm)) {
        return {
          algorithm: names[algorithm] ?? algorithm,
          hex: `(演示模式不提供 ${names[algorithm] ?? algorithm},请使用桌面版)`,
          base64: "",
        };
      }
      const digest = await crypto.subtle.digest(algorithm.toUpperCase(), bytes);
      const hex = [...new Uint8Array(digest)].map((b) => b.toString(16).padStart(2, "0")).join("");
      const b64 = btoa(String.fromCharCode(...new Uint8Array(digest)));
      return { algorithm: names[algorithm], hex, base64: b64 };
    }

    case "tool_encode": {
      const text = String(args.input ?? "").trim();
      if (!text) throw { code: "ERROR", message: "输入是空的" };
      const bytes = text.startsWith("0x")
        ? new Uint8Array(text.slice(2).match(/../g)?.map((h) => parseInt(h, 16)) ?? [])
        : new TextEncoder().encode(text);
      const hex = [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
      const b64 = btoa(String.fromCharCode(...bytes));
      const B58 = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
      let n = 0n;
      for (const b of bytes) n = n * 256n + BigInt(b);
      let b58 = "";
      while (n > 0n) {
        b58 = B58[Number(n % 58n)] + b58;
        n /= 58n;
      }
      for (const b of bytes) {
        if (b === 0) b58 = "1" + b58;
        else break;
      }
      return { hex, base64: b64, base58: b58 || "1" };
    }

    case "tool_parse_uri": {
      const text = String(args.uri ?? "").trim();
      const at = text.indexOf(":");
      if (at <= 0) throw { code: "ERROR", message: "不是 payment URI:缺少 scheme" };
      const scheme = text.slice(0, at).toLowerCase();
      const rest = text.slice(at + 1);
      const [path, query = ""] = rest.split("?");
      const params = new URLSearchParams(query);
      const chainMap = {
        bitcoin: "bitcoin", litecoin: "litecoin", dogecoin: "dogecoin", bitcoincash: "bitcoincash",
        ethereum: "ethereum", bsc: "bsc", polygon: "polygon", solana: "solana",
        bitcoinsv: "bitcoinsv", namecoin: "namecoin", litecoincash: "litecoincash",
        feathercoin: "feathercoin", viacoin: "viacoin", potcoin: "potcoin",
        egulden: "egulden", dash: "dash", groestlcoin: "groestlcoin", vertcoin: "vertcoin",
      };
      const id = chainMap[scheme];
      if (!id) throw { code: "ERROR", message: `不支持的 scheme:${scheme}` };
      const chain = CHAINS.find((c) => c.id === id);
      let amount = params.get("amount") ?? params.get("value");
      if (params.has("value") && amount && /^\d+$/.test(amount) && chain) {
        const raw = BigInt(amount);
        const divisor = 10n ** BigInt(chain.decimals);
        const whole = raw / divisor;
        const frac = (raw % divisor).toString().padStart(chain.decimals, "0").replace(/0+$/, "");
        amount = frac ? `${whole}.${frac}` : `${whole}`;
      }
      return {
        chain: id,
        chainName: chain?.name ?? id,
        symbol: chain?.symbol ?? "",
        address: path,
        amount,
        label: params.get("label"),
        message: params.get("message"),
        memo: null,
        requestId: null,
        warnings: [],
      };
    }

    case "tool_compose_uri": {
      const chain = CHAINS.find((c) => c.id === args.chain);
      if (!chain) throw { code: "UNSUPPORTED_CHAIN", message: `不支持的链:${args.chain}` };
      const parts = [];
      if (args.amount) parts.push(`amount=${args.amount}`);
      if (args.label) parts.push(`label=${encodeURIComponent(args.label)}`);
      if (args.message) parts.push(`message=${encodeURIComponent(args.message)}`);
      const scheme = { "ethereum-classic": "ethereum" }[chain.id] ?? chain.id;
      return `${scheme}:${args.address}${parts.length ? `?${parts.join("&")}` : ""}`;
    }

    case "tool_sign_message":
      throw { code: "LOCKED", message: "演示模式不提供消息签名;请使用桌面版。" };

    case "tool_verify_message":
      throw { code: "LOCKED", message: "演示模式不提供消息验签;请使用桌面版。" };

    case "touch_session":
      return null;

    case "window_minimize":
    case "window_toggle_maximize":
    case "window_close":
      return command === "window_toggle_maximize" ? false : null;

    default:
      throw { code: "UNKNOWN_COMMAND", message: `演示模式未实现：${command}` };
  }
}

export const mockChains = CHAINS;
