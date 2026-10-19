/** Application shell: boot sequence, router, rail, titlebar and auto-lock. */

import { api, hasTauri, isMock, listen } from "./api.js";
import { escapeHtml } from "./format.js";
import { icon, toast } from "./ui.js";
import { render as renderActivity } from "./views/activity.js";
import { render as renderContacts } from "./views/contacts.js";
import { render as renderDashboard } from "./views/dashboard.js";
import { render as renderOnboarding } from "./views/onboarding.js";
import { render as renderReceive } from "./views/receive.js";
import { render as renderSend } from "./views/send.js";
import { render as renderSettings } from "./views/settings.js";
import { render as renderUnlock } from "./views/unlock.js";
import { render as renderWallets } from "./views/wallets.js";

const VERSION = "0.1.0";
const PORTFOLIO_TTL = 60; // seconds
const TOUCH_INTERVAL = 20_000; // ms between session touches
const AUTO_REFRESH_INTERVAL = 120_000; // ms

const VIEWS = {
  onboarding: renderOnboarding,
  unlock: renderUnlock,
  dashboard: renderDashboard,
  receive: renderReceive,
  send: renderSend,
  activity: renderActivity,
  contacts: renderContacts,
  settings: renderSettings,
  wallets: renderWallets,
};

const RAIL_GROUPS = [
  {
    label: "钱包",
    items: [
      { id: "dashboard", label: "总览", icon: "dashboard" },
      { id: "receive", label: "收款", icon: "receive" },
      { id: "send", label: "转账", icon: "send" },
      { id: "activity", label: "交易记录", icon: "activity" },
    ],
  },
  {
    label: "管理",
    items: [
      { id: "wallets", label: "钱包管理", icon: "wallet" },
      { id: "contacts", label: "地址簿", icon: "contacts" },
      { id: "settings", label: "设置", icon: "settings" },
    ],
  },
];

const stage = document.getElementById("stage");
const rail = document.getElementById("rail");
const tbCenter = document.getElementById("tb-center");
const mockChip = document.getElementById("mock-chip");
const lockButton = document.getElementById("btn-lock");

const state = {
  view: "boot",
  params: {},
  wallet: null,
  status: null,
  portfolio: null,
  portfolioLoading: false,
  lastSendChain: null,
  lastReceiveChain: null,
};

let cleanup = null;
let portfolioFetch = null;
let lastTouch = 0;

const ctx = { state, go, setWallet, refreshPortfolio, applyTheme, lockNow };

/* ------------------------------------------------------------------ router */

function go(view, params = {}) {
  if (!VIEWS[view]) view = state.wallet ? "dashboard" : "unlock";
  state.view = view;
  state.params = { ...params };
  renderRoute();
}

function renderRoute() {
  const factory = VIEWS[state.view] ?? VIEWS.dashboard;
  cleanup?.();
  cleanup = null;
  const result = factory(ctx, state.params);
  stage.innerHTML = result.html;
  stage.scrollTop = 0;
  const dispose = result.mount?.(stage);
  cleanup = typeof dispose === "function" ? dispose : null;
  syncChrome();
}

function syncChrome() {
  const app = document.getElementById("app");
  if (app) app.dataset.view = state.view;
  rail.hidden = !state.wallet;
  lockButton.hidden = !state.wallet;
  mockChip.hidden = !isMock;
  rail.querySelectorAll(".rail-item").forEach((item) => {
    item.setAttribute("aria-current", item.dataset.view === state.view ? "true" : "false");
  });
  const walletCount = state.wallet?.wallets?.length ?? 0;
  tbCenter.textContent = state.wallet
    ? `${state.wallet.label}${walletCount > 1 ? `（共 ${walletCount} 个钱包）` : ""} · ${state.wallet.fingerprint}`
    : state.view === "onboarding"
      ? "创建或导入钱包"
      : state.status?.exists
        ? "钱包已锁定"
        : "欢迎";
}

function renderRail() {
  rail.innerHTML =
    RAIL_GROUPS.map(
      (group) => `<div class="rail-group">${escapeHtml(group.label)}</div>
        ${group.items
          .map(
            (item) =>
              `<button class="rail-item" type="button" data-view="${item.id}">${icon(
                item.icon,
              )}<span>${escapeHtml(item.label)}</span></button>`,
          )
          .join("")}`,
    ).join("") +
    `<div class="rail-foot">Lycoris Wallet v${VERSION}<br />${
      isMock ? "演示模式 · 数据不会离开浏览器" : "密钥仅保存在本机"
    }</div>`;
  rail.querySelectorAll(".rail-item").forEach((button) => {
    button.addEventListener("click", () => go(button.dataset.view));
  });
}

