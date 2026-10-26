/** Root component: decides between onboarding / unlock / the authenticated
 *  shell and mounts the view router. Ported from ui/js/main.js boot logic. */

import { useEffect } from "react";

import { api, isMock, walletFull } from "../api";
import { applyTheme } from "../boot";
import type { WalletView } from "../types";
import { SessionProvider, useSession } from "./session";
import { Rail, Titlebar, useSessionTimers } from "./chrome";
import { OnboardingView, UnlockView } from "./entry";
import { ModalHostProvider, ToastProvider } from "../ui/overlay";
import { DashboardView } from "../views/Dashboard";
import { WalletsView } from "../views/Wallets";
import { SettingsView } from "../views/Settings";
import { ActivityView } from "../views/Activity";
import { ContactsView } from "../views/Contacts";
import { ToolsView } from "../views/Tools";
import { ReceiveView } from "../views/Receive";
import { SendView } from "../views/Send";
import { AssetView } from "../views/Asset";

function CurrentView() {
  const { view } = useSession();
  switch (view) {
    case "dashboard":
      return <DashboardView />;
    case "wallets":
      return <WalletsView />;
    case "settings":
      return <SettingsView />;
    case "activity":
      return <ActivityView />;
    case "contacts":
      return <ContactsView />;
    case "tools":
      return <ToolsView />;
    case "receive":
      return <ReceiveView />;
    case "send":
      return <SendView />;
    case "asset":
      return <AssetView />;
    default:
      return <DashboardView />;
  }
}

function AuthenticatedShell() {
  const { wallet, view, refreshPortfolio } = useSession();
  useSessionTimers(Boolean(wallet));

  // 旧版在每次 setWallet 后 applyTheme()：主题跟随钱包设置（解锁后/保存后都走这里）。
  useEffect(() => {
    applyTheme(wallet?.settings?.theme ?? "system");
  }, [wallet?.settings?.theme]);

  // Portfolio refresh on entering the dashboard; mirrors the old effect.
  useEffect(() => {
    if (view === "dashboard") void refreshPortfolio();
  }, [view, refreshPortfolio]);

  if (!wallet) return null;
  return <CurrentView />;
}

function Router() {
  const { wallet, status, view, go, setWallet } = useSession();

  // Boot: decide onboarding vs unlock once the vault status is known. In demo
  // mode the vault may not exist yet, so probe the wallet directly first.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (wallet) return;
      const current = await walletFull().catch(() => null);
      if (cancelled) return;
      if (current) {
        setWallet(current as WalletView);
        go("dashboard");
        return;
      }
      const st = status ?? (await api.vaultStatus().catch(() => null));
      if (cancelled) return;
      if (!st || !st.exists) go("onboarding");
      else go("unlock");
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 旧版 #app 骨架（titlebar + shell）是静态 HTML,永远存在;引导/解锁页也渲染
  // 在 .stage 里,syncChrome() 只按状态隐藏 rail 与锁按钮。这里保持同构:否则
  // 预解锁状态没有窗口控制按钮(关闭/最大化),.onboarding 的 min-height 居中链也会断。
  const preUnlock = !wallet && (view === "onboarding" || view === "unlock");
  return (
    <div id="app" data-view={view}>
      <Titlebar walletVisible={!preUnlock} />
      <div className="shell">
        <Rail />
        <main className="stage" id="stage">
          {wallet ? (
            <AuthenticatedShell />
          ) : preUnlock ? (
            view === "onboarding" ? (
              <OnboardingView />
            ) : (
              <UnlockView />
            )
          ) : (
            /* boot 探测中:与旧版 index.html 的静态 boot 屏一致 */
            <div className="boot">
              <div className="boot-flower" aria-hidden="true" />
              <p>正在启动 Lycoris Wallet…</p>
            </div>
          )}
        </main>
      </div>
    </div>
  );
}

export function App() {
  // Demo mode has no window controls; guard against a missing modal root.
  useEffect(() => {
    if (!document.getElementById("modal-root")) {
      const root = document.createElement("div");
      root.id = "modal-root";
      document.body.appendChild(root);
    }
  }, []);

  return (
    <ToastProvider>
      <ModalHostProvider>
        <SessionProvider onLock={() => {}}>
          <Router />
        </SessionProvider>
      </ModalHostProvider>
    </ToastProvider>
  );
}

export const IS_MOCK = isMock;
