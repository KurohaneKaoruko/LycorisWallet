/** In-memory stand-in for the Rust backend.
 *
 * Used when the interface runs in a plain browser (`?mock=1` or no Tauri
 * host). It mimics the command surface, formats and error codes, but keeps
 * every "secret" fake — the UI shows a 演示模式 badge so nobody mistakes it for
 * a real wallet. */

const CHAINS = [
  { id: "bitcoin", name: "Bitcoin", symbol: "BTC", decimals: 8, accent: "#F7931A", kind: "utxo" },
  { id: "litecoin", name: "Litecoin", symbol: "LTC", decimals: 8, accent: "#345D9D", kind: "utxo" },
  { id: "dogecoin", name: "Dogecoin", symbol: "DOGE", decimals: 8, accent: "#C2A633", kind: "utxo" },
  { id: "bitcoincash", name: "Bitcoin Cash", symbol: "BCH", decimals: 8, accent: "#8DC351", kind: "utxo" },
  { id: "ethereum", name: "Ethereum", symbol: "ETH", decimals: 18, accent: "#627EEA", kind: "evm" },
  { id: "ethereum-classic", name: "Ethereum Classic", symbol: "ETC", decimals: 18, accent: "#3AB83A", kind: "evm" },
  { id: "bsc", name: "BNB Chain", symbol: "BNB", decimals: 18, accent: "#F3BA2F", kind: "evm" },
  { id: "polygon", name: "Polygon", symbol: "POL", decimals: 18, accent: "#8247E5", kind: "evm" },
  { id: "arbitrum", name: "Arbitrum One", symbol: "ETH", decimals: 18, accent: "#12AAFF", kind: "evm" },
  { id: "optimism", name: "OP Mainnet", symbol: "ETH", decimals: 18, accent: "#FF0420", kind: "evm" },
  { id: "base", name: "Base", symbol: "ETH", decimals: 18, accent: "#0052FF", kind: "evm" },
  { id: "avalanche", name: "Avalanche C-Chain", symbol: "AVAX", decimals: 18, accent: "#E84142", kind: "evm" },
  { id: "solana", name: "Solana", symbol: "SOL", decimals: 9, accent: "#14F195", kind: "ed25519" },
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
  arbitrum: { usd: 3182.7, cny: 22880.4 },
  optimism: { usd: 3182.7, cny: 22880.4 },
  base: { usd: 3182.7, cny: 22880.4 },
  avalanche: { usd: 38.42, cny: 276.2 },
  solana: { usd: 187.63, cny: 1348.9 },
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
  arbitrum: 820_000_000_000_000_000n,
  optimism: 640_000_000_000_000_000n,
  base: 1_180_000_000_000_000_000n,
  avalanche: 2_460_000_000_000_000_000n,
  solana: 8_420_000_000n,
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
  arbitrum: ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  optimism: ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  base: ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  avalanche: ["Keccak-256", "Ethereum 原生哈希 · 32 字节摘要"],
  solana: ["SHA-256", "FIPS 180-4 · 32 字节摘要"],
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
  arbitrum: "m/44'/60'/0'/0/0",
  optimism: "m/44'/60'/0'/0/0",
  base: "m/44'/60'/0'/0/0",
  avalanche: "m/44'/60'/0'/0/0",
  solana: "m/44'/501'/0'/0'",
};

