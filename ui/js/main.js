/** Application shell: boot sequence, router, rail, titlebar and auto-lock. */

import { api, hasTauri, isMock, listen } from "./api.js?v=24";
import { escapeHtml } from "./format.js?v=24";
import { icon, openModal, toast } from "./ui.js?v=24";
import { render as renderActivity } from "./views/activity.js?v=24";
import { render as renderAsset } from "./views/asset.js?v=24";
import { render as renderContacts } from "./views/contacts.js?v=24";
import { render as renderDashboard } from "./views/dashboard.js?v=24";
import { render as renderOnboarding } from "./views/onboarding.js?v=24";
import { render as renderReceive } from "./views/receive.js?v=24";
import { render as renderSend } from "./views/send.js?v=24";
import { render as renderSettings } from "./views/settings.js?v=24";
import { render as renderUnlock } from "./views/unlock.js?v=24";
import { render as renderWallets } from "./views/wallets.js?v=24";

const VERSION = "0.3.0";
const PORTFOLIO_TTL = 60; // seconds
const TOUCH_INTERVAL = 20_000; // ms between session touches
const AUTO_REFRESH_INTERVAL = 120_000; // ms

const VIEWS = {
  onboarding: renderOnboarding,
  unlock: renderUnlock,
  dashboard: renderDashboard,
  asset: renderAsset,
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
let compactRail = window.matchMedia("(max-width: 760px)").matches;

// 桌面侧栏 ↔ 手机底部标签栏：跨越断点时重建导航。
const compactQuery = window.matchMedia("(max-width: 760px)");
compactQuery.addEventListener?.("change", (event) => {
  if (event.matches !== compactRail) {
    compactRail = event.matches;
    if (state.wallet) renderRail();
  }
});

/** Re-read the vault status. Needed after the wallet file is deleted. */
async function refreshStatus() {
  try {
    state.status = await api.vaultStatus();
  } catch (error) {
    state.status = { exists: false, locked: true, hasHint: false, hint: null, label: null };
  }
  syncChrome();
  return state.status;
}

const ctx = { state, go, setWallet, refreshPortfolio, refreshStatus, applyTheme, lockNow };

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
  const compact = compactRail;
  if (!compact) {
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
    return;
  }

  // 手机底部标签栏：常用四项 + 「更多」抽屉
  const primary = ["dashboard", "receive", "send", "activity"];
  const primaryItems = RAIL_GROUPS.flatMap((group) => group.items).filter((item) =>
    primary.includes(item.id),
  );
  const moreItems = RAIL_GROUPS.flatMap((group) => group.items).filter(
    (item) => !primary.includes(item.id),
  );
  rail.innerHTML =
    primaryItems
      .map(
        (item) =>
          `<button class="rail-item" type="button" data-view="${item.id}">${icon(
            item.icon,
          )}<span>${escapeHtml(item.label)}</span></button>`,
      )
      .join("") +
    `<button class="rail-item" type="button" data-more aria-expanded="false">${icon(
      "menu",
    )}<span>更多</span></button>`;
  rail.querySelectorAll(".rail-item[data-view]").forEach((button) => {
    button.addEventListener("click", () => go(button.dataset.view));
  });
  rail.querySelector("[data-more]")?.addEventListener("click", () => {
    const current = state.view;
    openModal(
      `<h2>更多</h2><div class="stack-sm" style="margin-top:12px">
        ${moreItems
          .map(
            (item) =>
              `<button class="more-item${item.id === current ? " is-current" : ""}" type="button" data-view="${item.id}">${icon(
                item.icon,
              )}<span>${escapeHtml(item.label)}</span>${icon("chevronRight")}</button>`,
          )
          .join("")}
      </div>`,
      {
        onMount(element, close) {
          element.querySelectorAll("[data-view]").forEach((button) => {
            button.addEventListener("click", () => {
              close();
              go(button.dataset.view);
            });
          });
        },
      },
    );
  });
}

/* --------------------------------------------------------------- lifecycle */

function applyTheme(theme) {
  const choice = theme ?? state.wallet?.settings?.theme ?? "system";
  const dark =
    choice === "dark" ||
    (choice === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  const next = dark ? "dark" : "light";
  const changed = document.documentElement.dataset.theme !== next;
  document.documentElement.dataset.theme = next;
  // 二维码颜色是烤进 SVG 的，主题变了要重画
  if (changed) document.dispatchEvent(new CustomEvent("lycoris:theme"));
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
      // 链开关：总览仅展示可见链，法币总额按可见资产重算
      const visible = state.wallet?.settings?.visibleChains;
      if (Array.isArray(visible) && visible.length > 0 && Array.isArray(portfolio?.assets)) {
        const keep = new Set(visible);
        const assets = portfolio.assets.filter((a) => keep.has(a.chain));
        const sum = (key) =>
          assets.reduce((acc, a) => acc + (typeof a[key] === "number" ? a[key] : 0), 0);
        state.portfolio = {
          ...portfolio,
          assets,
          totalUsd: assets.some((a) => typeof a.valueUsd === "number") ? sum("valueUsd") : null,
          totalCny: assets.some((a) => typeof a.valueCny === "number") ? sum("valueCny") : null,
        };
      } else {
        state.portfolio = portfolio;
      }
      return state.portfolio;
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
