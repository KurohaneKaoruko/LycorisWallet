/** Dashboard: total balance, asset rows, quick actions, wallet warnings. */

import {
  escapeHtml,
  formatFiat,
  formatTotal,
  groupDigits,
  relativeTime,
  shortAddress,
} from "../format.js?v=10";
import { icon, toast } from "../ui.js?v=10";

export function render(ctx) {
  const wallet = ctx.state.wallet;
  const portfolio = ctx.state.portfolio;
  const settings = wallet?.settings ?? { fiat: "usd", hideBalances: false };
  const currency = settings.fiat ?? "usd";
  const hide = Boolean(settings.hideBalances);
  const loading = ctx.state.portfolioLoading && !portfolio;

  const total = portfolio ? (currency === "cny" ? portfolio.totalCny : portfolio.totalUsd) : null;
  const assetRows = loading
    ? Array.from({ length: ctx.state.wallet?.accounts?.length ?? 10 })
        .map(
          () =>
            `<div class="asset-row"><div class="skeleton" style="width:32px;height:32px;border-radius:50%"></div>
             <div><div class="skeleton" style="width:96px;height:14px"></div><div class="skeleton" style="width:150px;height:11px;margin-top:6px"></div></div>
             <div></div><div class="skeleton" style="width:96px;height:16px"></div></div>`,
        )
        .join("")
    : (portfolio?.assets ?? [])
        .map((asset) => {
          const value = currency === "cny" ? asset.valueCny : asset.valueUsd;
          return `<div class="asset-row" data-chain="${asset.chain}" role="button" tabindex="0">
            <span class="asset-mark" style="--accent:${asset.accent}">${escapeHtml(
              asset.symbol.slice(0, 2),
            )}</span>
            <div>
              <div class="asset-name">${escapeHtml(asset.name)}
                <span class="muted small">${escapeHtml(asset.symbol)}</span></div>
              <div class="asset-sub">${escapeHtml(shortAddress(asset.address, 12, 6))}</div>
            </div>
            <div></div>
            <div class="asset-balance">
              <div class="${hide ? "hide-balance" : ""}">${groupDigits(asset.amount)} <span class="muted small">${escapeHtml(
                asset.symbol,
              )}</span></div>
              ${
                asset.error
                  ? `<div class="asset-error">${escapeHtml(asset.error)}</div>`
                  : `<div class="fiat">${formatFiat(value, currency)}</div>`
              }
            </div>
          </div>`;
        })
        .join("");

  const warnings = portfolio?.warnings ?? [];

  const modeChip =
    wallet?.mode === "passphrase"
      ? `<span class="chip chip-warn" title="口令钱包的强度完全取决于这句话">口令模式${wallet.derivation === "strict" ? " · 严格" : " · 隔离"}</span>`
      : `<span class="chip" title="标准助记词派生">助记词 · BIP-39</span>`;

  return {
    html: `<section class="view">
      <div class="hero">
        <div class="label">总资产</div>
        <div class="total">
          <span class="${hide ? "hide-balance" : ""}">${formatTotal(total, currency)}</span>
          <span class="cur">${currency === "cny" ? "CNY" : "USD"}</span>
        </div>
        <div class="meta">
          <span>${escapeHtml(wallet?.label ?? "")}</span>
          <span class="fingerprint">${escapeHtml(wallet?.fingerprint ?? "")}</span>
          ${modeChip}
          ${
            (wallet?.wallets?.length ?? 1) > 1
              ? `<span>共 ${wallet.wallets.length} 个钱包</span>`
              : ""
          }
          <span>${portfolio ? `更新于 ${relativeTime(portfolio.fetchedAt)}` : "正在同步…"}</span>
        </div>
        <div class="hero-actions">
          <button class="btn btn-primary" data-action="receive" type="button">
            <svg viewBox="0 0 24 24"><path d="M12 4v12"/><path d="m7.5 12 4.5 4.5L16.5 12"/><path d="M4.5 19.5h15"/></svg>
            收款
          </button>
          <button class="btn" data-action="send" type="button">
            <svg viewBox="0 0 24 24"><path d="M12 19.5V6.5"/><path d="m6.5 12 5.5-5.5L17.5 12"/></svg>
            转账
          </button>
          <button class="btn btn-ghost" data-action="refresh" type="button" ${
            ctx.state.portfolioLoading ? "disabled" : ""
          }>
            <svg viewBox="0 0 24 24"><path d="M19.6 12a7.6 7.6 0 1 1-2.3-5.4"/><path d="M19.8 4.6v4.2h-4.2"/></svg>
            刷新余额
          </button>
          <button class="btn btn-ghost" data-action="wallets" type="button">
            ${icon("wallet")}钱包管理
          </button>
        </div>
      </div>

      ${
        wallet?.mode === "passphrase"
          ? `<div class="note note-warn" style="margin-top:16px">
              <svg viewBox="0 0 24 24"><path d="M12 4.8 3.4 19.2h17.2L12 4.8Z"/><path d="M12 10v4"/><path d="M12 16.6h.01"/></svg>
              <div>口令钱包的安全性等于这句话的强度。请不要使用歌词、名言、短句，建议使用只有你自己知道的长句子。</div>
            </div>`
          : ""
      }

      ${
        warnings.length
          ? `<div class="note note-warn" style="margin-top:12px">
              <svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="8.4"/><path d="M12 11v5.4"/><path d="M12 7.8h.01"/></svg>
              <div>${warnings.map(escapeHtml).join("<br />")}</div>
            </div>`
          : ""
      }

      <div class="card card-pad" style="margin-top:18px">
        <div class="section-title">
          <h2>资产</h2>
          <span class="hint">点击任意一行查看收款地址</span>
        </div>
        <div class="asset-list">${assetRows}</div>
      </div>
    </section>`,

    mount(root) {
      root.querySelectorAll(".asset-row[data-chain]").forEach((row) => {
        const open = () => ctx.go("receive", { chain: row.dataset.chain });
        row.addEventListener("click", open);
        row.addEventListener("keydown", (event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            open();
          }
        });
      });
      root.querySelector('[data-action="receive"]')?.addEventListener("click", () => ctx.go("receive"));
      root.querySelector('[data-action="send"]')?.addEventListener("click", () => ctx.go("send"));
      root.querySelector('[data-action="wallets"]')?.addEventListener("click", () => ctx.go("wallets"));
      root.querySelector('[data-action="refresh"]')?.addEventListener("click", async () => {
        try {
          await ctx.refreshPortfolio({ force: true });
          toast("余额已更新", "success");
        } catch (error) {
          toast(error.message, "error");
        }
      });
    },
  };
}
