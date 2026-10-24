/** 资产详情：总览的子页面。显示单个资产的余额、价格，以及该链在
 * 基础账户与所有派生账户下的地址。点击地址行即切换选中；在这里添加
 * 派生、查看地址详情，再从具体地址进入收款或转账。 */

import { api } from "../api.js?v=23";
import { escapeHtml, formatFiat, groupDigits, shortAddress } from "../format.js?v=23";
import { askPassword, bindCopy, icon, openModal, setBusy, toast } from "../ui.js?v=23";

const STRATEGY_LABELS = {
  path: { title: "路径派生", desc: "BIP-32 地址索引递增（助记词）" },
  salted: { title: "加盐派生", desc: "H(域名‖密语‖#序号)" },
  iterated: { title: "迭代哈希", desc: "基于基础私钥连续哈希 N 次" },
};

/** 派生策略按钱包模式过滤：助记词才有路径派生，口令钱包只有哈希类。 */
function strategiesFor(wallet) {
  const all = Object.entries(STRATEGY_LABELS);
  if (!wallet || wallet.mode === "mnemonic") return all;
  return all.filter(([key]) => key !== "path");
}

export function render(ctx) {
  const chainId = ctx.state.params.chain;
  const wallet = ctx.state.wallet;
  const portfolio = ctx.state.portfolio;
  const settings = wallet?.settings ?? { fiat: "usd", hideBalances: false };
  const currency = settings.fiat ?? "usd";
  const hide = Boolean(settings.hideBalances);
  const asset = portfolio?.assets?.find((item) => item.chain === chainId);
  const chainAccount = wallet?.accounts?.find((item) => item.chain === chainId);
  const name = asset?.name ?? chainAccount?.name ?? chainId;
  const symbol = asset?.symbol ?? chainAccount?.symbol ?? "";
  const accent = asset?.accent ?? chainAccount?.accent ?? "#b3553f";
  const value = asset ? (currency === "cny" ? asset.valueCny : asset.valueUsd) : null;
  const error = asset?.error ?? null;
  const amount = asset?.amount ?? null;

  return {
    html: `<section class="view">
      <div class="page-head">
        <div>
          <button class="btn btn-ghost btn-sm" type="button" data-action="back">${icon("arrow-left")}返回总览</button>
        </div>
      </div>

      <div class="card card-pad asset-hero" style="margin-top:10px">
        <div class="asset-hero-main">
          <span class="asset-mark asset-mark-lg" style="--accent:${accent}">${escapeHtml(
            symbol.slice(0, 2),
          )}</span>
          <div>
            <div class="asset-name">${escapeHtml(name)}
              <span class="muted small">${escapeHtml(symbol)}</span></div>
            <div class="total asset-total">
              <span class="${hide ? "hide-balance" : ""}">${
                amount != null ? groupDigits(amount) : "—"
              } <span class="cur">${escapeHtml(symbol)}</span></span>
            </div>
            ${
              error
                ? `<div class="asset-error">${escapeHtml(error)}</div>`
                : `<div class="fiat">${formatFiat(value, currency)}</div>`
            }
          </div>
        </div>
        ${
          asset?.priceUsd
            ? `<div class="asset-price muted small">单价 ${formatFiat(
                currency === "cny" ? asset.priceCny : asset.priceUsd,
                currency,
              )}</div>`
            : ""
        }
      </div>

      <div class="card card-pad" style="margin-top:16px">
        <div class="section-title">
          <h2>派生地址</h2>
          <span class="hint">点击行切换账户 · 收款、转账与详情跟随所选账户</span>
        </div>
        <div id="addr-list">
          <div class="skeleton" style="height:64px"></div>
          <div class="skeleton" style="height:64px;margin-top:8px"></div>
        </div>
        <div class="row" style="gap:8px;margin-top:14px">
          <button class="btn" type="button" id="addr-add-derived">${icon("layers")}添加派生地址</button>
          <span class="hint">按需派生：只有添加了的地址才会参与余额查询与总览合计</span>
        </div>
      </div>
    </section>`,

    mount(root) {
      root.querySelector('[data-action="back"]')?.addEventListener("click", () => ctx.go("dashboard"));

      /* 派生地址列表 */
      const list = root.querySelector("#addr-list");
      const paintAddresses = (rows) => {
        if (!rows.length) {
          list.innerHTML = `<p class="hint">暂无地址</p>`;
          return;
        }
        list.innerHTML = rows
          .map(
            (row) => `<div class="addr-row ${row.active ? "is-active" : ""}" data-id="${escapeHtml(
              row.id ?? "",
            )}" role="button" tabindex="0" title="点击切换到该账户">
            <div class="addr-main">
              <div class="addr-label">${escapeHtml(row.label)}
                ${row.active ? '<span class="chip chip-ok">当前</span>' : ""}</div>
              <div class="addr-mono">${escapeHtml(shortAddress(row.address, 18, 12))}</div>
              <div class="addr-path muted small">${escapeHtml(row.path)}</div>
            </div>
            <div class="addr-actions">
              <button class="btn btn-ghost btn-sm" type="button" data-copy="${escapeHtml(
                row.address,
              )}" title="复制地址">${icon("copy")}</button>
              <button class="btn btn-ghost btn-sm" type="button" data-act="detail" title="查看地址详情">${icon("info")}</button>
              ${
                row.id
                  ? `<button class="btn btn-ghost btn-sm" type="button" data-act="manage" title="重命名 / 删除该派生账户">${icon("more")}</button>`
                  : ""
              }
              <button class="btn btn-sm" type="button" data-act="receive">收款</button>
              <button class="btn btn-sm" type="button" data-act="send">转账</button>
            </div>
          </div>`,
          )
          .join("");
        bindCopy(list);
        list.querySelectorAll(".addr-row").forEach((row) => {
          const id = row.dataset.id || null;
          // 点击行或回车即选中该账户；按钮事件在下面单独拦截。
          const select = async () => {
            if ((ctx.state.wallet?.activeBatch ?? null) === id) return;
            try {
              ctx.setWallet(await api.selectBatchAccount(id));
              toast("已切换派生账户", "success");
              paintAddresses(rows.map((r) => ({ ...r, active: (r.id ?? null) === id })));
            } catch (error) {
              toast(error.message, "error");
            }
          };
          row.addEventListener("click", select);
          row.addEventListener("keydown", (event) => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              void select();
            }
          });
          const stop = (selector, handler) => {
            row.querySelector(selector)?.addEventListener("click", (event) => {
              event.stopPropagation();
              handler(event);
            });
          };
          stop('[data-act="receive"]', () => void withAccount(ctx, id, chainId, "receive"));
          stop('[data-act="send"]', () => void withAccount(ctx, id, chainId, "send"));
          stop('[data-act="detail"]', () => void detailFlow(ctx, chainId, id));
          stop('[data-act="manage"]', () => void manageFlow(ctx, id, () => paintAddresses(rows)));
        });
      };

      api
        .chainAddresses(chainId)
        .then(paintAddresses)
        .catch((error) => {
          list.innerHTML = `<div class="note note-warn">${icon("warn")}<div>${escapeHtml(
            error.message,
          )}</div></div>`;
        });

      /* 添加派生地址（针对这条链所属的钱包，一次添加后全部链生效） */
      root.querySelector("#addr-add-derived")?.addEventListener("click", () => {
        addDerivedFlow(ctx);
      });
    },
  };
}

