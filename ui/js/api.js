/** Bridge to the Rust backend.
 *
 * The Tauri shell exposes `window.__TAURI__.core.invoke` because the window is
 * configured with `withGlobalTauri`. Opening `index.html` in a plain browser
 * (or adding `?mock=1`) switches to an in-memory mock so the interface can be
 * developed and reviewed without building the desktop app. */

import { mockCall } from "./mock.js";

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

export async function call(command, args = {}) {
  if (isMock) return mockCall(command, args);
  try {
    return await tauri.core.invoke(command, args);
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
  renameWallet: (label) => call("rename_wallet", { label }),
  refreshPortfolio: () => call("refresh_portfolio"),
  quoteSend: (request) => call("quote_send", { request }),
  sendTransaction: (request) => call("send_transaction", { request }),
  saveContact: (contact) => call("save_contact", { contact }),
  deleteContact: (id) => call("delete_contact", { id }),
  saveSettings: (settings) => call("save_settings", { settings }),
  qrSvg: (text, dark, light) => call("qr_svg", { text, dark, light }),
  openExternal: (url) => call("open_external", { url }),
  deleteVault: (password) => call("delete_vault", { password }),
  touchSession: () => call("touch_session"),
  windowMinimize: () => call("window_minimize"),
  windowToggleMaximize: () => call("window_toggle_maximize"),
  windowClose: () => call("window_close"),
};
