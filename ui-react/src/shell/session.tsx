/** Application shell state: the session store + router context.
 *
 * Mirrors the state object and ctx helpers of ui/js/main.js but exposed as a
 * React context so views consume it with hooks instead of a mutable bag. */

import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type ReactNode,
} from "react";

import { api, applyVisibleChains, isMock, walletFull } from "../api";
import type { Portfolio, VaultStatus, WalletView } from "../types";

export type ViewId =
  | "onboarding"
  | "unlock"
  | "dashboard"
  | "asset"
  | "receive"
  | "send"
  | "activity"
  | "contacts"
  | "settings"
  | "wallets";

export interface SessionState {
  view: ViewId;
  params: Record<string, unknown>;
  wallet: WalletView | null;
  status: VaultStatus | null;
  portfolio: Portfolio | null;
  portfolioLoading: boolean;
  lastSendChain: string | null;
  lastReceiveChain: string | null;
}

export interface AppActions {
  go: (view: ViewId, params?: Record<string, unknown>) => void;
  setWallet: (wallet: WalletView) => void;
  /** 重新拉取钱包（可选保留当前派生选择之外的刷新）。 */
  reloadWallet: () => Promise<void>;
  refreshPortfolio: (force?: boolean) => Promise<Portfolio | null>;
  refreshStatus: () => Promise<VaultStatus>;
  lockNow: () => Promise<void>;
}

const fallbackStatus: VaultStatus = {
  exists: false,
  locked: true,
  hasHint: false,
  hint: null,
  label: null,
};

const SessionContext = createContext<(SessionState & AppActions) | null>(null);

export function useSession(): SessionState & AppActions {
  const ctx = useContext(SessionContext);
  if (!ctx) throw new Error("useSession outside SessionProvider");
  return ctx;
}

const PORTFOLIO_TTL = 60; // seconds

export function SessionProvider({
  children,
  onLock,
}: {
  children: ReactNode;
  onLock: () => void;
}) {
  const [state, setState] = useState<SessionState>({
    view: "unlock",
    params: {},
    wallet: null,
    status: null,
    portfolio: null,
    portfolioLoading: false,
    lastSendChain: null,
    lastReceiveChain: null,
  });
  let portfolioFetch: Promise<Portfolio> | null = null;
  let lastPortfolioAt = 0;

  const go = useCallback((view: ViewId, params: Record<string, unknown> = {}) => {
    setState((s) => ({ ...s, view, params: { ...params } }));
  }, []);

  const setWallet = useCallback((wallet: WalletView) => {
    setState((s) => ({
      ...s,
      wallet: applyVisibleChains(wallet, wallet.settings?.visibleChains ?? []),
    }));
  }, []);

  const reloadWallet = useCallback(async () => {
    const wallet = await api.wallet();
    setWallet(wallet);
  }, [setWallet]);

  const refreshPortfolio = useCallback(
    async (force = false): Promise<Portfolio | null> => {
      if (!force && Date.now() / 1000 - lastPortfolioAt < PORTFOLIO_TTL && state.portfolio) {
        return state.portfolio;
      }
      if (portfolioFetch) return portfolioFetch;
      setState((s) => ({ ...s, portfolioLoading: true }));
      portfolioFetch = api
        .refreshPortfolio()
        .then((portfolio) => {
          lastPortfolioAt = Date.now() / 1000;
          setState((s) => ({ ...s, portfolio, portfolioLoading: false }));
          return portfolio;
        })
        .catch(() => {
          setState((s) => ({ ...s, portfolioLoading: false }));
          portfolioFetch = null;
          return null as unknown as Portfolio;
        })
        .finally(() => {
          portfolioFetch = null;
        });
      return portfolioFetch;
    },
    [state.portfolio],
  );

  const refreshStatus = useCallback(async (): Promise<VaultStatus> => {
    try {
      const status = await api.vaultStatus();
      setState((s) => ({ ...s, status }));
      return status;
    } catch {
      setState((s) => ({ ...s, status: fallbackStatus }));
      return fallbackStatus;
    }
  }, []);

  const lockNow = useCallback(async () => {
    try {
      await api.lockVault();
    } catch {
      // demo mode may not implement locking; still drop to the lock screen.
    }
    if (isMock) await walletFull().catch(() => {});
    setState((s) => ({
      ...s,
      view: "unlock",
      params: {},
      wallet: null,
      portfolio: null,
    }));
    onLock();
  }, [onLock]);

  const value = useMemo(
    () => ({ ...state, go, setWallet, reloadWallet, refreshPortfolio, refreshStatus, lockNow }),
    [state, go, setWallet, reloadWallet, refreshPortfolio, refreshStatus, lockNow],
  );

  return <SessionContext.Provider value={value}>{children}</SessionContext.Provider>;
}