/** 确保切到目标账户后再跳转（收款/转账按钮）。 */
async function withAccount(ctx, id, chainId, target) {
  try {
    if ((ctx.state.wallet?.activeBatch ?? null) !== id) {
      ctx.setWallet(await api.selectBatchAccount(id));
    }
    ctx.go(target, { chain: chainId });
  } catch (error) {
    toast(error.message, "error");
  }
}

/** 地址详情：验证密码后展示派生信息、地址、公钥、私钥。 */
async function detailFlow(ctx, chainId, id) {
  const password = await askPassword({
    title: "查看地址详情",
    description: "详情包含该地址的私钥。验证本机密码后显示，请确认周围没有人。",
    confirmText: "查看",
  });
  if (!password) return; // 用户取消
  let detail;
  try {
    detail = await api.accountDetail(chainId, id, password);
  } catch (error) {
    toast(error.message, "error");
    return;
  }
  openModal(detailMarkup(detail), {
    onMount(element) {
      bindCopy(element, (ok) => toast(ok ? "已复制" : "复制失败", ok ? "success" : "error"));
      // 链尾的「私钥」框内置眼睛图标,点它才露出真实私钥。
      const reveal = element.querySelector("#detail-reveal");
      const finalText = element.querySelector(".key-chain-final-text");
      const chainFinal = element.querySelector(".key-chain-final");
      reveal?.addEventListener("click", () => {
        const showing = chainFinal.classList.toggle("is-revealed");
        finalText.textContent = showing ? detail.privateKey : "私钥";
        reveal.innerHTML = icon(showing ? "eyeOff" : "eye");
        reveal.title = showing ? "隐藏私钥" : "显示私钥";
        reveal.setAttribute("aria-label", reveal.title);
      });
    },
  });
}

