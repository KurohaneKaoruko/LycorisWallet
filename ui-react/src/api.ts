/** Bridge to the Rust backend, ported from ui/js/api.js.
 *
 * The Tauri shell exposes `window.__TAURI__.core.invoke` (withGlobalTauri).
 * Opening the dev server in a plain browser — or adding `?mock=1` — switches to
 * the in-memory mock kernel so the UI can run without the desktop app. */

import { mockCall } from "./mock/mock";

import type {
  AccountDetailView,
  ApiErrorLike,
  BatchRequest,
  ChainAddressView,
  ChainId,
  Contact,
  DerivationPreview,
  KeySlotKind,
  Portfolio,
  SendQuote,
  SendResult,
  Settings,
  VaultStatus,
  WalletView,
} from "./types";

interface TauriLike {
  core: { invoke(cmd: string, args?: Record<string, unknown>): Promise<unknown> };
  event?: {
    listen(event: string, handler: (payload: unknown) => void): Promise<() => void>;
  };
}

const tauri = (globalThis as { __TAURI__?: TauriLike }).__TAURI__;
const forceMock = new URLSearchParams(window.location.search).has("mock");

export const hasTauri = Boolean(tauri?.core?.invoke);
export const isMock = forceMock || !hasTauri;

function normalizeError(error: unknown): ApiErrorLike {
  if (error && typeof error === "object") {
    const e = error as Record<string, unknown>;
    const code = (e.code ?? e.type ?? "ERROR") as string;
    const message = String(e.message ?? e.error ?? "未知错误");
    return { code, message };
  }
  return { code: "ERROR", message: String(error ?? "未知错误") };
}

/** 链开关：仅过滤 UI 展示的账户，密钥派生不受影响。空数组/未设置 = 全部显示。 */
export function applyVisibleChains(wallet: WalletView, visibleChains: ChainId[]): WalletView {
  if (
    !Array.isArray(visibleChains) ||
    visibleChains.length === 0 ||
    !Array.isArray(wallet?.accounts)
  ) {
    return wallet;
  }
  return { ...wallet, accounts: wallet.accounts.filter((a) => visibleChains.includes(a.chain)) };
}

async function invoke(command: string, args: Record<string, unknown>): Promise<unknown> {
  try {
    return await tauri!.core.invoke(command, args);
  } catch (error) {
    throw normalizeError(error);
  }
}

export async function call(command: string, args: Record<string, unknown> = {}): Promise<unknown> {
  const unwrap = (result: unknown) =>
    command === "wallet"
      ? applyVisibleChains(
          result as WalletView,
          (result as WalletView | null)?.settings?.visibleChains ?? [],
        )
      : result;
  if (isMock) return unwrap(await mockCall(command, args));
  return unwrap(await invoke(command, args));
}

/** 未应用链开关过滤的完整钱包视图（设置页的链开关需要全量列表）。 */
export async function walletFull(): Promise<WalletView> {
  if (isMock) return (await mockCall("wallet", {})) as WalletView;
  return (await invoke("wallet", {})) as WalletView;
}

export function listen(event: string, handler: (payload: unknown) => void): () => void {
  if (isMock || !tauri?.event?.listen) return () => {};
  let unlisten = () => {};
  tauri.event
    .listen(event, (payload) => handler(payload))
    .then((fn) => {
      unlisten = fn;
    })
    .catch(() => {});
  return () => unlisten();
}

/* ------------------------------------------------------------- typed calls */

