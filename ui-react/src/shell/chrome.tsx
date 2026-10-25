/** App chrome: titlebar (window controls, lock button, mock chip), the nav
 *  rail / mobile tab bar, and the auto-lock + session touch timers.
 *  Ported from ui/js/main.js renderRail/syncChrome/timers. */

import { Fragment, useEffect, useState } from "react";

import { api, isMock } from "../api";
import { useSession, type ViewId } from "./session";
import { Icon } from "../ui/Icon";

const VERSION = "0.4.0";
const TOUCH_INTERVAL = 20_000; // ms between session touches
const AUTO_REFRESH_INTERVAL = 120_000; // ms

const RAIL_GROUPS: Array<{ label: string; items: Array<{ id: ViewId; label: string; icon: string }> }> = [
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
      { id: "contacts", label: "联系人", icon: "contacts" },
      { id: "settings", label: "设置", icon: "settings" },
    ],
  },
];

function useCompact(): boolean {
  const [compact, setCompact] = useState(() => window.matchMedia("(max-width: 760px)").matches);
  useEffect(() => {
    const query = window.matchMedia("(max-width: 760px)");
    const onChange = (event: MediaQueryListEvent) => setCompact(event.matches);
    query.addEventListener?.("change", onChange);
    return () => query.removeEventListener?.("change", onChange);
  }, []);
  return compact;
}

export function Titlebar({ walletVisible }: { walletVisible: boolean }) {
  const { wallet, view, status, lockNow } = useSession();
  // 旧版 syncChrome():中段文字随状态切换(钱包指纹 / 引导 / 已锁定 / 欢迎)。
  const walletCount = wallet?.wallets?.length ?? 0;
  const center = wallet
    ? `${wallet.label}${walletCount > 1 ? `（共 ${walletCount} 个钱包）` : ""} · ${wallet.fingerprint}`
    : view === "onboarding"
      ? "创建或导入钱包"
      : status?.exists
        ? "钱包已锁定"
        : "欢迎";

  const winControls = (
    <div className="win-controls">
      <button className="win-btn" type="button" title="最小化" aria-label="最小化" onClick={() => api.windowMinimize()}>
        <svg viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" strokeWidth="1.2" /></svg>
      </button>
      <button
        className="win-btn"
        type="button"
        title="最大化 / 还原"
        aria-label="最大化或还原"
        onClick={() => api.windowToggleMaximize()}
      >
        <svg viewBox="0 0 12 12"><rect x="2.6" y="2.6" width="6.8" height="6.8" rx="1" stroke="currentColor" strokeWidth="1.2" fill="none" /></svg>
      </button>
      <button className="win-btn win-close" type="button" title="关闭" aria-label="关闭" onClick={() => api.windowClose()}>
        <svg viewBox="0 0 12 12"><path d="m3 3 6 6M9 3l-6 6" stroke="currentColor" strokeWidth="1.2" /></svg>
      </button>
    </div>
  );

  return (
    <header className="titlebar" data-tauri-drag-region>
      <div className="tb-brand" data-tauri-drag-region>
        <img className="logo" src="/favicon-flower.svg" alt="" draggable={false} />
        <span className="tb-title">Lycoris Wallet</span>
      </div>
      <div className="tb-center" data-tauri-drag-region>
        {center}
      </div>
      <div className="tb-actions">
        {isMock ? <span className="chip chip-mock">演示模式</span> : null}
        {walletVisible && wallet ? (
          <button className="tb-btn" type="button" title="立即锁定" onClick={() => void lockNow()}>
            <svg viewBox="0 0 20 20" aria-hidden="true">
              <path d="M6 9V7.2a4 4 0 1 1 8 0V9" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
              <rect x="4.4" y="9" width="11.2" height="8.2" rx="2.2" fill="none" stroke="currentColor" strokeWidth="1.6" />
            </svg>
          </button>
        ) : null}
        {winControls}
      </div>
    </header>
  );
}

export function Rail() {
  const { wallet, view, go } = useSession();
  const compact = useCompact();
  const [moreOpen, setMoreOpen] = useState(false);

  // 旧版 syncChrome():未解锁时导航整条隐藏(rail.hidden = !wallet)。
  if (!wallet) return null;

  if (compact) {
    // 手机底部标签栏：常用四项 + 「更多」抽屉（与旧版一致）。
    const primary = ["dashboard", "receive", "send", "activity"];
    const all = RAIL_GROUPS.flatMap((group) => group.items);
    const primaryItems = all.filter((item) => primary.includes(item.id));
    const moreItems = all.filter((item) => !primary.includes(item.id));
    return (
      <nav className="rail" aria-label="页面导航">
        {primaryItems.map((item) => (
          <button
            key={item.id}
            className="rail-item"
            type="button"
            data-view={item.id}
            onClick={() => go(item.id)}
          >
            <Icon name={item.icon} />
            <span>{item.label}</span>
          </button>
        ))}
        <button
          className="rail-item"
          type="button"
          data-more
          aria-expanded={moreOpen}
          onClick={() => setMoreOpen(true)}
        >
          <Icon name="menu" />
          <span>更多</span>
        </button>
        {moreOpen ? (
          <div className="modal-backdrop" onClick={() => setMoreOpen(false)}>
            <div
              className="modal"
              role="dialog"
              aria-modal="true"
              onClick={(event) => event.stopPropagation()}
            >
              <h2>更多</h2>
              <div className="stack-sm" style={{ marginTop: 12 }}>
                {moreItems.map((item) => (
                  <button
                    key={item.id}
                    className={`more-item${view === item.id ? " is-current" : ""}`}
                    type="button"
                    data-view={item.id}
                    onClick={() => {
                      setMoreOpen(false);
                      go(item.id);
                    }}
                  >
                    <Icon name={item.icon} />
                    <span>{item.label}</span>
                    <Icon name="chevronRight" />
                  </button>
                ))}
              </div>
            </div>
          </div>
        ) : null}
      </nav>
    );
  }

  return (
    <nav className="rail">
      {RAIL_GROUPS.map((group) => (
        <Fragment key={group.label}>
          <div className="rail-group">{group.label}</div>
          {group.items.map((item) => (
            <button
              key={item.id}
              className="rail-item"
              type="button"
              data-view={item.id}
              aria-current={view === item.id ? "true" : "false"}
              onClick={() => go(item.id)}
            >
              <Icon name={item.icon} />
              <span>{item.label}</span>
            </button>
          ))}
        </Fragment>
      ))}
      <div className="rail-foot">
        Lycoris Wallet v{VERSION}
        <br />
        {isMock ? "演示模式 · 数据不离开本机" : "密钥永远留在本机"}
      </div>
    </nav>
  );
}

/** Auto-lock countdown + periodic session touch + background refreshes. */
export function useSessionTimers(enabled: boolean) {
  const { touch } = { touch: api.touchSession };
  const { reloadWallet, refreshStatus } = useSession();

  useEffect(() => {
    if (!enabled) return;
    let lastTouch = 0;
    const touchTimer = window.setInterval(() => {
      const now = Date.now();
      if (now - lastTouch >= TOUCH_INTERVAL) {
        lastTouch = now;
        void touch().catch(() => {});
      }
    }, 5_000);
    const refreshTimer = window.setInterval(() => {
      void reloadWallet().catch(() => {});
    }, AUTO_REFRESH_INTERVAL);
    return () => {
      window.clearInterval(touchTimer);
      window.clearInterval(refreshTimer);
    };
  }, [enabled, touch, reloadWallet, refreshStatus]);
}