function maskText(value) {
  return "•".repeat(Math.min(Math.max(value.length, 12), 48));
}

function detailRow(label, value, mono = true) {
  return `<div class="detail-row">
    <span class="detail-key">${label}</span>
    <span class="detail-value ${mono ? "mono" : ""}">${escapeHtml(value)}</span>
  </div>`;
}

function detailMarkup(detail) {
  // 三段式来源链:密语 → 公式 → 私钥,折叠默认收起;私钥框点「显示」露出。
  const source = `<div class="key-chain">
      <span class="key-chain-box mono">${escapeHtml(detail.sourcePreview)}</span>
      <span class="key-chain-arrow">↓</span>
      <span class="key-chain-box key-chain-formula mono">${escapeHtml(detail.formula)}</span>
      <span class="key-chain-arrow">↓</span>
      <span class="key-chain-box key-chain-final mono">
        <span class="key-chain-final-text">私钥</span>
        <button class="key-chain-eye" type="button" id="detail-reveal" title="显示私钥" aria-label="显示私钥">${icon("eye")}</button>
      </span>
    </div>`;
  return `<h2>地址详情</h2>
    <p class="hint" style="margin:4px 0 12px">${escapeHtml(detail.chainName)}（${escapeHtml(
      detail.symbol,
    )}）· ${escapeHtml(detail.label)}</p>
    <div class="detail-card">
      ${detailRow("派生账户", detail.label, false)}
      ${detailRow("派生路径", detail.path)}
      ${detailRow("派生方式", detail.derivation, false)}
      ${detailRow("哈希算法", detail.hashAlgorithm, false)}
      ${detailRow("地址格式", detail.addressFormat, false)}
      ${detailRow("地址", detail.address)}
      ${detailRow("公钥", detail.publicKey)}
    </div>
    <details class="detail-source" style="margin-top:12px">
      <summary>${icon("info")}私钥信息<span class="hint">（点击展开）</span></summary>
      ${source}
    </details>
    <div class="note note-warn" style="margin-top:12px">${icon("warn")}<div>${escapeHtml(
      detail.warning,
    )}</div></div>
    <div class="modal-foot">
      <button class="btn btn-ghost" data-copy="${escapeHtml(detail.privateKey)}" type="button">${icon(
        "copy",
      )}复制私钥</button>
      <button class="btn" data-close type="button">关闭</button>
    </div>`;
}

/** 派生账户管理（重命名 / 删除）：从资产页地址行的「⋯」进入。 */
async function manageFlow(ctx, id, refresh) {
  const wallet = ctx.state.wallet;
  const account = (wallet?.batchAccounts ?? []).find((item) => item.id === id);
  if (!account) return;
  openModal(
    `<h2>派生账户「${escapeHtml(account.label)}」</h2>
     <p class="hint" style="margin:6px 0 12px">${escapeHtml(
       strategyTitle(account.strategy),
     )} · 序号 ${account.index}${account.fingerprint ? ` · 指纹 ${escapeHtml(account.fingerprint)}` : ""}</p>
     <div class="field" style="margin-bottom:4px">
       <label for="manage-label">名称</label>
       <input class="input" id="manage-label" type="text" maxlength="40" value="${escapeHtml(account.label)}" />
     </div>
     <p class="error-text" id="manage-error" hidden></p>
     <div class="modal-foot" style="justify-content:space-between">
       <button class="btn btn-ghost" data-manage="remove" type="button" style="color:var(--danger, #c0392b)">${icon("trash")}删除</button>
       <span class="row" style="gap:10px">
         <button class="btn btn-ghost" data-close type="button">取消</button>
         <button class="btn btn-primary" data-manage="save" type="button">保存</button>
       </span>
     </div>`,
    {
      onMount(element, close) {
        const input = element.querySelector("#manage-label");
        const error = element.querySelector("#manage-error");
        element.querySelector('[data-manage="save"]').addEventListener("click", async () => {
          const value = input.value.trim();
          if (!value) {
            error.hidden = false;
            error.textContent = "名称不能为空";
            return;
          }
          try {
            ctx.setWallet(await api.renameBatchAccount(id, value));
            close();
            refresh();
            toast("名称已更新", "success");
          } catch (failure) {
            error.hidden = false;
            error.textContent = failure.message;
          }
        });
        input.addEventListener("keydown", (event) => {
          if (event.key === "Enter") element.querySelector('[data-manage="save"]').click();
        });
        element.querySelector('[data-manage="remove"]').addEventListener("click", async () => {
          try {
            const view = await api.removeBatchAccount(id);
            close();
            ctx.setWallet(view);
            // 被删的若是当前账户，后端已回落基础账户；重挂资产页刷新地址列表。
            ctx.go("asset", { chain: ctx.state.params.chain });
            toast("派生账户已移除", "success");
          } catch (failure) {
            error.hidden = false;
            error.textContent = failure.message;
          }
        });
      },
    },
  );
}

