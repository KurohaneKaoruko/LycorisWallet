/** Domain model mirroring `crates/core/src/models.rs` and `derive.rs`.
 *
 * Every struct on the Rust side is `#[serde(rename_all = "camelCase")]`, so
 * the field names here are exactly what `tauri.core.invoke` returns. Keep the
 * two files in sync when editing the Rust models. */

export type ChainId =
  | "bitcoin"
  | "litecoin"
  | "dogecoin"
  | "bitcoincash"
  | "ethereum"
  | "ethereum-classic"
  | "bsc"
  | "polygon"
  | "solana"
  | "monero";

export type FiatCurrency = "usd" | "cny";
export type Theme = "system" | "light" | "dark";
export type WalletMode = "mnemonic" | "passphrase" | "private_key" | "aggregate";
/** 钥匙环的槽类别(私钥钱包)。 */
export type KeySlotKind = "secp256k1" | "ed25519" | "monero";
/** 钥匙环快照:每个槽 present 与否决定对应链是否显示。 */
export interface KeyRingView {
  secp256k1: string | null;
  ed25519: string | null;
  monero: string | null;
}
/** 聚合钱包的钥匙目录(id/链/备注,不含钥匙本体)。 */
export interface AggregateEntryView {
  id: string;
  chain: ChainId;
  label: string | null;
}
export type DerivationMode = "isolated" | "strict";
export type BatchStrategy = "path" | "salted" | "iterated";
export type Direction = "send" | "receive";
export type ActivityStatus = "pending" | "confirmed" | "failed";

export interface Settings {
  fiat: FiatCurrency;
  theme: Theme;
  hideBalances: boolean;
  autoLockMinutes: number;
  priceService: boolean;
  /** 链开关：空数组 = 全部显示。 */
  visibleChains: ChainId[];
  blockchairApiKey: string;
  moneroRpcUrl: string;
  moneroRpcUser: string;
  moneroRpcPassword: string;
  moneroRestoreHeight: number;
}

export interface Contact {
  id: string;
  name: string;
  chain: ChainId;
  address: string;
  note: string;
  createdAt: number;
}

export interface ActivityItem {
  id: string;
  chain: ChainId;
  direction: Direction;
  amount: string;
  symbol: string;
  counterparty: string;
  txid: string;
  status: ActivityStatus;
  timestamp: number;
  note: string;
}

export interface Account {
  chain: ChainId;
  name: string;
  symbol: string;
  decimals: number;
  accent: string;
  address: string;
  path: string;
  derivation: string;
  hashAlgorithm: string;
  /** 该链哈希算法的说明文案（Rust Account.hashDetail）。 */
  hashDetail: string;
  publicKey: string;
  addressFormat: string;
  /** 该链交易浏览器前缀（Rust Account.explorerUrl）。 */
  explorerUrl: string;
  sendSupported: boolean;
}

export interface WalletSummary {
  id: string;
  label: string;
  mode: WalletMode;
  derivation: DerivationMode | null;
  fingerprint: string;
  createdAt: number;
  activityCount: number;
  active: boolean;
}

/** 派生账户（配方 + 公开值，不含密钥材料）。 */
export interface DerivedAccountView {
  id: string;
  strategy: BatchStrategy;
  /** 该派生账户只作用于这一条链。 */
  chain: ChainId;
  index: number;
  label: string;
  fingerprint: string;
  active: boolean;
}

export interface WalletView {
  id: string;
  mode: WalletMode;
  derivation: DerivationMode | null;
  label: string;
  fingerprint: string;
  createdAt: number;
  formula: string;
  /** 私钥钱包的钥匙环快照(其他模式为 null)。 */
  keyRing: KeyRingView | null;
  /** 聚合钱包的钥匙目录(其他模式为 null)。 */
  aggregateEntries: AggregateEntryView[] | null;
  accounts: Account[];
  settings: Settings;
  contacts: Contact[];
  activity: ActivityItem[];
  /** 保险柜里的全部钱包，active 一个带 active: true。 */
  wallets: WalletSummary[];
  batchAccounts: DerivedAccountView[];
  /** null = 正在显示基础派生。 */
  activeBatch: string | null;
}

export interface VaultStatus {
  exists: boolean;
  locked: boolean;
  hasHint: boolean;
  hint: string | null;
  label: string | null;
}

/** 资产子页的一行地址（基础 + 该链的派生账户）。 */
export interface ChainAddressView {
  /** null = 基础派生。 */
  id: string | null;
  label: string;
  address: string;
  path: string;
  active: boolean;
}

/** 地址详情弹窗：公开信息 + 验密后才返回的私钥与来源链。 */
export interface AccountDetailView {
  chain: ChainId;
  chainName: string;
  symbol: string;
  id: string | null;
  label: string;
  address: string;
  addressFormat: string;
  path: string;
  derivation: string;
  hashAlgorithm: string;
  publicKey: string;
  privateKeyFormat: string;
  privateKey: string;
  sourceKind: string;
  sourcePreview: string;
  steps: string[];
  formula: string;
  warning: string;
}

export interface AssetBalance {
  chain: ChainId;
  name: string;
  symbol: string;
  decimals: number;
  accent: string;
  address: string;
  amount: string;
  priceUsd: number | null;
  priceCny: number | null;
  valueUsd: number | null;
  valueCny: number | null;
  explorerUrl: string;
  error: string | null;
}

export interface Portfolio {
  assets: AssetBalance[];
  totalUsd: number | null;
  totalCny: number | null;
  fetchedAt: number;
  warnings: string[];
}

export interface FeeOption {
  level: string;
  label: string;
  fee: string;
  feeFiat: string | null;
  eta: string;
  note: string;
  selected: boolean;
}

export interface SendQuote {
  chain: ChainId;
  symbol: string;
  decimals: number;
  balance: string;
  maxAmount: string;
  amount: string;
  to: string;
  feeOptions: FeeOption[];
  canSend: boolean;
  warning: string | null;
  note: string;
}

export interface SendResult {
  chain: ChainId;
  symbol: string;
  txid: string;
  explorerUrl: string;
  amount: string;
  fee: string;
  to: string;
  timestamp: number;
}

/** 派生预览（批量添加前的确认列表）。 */
export interface DerivationPreview {
  mode: WalletMode;
  derivation: DerivationMode | null;
  accounts: Account[];
  fingerprint: string;
  formula: string;
}

/** 批量派生请求：单链模型，一次只添加一个币种的地址。 */
export interface BatchRequest {
  strategy: BatchStrategy;
  chain: ChainId;
  count: number;
  start: number;
}

/** 密钥来源 discriminated union，对应 Rust `SecretRequest`。 */
export type SecretRequest =
  | { mode: "mnemonic"; mnemonic: string }
  | { mode: "passphrase"; phrase: string; derivation: DerivationMode };

/** 后端错误的统一形态（ApiError）。 */
export interface ApiErrorLike {
  code: string;
  message: string;
}