const TX_EXPLORERS = {
  bitcoin: "https://mempool.space/tx/",
  litecoin: "https://litecoinspace.org/tx/",
  dogecoin: "https://blockchair.com/dogecoin/transaction/",
  bitcoincash: "https://blockchair.com/bitcoin-cash/transaction/",
  ethereum: "https://etherscan.io/tx/",
  "ethereum-classic": "https://blockscout.com/etc/mainnet/tx/",
  bsc: "https://bscscan.com/tx/",
  polygon: "https://polygonscan.com/tx/",
  arbitrum: "https://arbiscan.io/tx/",
  optimism: "https://optimistic.etherscan.io/tx/",
  base: "https://basescan.org/tx/",
  avalanche: "https://snowtrace.io/tx/",
  solana: "https://solscan.io/tx/",
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

function fakeAddress(chain, seedText) {
  const rand = mulberry32(fnv1a(`${chain}:${seedText}`));
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

const store = {
  version: 2,
  exists: false,
  locked: true,
  password: "lycoris-demo",
  hint: "演示口令：lycoris-demo",
  wallets: [],
  activeId: null,
  settings: {
    fiat: "usd",
    theme: "system",
    hideBalances: false,
    autoLockMinutes: 5,
    priceService: true,
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
  const seedText = secret.mode === "mnemonic" ? secret.mnemonic : `${secret.phrase}:${secret.derivation}`;
  return CHAINS.map((chain) => {
    const [algorithm, detail] = ALGOS[chain.id];
    return {
      chain: chain.id,
      name: chain.name,
      symbol: chain.symbol,
      address: fakeAddress(chain.id, seedText),
      path: secret.mode === "mnemonic" ? PATHS[chain.id] : "—",
      derivation:
        secret.mode === "mnemonic"
          ? "BIP-32 / SLIP-0010"
          : secret.derivation === "strict"
            ? "H(phrase)"
            : "H_chain(隔离)",
      hashAlgorithm: algorithm,
      hashDetail: detail,
      addressFormat: chain.id === "solana" ? "base58" : chain.kind === "evm" ? "EIP-55" : "bech32",
      publicKey: `0x${pick(mulberry32(fnv1a(`${chain.id}:${seedText}:pub`)), HEX, 64)}`,
      accent: chain.accent,
      explorerUrl: "#",
      sendSupported: !["dogecoin", "bitcoincash"].includes(chain.id),
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
  const isMnemonic = wallet.secret.mode === "mnemonic";
  return {
    id: wallet.id,
    mode: wallet.secret.mode,
    derivation: isMnemonic ? null : wallet.secret.derivation ?? "isolated",
    label: wallet.label,
    fingerprint: fingerprintOf(accounts),
    createdAt: wallet.createdAt ?? nowSeconds(),
    formula: isMnemonic
      ? "BIP-39 助记词 → 512 位种子 → BIP-32 / SLIP-0010 路径"
      : wallet.secret.derivation === "strict"
        ? 'H_chain(phrase) → 32 字节 → 私钥'
        : 'H_chain("lycoris/v1/<chain>\\0" + phrase) → 32 字节 → 私钥',
    accounts,
    settings: store.settings,
    contacts: store.contacts,
    activity: wallet.activity ?? [],
    wallets: store.wallets.map((item) => ({
      id: item.id,
      label: item.label,
      mode: item.secret.mode,
      derivation: item.secret.mode === "mnemonic" ? null : item.secret.derivation ?? "isolated",
      fingerprint: fingerprintOf(accountsFor(item.secret)),
      createdAt: item.createdAt ?? nowSeconds(),
      activityCount: (item.activity ?? []).length,
      active: item.id === wallet.id,
    })),
  };
}

function portfolio() {
  const now = Math.floor(Date.now() / 1000);
  const accounts = accountsFor(activeWallet().secret);
  const assets = accounts.map((account) => {
    const chain = CHAINS.find((item) => item.id === account.chain);
    const amount = unitsToText(BALANCES[account.chain], chain.decimals);
    const human = Number(BALANCES[account.chain]) / 10 ** chain.decimals;
    const price = PRICES[account.chain];
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

function quoteFor(request) {
  const chain = CHAINS.find((item) => item.id === request.chain);
  if (!chain) throw { code: "UNSUPPORTED_CHAIN", message: "不支持的链" };
  if (["dogecoin", "bitcoincash"].includes(chain.id)) {
    throw { code: "SEND_UNSUPPORTED", message: "该链的转账功能即将支持，收款地址已可用" };
  }
  if (!request.to || request.to.length < 12) {
    throw { code: "INVALID_ADDRESS", message: "地址格式不正确" };
  }
  const balance = BALANCES[chain.id];
  const decimals = chain.decimals;
  const price = PRICES[chain.id].usd;

  const feeBase =
    chain.kind === "utxo"
      ? 2_400n
      : chain.id === "solana"
        ? 5_000n
        : 210_000_000_000_000n;

  const levels = ["fast", "normal", "economy"].map((level, index) => {
    const scale = index === 0 ? 14n : index === 1 ? 10n : 7n;
    const fee = (feeBase * scale) / 10n;
    const feeText = unitsToText(fee, decimals);
    const human = Number(fee) / 10 ** decimals;
    return {
      level,
      label: levelLabel(level),
      fee: feeText,
      feeFiat: `≈ $${(human * price).toFixed(2)}`,
      eta: chain.kind === "utxo" ? (index === 0 ? "约 10 分钟" : index === 1 ? "约 30 分钟" : "约 1 小时") : "约 15 秒",
      note: chain.kind === "utxo" ? `${[14, 10, 7][index]} 聪/vB` : `${[6.2, 4.4, 2.8][index].toFixed(1)} gwei`,
      selected: level === "normal",
    };
  });

  const normalFee = (feeBase * 10n) / 10n;
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
          ? "Legacy P2PKH（转账即将支持）"
          : chain.id === "bitcoincash"
            ? "cashaddr（转账即将支持）"
            : "原生 SegWit（Bech32）"
        : chain.kind === "evm"
          ? "EIP-1559 · 21,000 gas"
          : "Ed25519 系统转账",
  };
}

function pseudoQr(text) {
  const size = 29;
  const rand = mulberry32(fnv1a(text));
  const modules = Array.from({ length: size }, () => new Array(size).fill(0));
  const inFinder = (x, y) =>
    (x < 8 && y < 8) || (x >= size - 8 && y < 8) || (x < 8 && y >= size - 8);
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      modules[y][x] = rand() > 0.52 ? 1 : 0;
    }
  }
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
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      if (inFinder(x, y)) continue;
    }
  }
  const paths = [];
  const scale = 8;
  for (let y = 0; y < size; y += 1) {
    for (let x = 0; x < size; x += 1) {
      if (modules[y][x]) paths.push(`M${x * scale} ${y * scale}h${scale}v${scale}h-${scale}z`);
    }
  }
  const extent = size * scale;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${extent} ${extent}" shape-rendering="crispEdges"><rect width="${extent}" height="${extent}" fill="#f7f1e8"/><path d="${paths.join("")}" fill="#1b1a19"/></svg>`;
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
      store.hint = request.hint ?? null;
      store.contacts = [];
      persist();
      return walletView();
    }

    case "unlock_vault": {
      if (!store.exists) throw { code: "VAULT_NOT_FOUND", message: "找不到本地钱包文件" };
      if (args.password !== store.password) {
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

    case "generate_mnemonic": {
      const words =
        "abandon ability able about above absent absorb abstract absurd abuse access accident account achieve acid acoustic acquire across action adapt address adjust admit adult advance advice aerobic affair afford afraid again agent agree ahead aim air airport aisle alarm album alert alien alley allow almost alone alpha already also alter always amazing among amount amused analyst anchor ancient anger angle angry animal ankle announce annual another answer antenna antique anxiety any apart apology appear apple approve april arch arctic area arena argue arm armed armor army around arrange arrest arrive arrow artefact artist artwork ask aspect assault asset assist assume asthma athlete atom attack attend attitude attract auction audit august aunt author auto autumn average avocado avoid awake aware away awesome awful awkward axis".split(
          " ",
        );
      const rand = mulberry32(Math.floor(Math.random() * 2 ** 32));
      const count = args.wordCount === 24 ? 24 : 12;
      const picked = [];
      while (picked.length < count) {
        const word = words[Math.floor(rand() * words.length)];
        picked.push(word);
      }
      return picked.join(" ");
    }

    case "preview_derivation": {
      const secret = args.secret ?? {};
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
      return `${wallet.secret.mnemonic} (演示数据)`;
    }

    case "export_private_key": {
      if (args.password !== store.password) {
        throw { code: "INCORRECT_PASSWORD", message: "密码错误" };
      }
      const rand = mulberry32(fnv1a(`${args.chain}:demo-key`));
      const value =
        args.chain === "bitcoin" || args.chain === "litecoin"
          ? `${pick(rand, BASE58, 5)}…演示WIF…${pick(rand, BASE58, 5)}`
          : args.chain === "solana"
            ? pick(rand, BASE58, 64)
            : `0x${pick(rand, HEX, 64)}`;
      return {
        chain: args.chain,
        format: args.chain === "solana" ? "Base58 keypair" : args.chain === "bitcoin" || args.chain === "litecoin" ? "WIF" : "Hex (0x…)",
        value,
        warning: "演示模式导出的私钥是随机字符串，不控制任何资产。",
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
      const txid = pick(mulberry32(fnv1a(`${request.chain}:${Date.now()}`)), HEX, 64);
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
      if (String(draft.address ?? "").trim().length < 12) {
        throw { code: "INVALID_ADDRESS", message: "地址格式不正确" };
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

    case "qr_svg":
      return pseudoQr(args.text ?? "");

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