export const api = {
  vaultStatus: (): Promise<VaultStatus> => call("vault_status") as Promise<VaultStatus>,
  createVault: (request: {
    password: string;
    hint?: string | null;
    label?: string | null;
    secret: unknown;
  }) => call("create_vault", { request }) as Promise<WalletView>,
  unlockVault: (password: string) => call("unlock_vault", { password }) as Promise<WalletView>,
  lockVault: () => call("lock_vault") as Promise<void>,
  wallet: (): Promise<WalletView> => call("wallet") as Promise<WalletView>,
  generateMnemonic: (wordCount = 12) =>
    call("generate_mnemonic", { wordCount }) as Promise<string>,
  previewDerivation: (secret: unknown) =>
    call("preview_derivation", { secret }) as Promise<DerivationPreview>,
  revealMnemonic: (password: string) =>
    call("reveal_mnemonic", { password }) as Promise<string>,
  exportPrivateKey: (chain: ChainId, password: string) =>
    call("export_private_key", { chain, password }) as Promise<{ privateKey: string; format: string }>,
  updateKeySlot: (slot: KeySlotKind, key: string, password: string) =>
    call("update_key_slot", { slot, key, password }) as Promise<WalletView>,
  removeKeySlot: (slot: KeySlotKind, password: string) =>
    call("remove_key_slot", { slot, password }) as Promise<WalletView>,
  changePassword: (current: string, newPassword: string) =>
    call("change_password", { current, newPassword }) as Promise<void>,
  renameWallet: (id: string, label: string) =>
    call("rename_wallet", { id, label }) as Promise<WalletView>,
  addWallet: (request: unknown) => call("add_wallet", { request }) as Promise<WalletView>,
  addAggregateKey: (chain: ChainId, key: string, label: string | null, password: string) =>
    call("add_aggregate_key", { chain, key, label, password }) as Promise<WalletView>,
  removeAggregateKey: (id: string, password: string) =>
    call("remove_aggregate_key", { id, password }) as Promise<WalletView>,
  renameAggregateKey: (id: string, label: string, password: string) =>
    call("rename_aggregate_key", { id, label, password }) as Promise<WalletView>,
  previewBatch: (batch: BatchRequest) =>
    call("preview_batch", { batch }) as Promise<DerivationPreview[]>,
  addWalletsBatch: (request: { batch: BatchRequest; name: string | null }) =>
    call("add_wallets_batch", { request }) as Promise<WalletView>,
  selectBatchAccount: (id: string | null) => call("select_batch_account", { id }) as Promise<WalletView>,
  chainAddresses: (chain: ChainId) => call("chain_addresses", { chain }) as Promise<ChainAddressView[]>,
  accountDetail: (chain: ChainId, id: string | null, password: string) =>
    call("account_detail", { chain, id, password }) as Promise<AccountDetailView>,
  renameBatchAccount: (id: string, label: string) =>
    call("rename_batch_account", { id, label }) as Promise<WalletView>,
  removeBatchAccount: (id: string) => call("remove_batch_account", { id }) as Promise<WalletView>,
  switchWallet: (id: string) => call("switch_wallet", { id }) as Promise<WalletView>,
  deleteWallet: (id: string, password: string) =>
    call("delete_wallet", { id, password }) as Promise<WalletView>,
  refreshPortfolio: (): Promise<Portfolio> => call("refresh_portfolio") as Promise<Portfolio>,
  quoteSend: (request: unknown) => call("quote_send", { request }) as Promise<SendQuote>,
  sendTransaction: (request: unknown) => call("send_transaction", { request }) as Promise<SendResult>,
  saveContact: (contact: Partial<Contact>) => call("save_contact", { contact }) as Promise<Contact[]>,
  deleteContact: (id: string) => call("delete_contact", { id }) as Promise<Contact[]>,
  saveSettings: (settings: Settings) => call("save_settings", { settings }) as Promise<Settings>,
  moneroStatus: () => call("monero_status") as Promise<Record<string, unknown>>,
  qrSvg: (text: string, dark = "#1a1c1e", light = "#ffffff") =>
    call("qr_svg", { text, dark, light }) as Promise<string>,
  openExternal: (url: string) => call("open_external", { url }) as Promise<void>,
  deleteVault: (password: string) => call("delete_vault", { password }) as Promise<void>,
  resetVault: () => call("reset_vault", { confirm: true }) as Promise<void>,
  touchSession: () => call("touch_session") as Promise<void>,
  windowMinimize: () => call("window_minimize") as Promise<void>,
  windowToggleMaximize: () => call("window_toggle_maximize") as Promise<void>,
  windowClose: () => call("window_close") as Promise<void>,
};
