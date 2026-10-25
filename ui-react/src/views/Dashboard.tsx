/** Dashboard: total balance, asset rows, quick actions, wallet warnings.
 *  Ported from ui/js/views/dashboard.js. */

import { useSession } from "../shell/session";
import { useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { formatFiat, formatRelative, formatTotal, groupDigits, shortAddress } from "../format";
import type { AssetBalance } from "../types";

function AssetRow({
  asset,
  currency,
  hide,
  onOpen,
}: {
  asset: AssetBalance;
  currency: string;
  hide: boolean;
  onOpen: () => void;
}) {
  const value = currency === "cny" ? asset.valueCny : asset.valueUsd;
  return (
    <div
      className="asset-row"
      data-chain={asset.chain}
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onOpen();
        }
      }}
    >
      <span className="asset-mark" style={{ ["--accent" as string]: asset.accent }}>
        {asset.symbol.slice(0, 2)}
      </span>
      <div>
        <div className="asset-name">
          {asset.name} <span className="muted small">{asset.symbol}</span>
        </div>
        <div className="asset-sub">{shortAddress(asset.address, 12, 6)}</div>
      </div>
      <div />
      <div className="asset-balance">
        <div className={hide ? "hide-balance" : ""}>
          {groupDigits(asset.amount)} <span className="muted small">{asset.symbol}</span>
        </div>
        {asset.error ? (
          <div className="asset-error">{asset.error}</div>
        ) : (
          <div className="fiat">{formatFiat(value, currency)}</div>
        )}
      </div>
    </div>
  );
}

export function DashboardView() {
  const { wallet, portfolio, portfolioLoading, go, refreshPortfolio } = useSession();
  const toast = useToast();

  const settings = wallet?.settings;
  const currency = settings?.fiat ?? "usd";
  const hide = Boolean(settings?.hideBalances);
  const loading = portfolioLoading && !portfolio;
  const total = portfolio ? (currency === "cny" ? portfolio.totalCny : portfolio.totalUsd) : null;
  const warnings = portfolio?.warnings ?? [];

  const rowSkeletons = Array.from({ length: wallet?.accounts?.length ?? 10 });

  return (
    <section className="view">
      <div className="hero">
        <div className="label">总资产</div>
        <div className="total">
          <span className={hide ? "hide-balance" : ""}>{formatTotal(total, currency)}</span>
          <span className="cur">{currency === "cny" ? "CNY" : "USD"}</span>
        </div>
        <div className="meta">
          <span>{wallet?.label ?? ""}</span>
          <span className="fingerprint">{wallet?.fingerprint ?? ""}</span>
          {wallet?.mode === "passphrase" ? (
            <span className="chip chip-warn" title="口令钱包的强度完全取决于这句话">
              口令模式{wallet.derivation === "strict" ? " · 严格" : " · 隔离"}
            </span>
          ) : wallet?.mode === "private_key" ? (
            <span className="chip chip-warn" title="钥匙环:导入的原始私钥直接管理资产">
              私钥导入 · {wallet.accounts.length} 条链
            </span>
          ) : wallet?.mode === "aggregate" ? (
            <span className="chip chip-warn" title="聚合钱包:每把私钥独立管理一个地址">
              聚合 · {wallet.aggregateEntries?.length ?? 0} 把钥匙
            </span>
          ) : (
            <span className="chip" title="标准助记词派生">
              助记词 · BIP-39
            </span>
          )}
          {(wallet?.wallets?.length ?? 1) > 1 ? <span>共 {wallet?.wallets.length} 个钱包</span> : null}
          <span>{portfolio ? `更新于 ${formatRelative(portfolio.fetchedAt)}` : "正在同步…"}</span>
        </div>
        <div className="hero-actions">
          <button className="btn btn-primary" type="button" onClick={() => go("receive")}>
            <Icon name="receive" /> 收款
          </button>
          <button className="btn" type="button" onClick={() => go("send")}>
            <Icon name="send" /> 转账
          </button>
          <button
            className="btn btn-ghost"
            type="button"
            disabled={portfolioLoading}
            onClick={async () => {
              try {
                await refreshPortfolio(true);
                toast("余额已更新", "success");
              } catch (error) {
                toast((error as Error).message, "error");
              }
            }}
          >
            <Icon name="refresh" /> 刷新余额
          </button>
          <button className="btn btn-ghost" type="button" onClick={() => go("wallets")}>
            <Icon name="wallet" /> 钱包管理
          </button>
        </div>
      </div>

      {wallet?.mode === "passphrase" ? (
        <div className="note note-warn" style={{ marginTop: 16 }}>
          <Icon name="warn" />
          <div>口令钱包的安全性等于这句话的强度。请不要使用歌词、名言、短句，建议使用只有你自己知道的长句子。</div>
        </div>
      ) : null}

      {wallet?.mode === "private_key" ? (
        <div className="note note-warn" style={{ marginTop: 16 }}>
          <Icon name="warn" />
          <div>
            私钥钱包由钥匙环里的原始私钥直接管理资产(当前 {wallet.accounts.length} 条链)。
            可在 设置 → 钥匙环 随时补加 Solana / Monero 槽;私钥泄露即全部资产泄露,请离线备份。
          </div>
        </div>
      ) : null}

      {warnings.length ? (
        <div className="note note-warn" style={{ marginTop: 12 }}>
          <Icon name="info" />
          <div>
            {warnings.map((line, index) => (
              <span key={index}>
                {line}
                {index < warnings.length - 1 ? <br /> : null}
              </span>
            ))}
          </div>
        </div>
      ) : null}

      <div className="card card-pad" style={{ marginTop: 18 }}>
        <div className="section-title">
          <h2>资产</h2>
          <span className="hint">点击任意一行查看收款地址</span>
        </div>
        <div className="asset-list">
          {loading
            ? rowSkeletons.map((_, index) => (
                <div className="asset-row" key={index}>
                  <div className="skeleton" style={{ width: 32, height: 32, borderRadius: "50%" }} />
                  <div>
                    <div className="skeleton" style={{ width: 96, height: 14 }} />
                    <div className="skeleton" style={{ width: 150, height: 11, marginTop: 6 }} />
                  </div>
                  <div />
                  <div className="skeleton" style={{ width: 96, height: 16 }} />
                </div>
              ))
            : (portfolio?.assets ?? []).map((asset) => (
                <AssetRow
                  key={asset.chain}
                  asset={asset}
                  currency={currency}
                  hide={hide}
                  onOpen={() => go("asset", { chain: asset.chain })}
                />
              ))}
        </div>
      </div>
    </section>
  );
}