/* --------------------------------------------------------------- lifecycle */

function applyTheme(theme) {
  const choice = theme ?? state.wallet?.settings?.theme ?? "system";
  const dark =
    choice === "dark" ||
    (choice === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
}

function setWallet(wallet) {
  const previous = state.wallet;
  state.wallet = wallet;
  if (!wallet) {
    state.portfolio = null;
    state.portfolioLoading = false;
  }
  applyTheme();
  renderRail();
  if (wallet && (!previous || previous.fingerprint !== wallet.fingerprint)) {
    state.portfolio = null;
    void refreshPortfolio({ force: true }).catch(() => {});
  }
  syncChrome();
}

async function refreshPortfolio({ force = false } = {}) {
  if (!state.wallet) return null;
  if (
    !force &&
    state.portfolio &&
    Date.now() / 1000 - state.portfolio.fetchedAt < PORTFOLIO_TTL
  ) {
    return state.portfolio;
  }
  if (portfolioFetch) return portfolioFetch;
  state.portfolioLoading = true;
  if (state.view === "dashboard") renderRoute();
  portfolioFetch = api
    .refreshPortfolio()
    .then((portfolio) => {
      state.portfolio = portfolio;
      return portfolio;
    })
    .finally(() => {
      state.portfolioLoading = false;
      portfolioFetch = null;
      if (state.view === "dashboard") renderRoute();
    });
  return portfolioFetch;
}

async function lockNow({ silent = false } = {}) {
  if (!state.wallet) return;
  const previous = state.status ?? {};
  const label = state.wallet.label;
  try {
    await api.lockVault();
  } catch {
    /* the local session is dropped either way */
  }
  state.status = { ...previous, exists: true, locked: true, label };
  setWallet(null);
  go("unlock");
  if (!silent) toast("钱包已锁定", "info");
}

/* -------------------------------------------------------------------- boot */

async function boot() {
  mockChip.hidden = !isMock;
  applyTheme();
  if (isMock) {
    document.title = "Lycoris Wallet（演示模式）";
    mockChip.title = "未连接到 Rust 后端，当前展示的是浏览器内的演示数据。";
  }
  if (!hasTauri) {
    document.querySelector(".win-controls")?.remove();
  } else {
    document.getElementById("win-min")?.addEventListener("click", () => void api.windowMinimize());
    document.getElementById("win-max")?.addEventListener("click", () => void api.windowToggleMaximize());
    document.getElementById("win-close")?.addEventListener("click", () => void api.windowClose());
  }
  lockButton?.addEventListener("click", () => void lockNow());

  try {
    state.status = await api.vaultStatus();
  } catch (error) {
    toast(error.message, "error");
    state.status = { exists: false, locked: true, hasHint: false, hint: null, label: null };
  }

  if (!state.status.exists) {
    go("onboarding");
    return;
  }
  if (state.status.locked) {
    go("unlock");
    return;
  }
  try {
    const wallet = await api.wallet();
    setWallet(wallet);
    go("dashboard");
  } catch {
    go("unlock");
  }
}

/* ------------------------------------------------------------------ events */

listen("lycoris://locked", () => {
  if (!state.wallet) return;
  state.status = { ...(state.status ?? {}), exists: true, locked: true };
  setWallet(null);
  go("unlock");
  toast("长时间未操作，钱包已自动锁定", "info");
});

const onActivity = () => {
  if (!state.wallet) return;
  const now = Date.now();
  if (now - lastTouch < TOUCH_INTERVAL) return;
  lastTouch = now;
  void api.touchSession().catch(() => {});
};
document.addEventListener("pointerdown", onActivity, { passive: true });
document.addEventListener("keydown", onActivity);

matchMedia("(prefers-color-scheme: dark)").addEventListener("change", () => {
  if ((state.wallet?.settings?.theme ?? "system") === "system") applyTheme();
});

setInterval(() => {
  if (state.wallet && document.visibilityState === "visible") {
    void refreshPortfolio().catch(() => {});
  }
}, AUTO_REFRESH_INTERVAL);

document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible" && state.wallet) {
    void refreshPortfolio().catch(() => {});
  }
});

void boot();
