/** Root component: decides between onboarding / unlock / the authenticated
 *  shell and mounts the view router. Ported from ui/js/main.js boot logic. */

import { useEffect } from "react";

import { api, isMock, walletFull } from "../api";
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

  // Portfolio refresh on entering the dashboard; mirrors the old effect.
  useEffect(() => {
    if (view === "dashboard") void refreshPortfolio();
  }, [view, refreshPortfolio]);

  if (!wallet) return null;
  const fullBleed = view === "asset";
  return (
    <div id="app" data-view={view} className={fullBleed ? "view-asset" : undefined}>
      <Titlebar walletVisible />
      <div className="body">
        <Rail />
        <main className="stage" id="stage">
          <CurrentView />
        </main>
      </div>
      <span hidden aria-hidden data-status={status ? "ready" : "init"} />
    </div>
  );
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

  if (!wallet) {
    // Pre-unlock: the onboarding wizard (no vault yet) or the lock screen.
    return view === "onboarding" ? <OnboardingView /> : <UnlockView />;
  }
  if (view === "unlock") return <UnlockView />;
  return <AuthenticatedShell />;
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