function strategyTitle(strategy) {
  return STRATEGY_LABELS[strategy]?.title ?? strategy;
}

/** 在资产页内直接添加派生账户的小弹窗（复用钱包管理页的派生策略）。 */
function addDerivedFlow(ctx) {
  const isMnemonic = !ctx.state.wallet || ctx.state.wallet.mode === "mnemonic";
  const local = {
    strategy: isMnemonic ? "path" : "salted",
    count: 1,
    // 迭代哈希基于基础私钥,0 号会与基础账户重合,从 1 起。
    start: 1,
    busy: false,
  };
  openModal(`<div id="asset-derived-body"></div>`, {
    onMount(element, close) {
      const body = element.querySelector("#asset-derived-body");
      const paint = () => {
        body.innerHTML = markup(local);
        bind(body, local, { ctx, close, paint });
      };
      paint();
    },
  });

  function markup(local) {
    return `<h2>添加派生地址</h2>
      <p class="hint" style="margin:6px 0 12px">为当前钱包追加派生账户；只记录「派生方式 + 序号」，不保存额外密钥。</p>
      <div class="field">
        <span class="field-label">派生方式</span>
        <div class="seg seg-col" role="group">
          ${strategiesFor(ctx.state.wallet)
            .map(
              ([key, meta]) =>
                `<button type="button" data-strategy="${key}" aria-pressed="${local.strategy === key}">
                   <span class="seg-title">${meta.title}</span><span class="seg-desc">${meta.desc}</span>
                 </button>`,
            )
            .join("")}
        </div>
      </div>
      <div class="row" style="gap:12px">
        <div class="field" style="flex:1">
          <label for="derived-count">数量（1–${maxCount(local)}）</label>
          <input class="input" id="derived-count" type="number" min="1" max="${maxCount(local)}" value="${local.count}" />
        </div>
        <div class="field" style="flex:1">
          <label for="derived-start">起始序号${local.strategy === "iterated" ? "（≥1）" : ""}</label>
          <input class="input" id="derived-start" type="number" min="${local.strategy === "iterated" ? 1 : 0}" max="${maxCount(local) - 1}" value="${local.start}" />
        </div>
      </div>
      <div class="modal-foot">
        <button class="btn btn-ghost" data-close type="button">取消</button>
        <button class="btn btn-primary" id="derived-save" type="button">添加</button>
      </div>`;
  }

  function maxCount(local) {
    return local.strategy === "iterated" ? 50 : 100;
  }

  function bind(body, local, { ctx, close, paint }) {
    body.querySelectorAll("[data-strategy]").forEach((button) => {
      button.addEventListener("click", () => {
        local.strategy = button.dataset.strategy;
        const max = maxCount(local);
        local.count = Math.min(local.count, max);
        // 迭代哈希 0 号即基础账户,切换时把起始序号抬到 1。
        if (local.strategy === "iterated") local.start = Math.max(1, local.start);
        local.start = Math.min(local.start, max - 1);
        paint();
      });
    });
    body.querySelector("#derived-count").addEventListener("input", (event) => {
      const value = Number(event.target.value);
      if (Number.isFinite(value)) {
        local.count = Math.max(1, Math.min(maxCount(local), Math.floor(value)));
      }
    });
    body.querySelector("#derived-start").addEventListener("input", (event) => {
      const value = Number(event.target.value);
      if (Number.isFinite(value)) {
        // 迭代哈希从 1 起:0 号等于基础账户本身。
        const min = local.strategy === "iterated" ? 1 : 0;
        local.start = Math.max(min, Math.min(maxCount(local) - 1, Math.floor(value)));
      }
    });
    body.querySelector("#derived-save").addEventListener("click", async (event) => {
      if (local.busy) return;
      local.busy = true;
      const button = event.currentTarget;
      setBusy(button, true, "派生中…");
      try {
        const view = await api.addWalletsBatch({
          batch: { strategy: local.strategy, count: local.count, start: local.start },
          name: null,
        });
        ctx.setWallet(view);
        close();
        toast(`已添加 ${local.count} 个派生地址`, "success");
        // 重新渲染资产子页，新地址立即出现在列表里。
        ctx.go("asset", { chain: ctx.state.params.chain });
      } catch (error) {
        local.busy = false;
        setBusy(button, false);
        toast(error.message, "error");
      }
    });
  }
}
