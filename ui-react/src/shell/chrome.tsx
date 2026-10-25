/** App chrome: titlebar (window controls, lock button, mock chip), the nav
 *  rail / mobile tab bar, and the auto-lock + session touch timers.
 *  Ported from ui/js/main.js renderRail/syncChrome/timers. */

import { useEffect, useState } from "react";

import { api, isMock } from "../api";
import { useSession, type ViewId } from "./session";
import { Icon } from "../ui/Icon";

const VERSION = "0.3.0";
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
  const { wallet, lockNow } = useSession();

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
        {wallet ? `${wallet.label} · ${wallet.fingerprint}` : ""}
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
  const { view, go } = useSession();
  const compact = useCompact();

  if (compact) {
    const tabs: Array<{ id: ViewId; label: string; icon: string }> = [
      { id: "dashboard", label: "总览", icon: "dashboard" },
      { id: "receive", label: "收款", icon: "receive" },
      { id: "send", label: "转账", icon: "send" },
      { id: "activity", label: "记录", icon: "activity" },
      { id: "wallets", label: "钱包", icon: "wallet" },
    ];
    return (
      <nav className="tabbar" aria-label="页面导航">
        {tabs.map((tab) => (
          <button
            key={tab.id}
            className={`tab-item${view === tab.id ? " active" : ""}`}
            type="button"
            onClick={() => go(tab.id)}
          >
            <Icon name={tab.icon} />
            <span>{tab.label}</span>
          </button>
        ))}
      </nav>
    );
  }

  return (
    <aside className="rail">
      <nav className="rail-nav">
        {RAIL_GROUPS.map((group) => (
          <div key={group.label} className="rail-group">
            <div className="rail-label">{group.label}</div>
            {group.items.map((item) => (
              <button
                key={item.id}
                className={`rail-item${view === item.id ? " active" : ""}`}
                type="button"
                onClick={() => go(item.id)}
              >
                <Icon name={item.icon} />
                <span>{item.label}</span>
              </button>
            ))}
          </div>
        ))}
      </nav>
      <div className="rail-foot">Lycoris Wallet v{VERSION}</div>
    </aside>
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
