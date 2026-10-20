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
};

const TX_EXPLORERS = {  bitcoin: "https://mempool.space/tx/",
  litecoin: "https://litecoinspace.org/tx/",
  dogecoin: "https://blockchair.com/dogecoin/transaction/",
  bitcoincash: "https://blockchair.com/bitcoin-cash/transaction/",
  ethereum: "https://etherscan.io/tx/",
  "ethereum-classic": "https://blockscout.com/etc/mainnet/tx/",
  bsc: "https://bscscan.com/tx/",
  polygon: "https://polygonscan.com/tx/",
  solana: "https://solscan.io/tx/",
  monero: "https://xmrchain.net/tx/",
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
  return parts.join(" ");
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
  const preview = corePreview(secret);
  if (preview) return preview.accounts;
  const seedText = secret.mode === "mnemonic" ? secret.mnemonic : `${secret.phrase}:${secret.derivation}`;
  return CHAINS.map((chain) => {
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
              : "bech32",
      publicKey: `0x${pick(keyStream(chain.id, seedText, secret.mode === "mnemonic" ? "mnemonic" : secret.derivation), HEX, 64)}`,
      accent: chain.accent,
      explorerUrl: "#",
      sendSupported: true,
      decimals: chain.decimals,
    };
  });
}

function walletView() {
  const wallet = activeWallet();
  if (store.locked || !wallet) {
    throw { code: "LOCKED", message: "钱包已锁定" };
  }
  const accounts = accountsFor(wallet.secret);
  const preview = corePreview(wallet.secret);
  const isMnemonic = wallet.secret.mode === "mnemonic";
  return {
    id: wallet.id,
    mode: wallet.secret.mode,
    derivation: isMnemonic ? null : wallet.secret.derivation ?? "isolated",
    label: wallet.label,
    fingerprint: preview?.fingerprint ?? fingerprintOf(accounts),
    createdAt: wallet.createdAt ?? nowSeconds(),
    formula:
      preview?.formula ??
      (isMnemonic
        ? "BIP-39 助记词 → 512 位种子 → BIP-32 / SLIP-0010 路径"
        : wallet.secret.derivation === "strict"
          ? "H_chain(phrase) → 32 字节 → 私钥"
          : 'H_chain("lycoris/v1/<chain>\\0" + phrase) → 32 字节 → 私钥'),
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
  };
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
      const secret = request.secret ?? {};
      // Real BIP-39 checksum / derivation check before the wallet is stored.
      if (core) corePreview(secret);
      const id = makeId();
      store.wallets = [
        {
          id,
          label:
            request.label ?? (secret.mode === "mnemonic" ? "助记词钱包" : "口令钱包"),
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
      const secret = request.secret ?? {};
      if (core) corePreview(secret);
      if (secret.mode === "mnemonic") {
        const words = String(secret.mnemonic ?? "").trim().split(/\s+/).filter(Boolean);
        if (words.length !== 12 && words.length !== 24) {
          throw { code: "INVALID_MNEMONIC", message: "助记词需要 12 或 24 个单词" };
        }
      } else if (!String(secret.phrase ?? "").trim()) {
        throw { code: "INVALID_PASSPHRASE", message: "口令不能为空" };
      }
      const id = makeId();
      const label = uniqueLabel(
        request.name ?? (secret.mode === "mnemonic" ? "助记词钱包" : "口令钱包"),
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

    case "generate_mnemonic":
      // Real BIP-39: 128/256 bits of entropy plus a SHA-256 checksum. The
      // wasm core accepts the result, so the demo never creates an invalid wallet.
      return generateMnemonic(args.wordCount === 24 ? 24 : 12);

    case "preview_derivation": {
      const secret = args.secret ?? {};
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
