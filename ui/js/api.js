/** Bridge to the Rust backend.
 *
 * The Tauri shell exposes `window.__TAURI__.core.invoke` because the window is
 * configured with `withGlobalTauri`. Opening `index.html` in a plain browser
 * (or adding `?mock=1`) switches to an in-memory mock so the interface can be
 * developed and reviewed without building the desktop app. */

import { mockCall } from "./mock.js?v=23";

const tauri = globalThis.__TAURI__;
const forceMock = new URLSearchParams(location.search).has("mock");

export const hasTauri = Boolean(tauri?.core?.invoke);
export const isMock = forceMock || !hasTauri;

function normalizeError(error) {
  if (error && typeof error === "object") {
    const code = error.code ?? error.type ?? "ERROR";
    const message = error.message ?? error.error ?? String(error);
    return { code, message };
  }
  return { code: "ERROR", message: String(error ?? "未知错误") };
}

/** 链开关：仅过滤 UI 展示的账户，密钥派生不受影响。空数组/未设置 = 全部显示。 */
export function applyVisibleChains(wallet, visibleChains) {
  if (!Array.isArray(visibleChains) || visibleChains.length === 0 || !Array.isArray(wallet?.accounts)) {
    return wallet;
  }
  return { ...wallet, accounts: wallet.accounts.filter((a) => visibleChains.includes(a.chain)) };
}

export async function call(command, args = {}) {
  const unwrap = (result) =>
    command === "wallet" ? applyVisibleChains(result, result?.settings?.visibleChains) : result;
  if (isMock) return unwrap(await mockCall(command, args));
  try {
    return unwrap(await tauri.core.invoke(command, args));
  } catch (error) {
    throw normalizeError(error);
  }
}

/** 未应用链开关过滤的完整钱包视图（设置页的链开关需要全量列表）。 */
export async function walletFull() {
  if (isMock) return mockCall("wallet", {});
  try {
    return await tauri.core.invoke("wallet", {});
  } catch (error) {
    throw normalizeError(error);
  }
}

export function listen(event, handler) {
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
  vaultStatus: () => call("vault_status"),
  createVault: (request) => call("create_vault", { request }),
  unlockVault: (password) => call("unlock_vault", { password }),
  lockVault: () => call("lock_vault"),
  wallet: () => call("wallet"),
  generateMnemonic: (wordCount = 12) => call("generate_mnemonic", { wordCount }),
  previewDerivation: (secret) => call("preview_derivation", { secret }),
  revealMnemonic: (password) => call("reveal_mnemonic", { password }),
  exportPrivateKey: (chain, password) => call("export_private_key", { chain, password }),
  changePassword: (current, newPassword) =>
    call("change_password", { current, newPassword }),
  renameWallet: (id, label) => call("rename_wallet", { id, label }),
  addWallet: (request) => call("add_wallet", { request }),
  switchWallet: (id) => call("switch_wallet", { id }),
  deleteWallet: (id, password) => call("delete_wallet", { id, password }),
  refreshPortfolio: () => call("refresh_portfolio"),
  quoteSend: (request) => call("quote_send", { request }),
  sendTransaction: (request) => call("send_transaction", { request }),
  saveContact: (contact) => call("save_contact", { contact }),
  deleteContact: (id) => call("delete_contact", { id }),
  saveSettings: (settings) => call("save_settings", { settings }),
  moneroStatus: () => call("monero_status"),
  qrSvg: (text, dark, light) => call("qr_svg", { text, dark, light }),
  openExternal: (url) => call("open_external", { url }),
  deleteVault: (password) => call("delete_vault", { password }),
  resetVault: () => call("reset_vault", { confirm: true }),
  touchSession: () => call("touch_session"),
  windowMinimize: () => call("window_minimize"),
  windowToggleMaximize: () => call("window_toggle_maximize"),
  windowClose: () => call("window_close"),
};
