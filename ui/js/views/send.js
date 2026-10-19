/** Send: chain + recipient + amount, live fee quotes from Rust, then a
 *  confirmation dialog before broadcasting. */

import { api } from "../api.js?v=10";
import { copyText, escapeHtml, groupDigits, normalizeAmountInput, shortAddress } from "../format.js?v=10";
import { confirmDialog, debounce, enhanceTabScroll, icon, openModal, setBusy, toast } from "../ui.js?v=10";

export function render(ctx, params = {}) {
  const wallet = ctx.state.wallet;
  const local = {
    chain: params.chain ?? ctx.state.lastSendChain ?? "bitcoin",
    to: params.to ?? "",
    amount: params.amount ?? "",
    level: "normal",
    quote: null,
    quoting: false,
    quoteError: null,
    sending: false,
    result: null,
  };
  ctx.state.lastSendChain = local.chain;

  const accountFor = (id) => wallet.accounts.find((account) => account.chain === id);

  return {
    html: `<section class="view" id="send-view"></section>`,

    mount(root) {
      const view = root.querySelector("#send-view");
      const body = () => view.querySelector("#send-body");
      let disposeScroll = () => {};

      const paint = () => {
        if (local.result) {
          view.innerHTML = successMarkup(local);
          bindSuccess(view);
          return;
        }
        const account = accountFor(local.chain);
        const asset = ctx.state.portfolio?.assets.find((item) => item.chain === local.chain);
        const info = account;
        view.innerHTML = `
        <div class="page-head">
          <div>
            <h1>转账</h1>
            <div class="sub">金额与手续费都由本地 Rust 引擎计算，签名永远不离开设备。</div>
          </div>
          <button class="btn" data-action="go-receive" type="button">${icon("receive")}收款</button>
        </div>

        <div class="tabs" role="tablist" style="margin-bottom:16px">
          ${wallet.accounts
            .map(
              (item) => `<button class="tab" role="tab" data-chain="${item.chain}" aria-selected="${
                item.chain === local.chain
              }" ${item.sendSupported === false ? 'data-coming="1" title="转账即将支持"' : ""}><span class="dot" style="--accent:${item.accent}"></span>${escapeHtml(item.symbol)}${
                item.sendSupported === false ? '<span class="tab-soon">即将</span>' : ""
              }</button>`,
            )
            .join("")}
        </div>

        <div class="send-layout">
          <div class="card card-pad stack" id="send-body">
            <div class="field">
              <div class="row-between">
                <label for="to">收款地址</label>
                <button class="btn btn-ghost btn-sm" data-action="pick-contact" type="button">${icon("contacts")}从联系人选择</button>
              </div>
              <input class="input mono" id="to" placeholder="${escapeHtml(
                placeholderFor(local.chain),
              )}" spellcheck="false" autocomplete="off" value="${escapeHtml(local.to)}" />
            </div>

            <div class="field">
              <div class="row-between">
                <label for="amount">金额</label>
                <span class="hint">可用 ${groupDigits(asset?.amount ?? "0")} ${escapeHtml(
                  info.symbol,
                )}${asset?.valueUsd ? ` · ≈ $${asset.valueUsd.toFixed(2)}` : ""}</span>
              </div>
              <div class="amount-input">
                <input id="amount" inputmode="decimal" placeholder="0.0" value="${escapeHtml(
                  local.amount,
                )}" autocomplete="off" />
                <span class="unit">${escapeHtml(info.symbol)}</span>
                <button class="btn btn-ghost btn-sm" data-action="max" type="button">全部</button>
              </div>
            </div>

            <div class="field">
              <span class="field-label">手续费</span>
              <div class="fee-list" id="fee-list">${feeMarkup(local)}</div>
              <p class="hint" id="fee-foot">${escapeHtml(local.quote?.note ?? info.hashAlgorithm ?? "")}</p>
            </div>

            ${local.quoteError ? `<div class="note note-danger">${icon("warn")}<div>${escapeHtml(local.quoteError)}</div></div>` : ""}
            ${local.quote?.warning ? `<div class="note note-warn">${icon("warn")}<div>${escapeHtml(local.quote.warning)}</div></div>` : ""}

            <div class="row" style="justify-content:flex-end">
              <button class="btn btn-primary" id="review" type="button" ${
                local.quote?.canSend && !local.quoting ? "" : "disabled"
              }>${icon("send")}确认转账</button>
            </div>
          </div>

          <div class="stack">
            <div class="card card-pad">
              <div class="section-title"><h2>摘要</h2></div>
              <div class="summary stack-sm" id="summary">${summaryMarkup(local)}</div>
            </div>
            <div class="card card-pad">
              <div class="section-title"><h2>发送账户</h2></div>
              <div class="kv"><span class="kv-key">地址</span><span class="kv-value mono">${escapeHtml(
                shortAddress(account.address, 14, 8),
              )}</span></div>
              <div class="kv"><span class="kv-key">派生</span><span class="kv-value">${escapeHtml(
                account.derivation,
              )}</span></div>
              <div class="kv"><span class="kv-key">路径</span><span class="kv-value mono">${escapeHtml(
                account.path,
              )}</span></div>
            </div>
            <div class="note">${icon("info")}<div>转账前请再次核对地址的网络。区块链上的交易无法撤销。</div></div>
          </div>
        </div>`;
        bind();
        disposeScroll();
        disposeScroll = enhanceTabScroll(view.querySelector(".tabs"));
      };

      const feeMarkup = (state) => {
        if (state.quoting) {
          return `<div class="skeleton" style="height:56px"></div>
                  <div class="skeleton" style="height:56px"></div>
                  <div class="skeleton" style="height:56px"></div>`;
        }
        if (!state.quote) {
          return `<p class="hint">填写地址与金额后自动获取手续费。</p>`;
        }
        return state.quote.feeOptions
          .map(
            (option) => `<button class="fee-option" type="button" data-level="${option.level}" aria-pressed="${
              option.level === state.level
            }">
              <span class="fee-radio"></span>
              <span>
                <span class="fee-name">${escapeHtml(option.label)}</span>
                <span class="fee-note">${escapeHtml(option.note)} · ${escapeHtml(option.eta)}</span>
              </span>
              <span class="fee-value">${groupDigits(option.fee)}
                <span class="muted small">${escapeHtml(state.quote.symbol)}</span>
                ${option.feeFiat ? `<div class="fiat">${escapeHtml(option.feeFiat)}</div>` : ""}
              </span>
            </button>`,
          )
          .join("");
      };

      const summaryMarkup = (state) => {
        if (!state.quote) return `<p class="hint">等待报价…</p>`;
        const quote = state.quote;
        const fee = quote.feeOptions.find((option) => option.level === state.level) ?? quote.feeOptions[1];
        const amount = Number(state.amount || quote.amount || 0);
        const feeNumber = Number(fee?.fee ?? 0);
        return `
          <div class="kv"><span class="kv-key">金额</span><span class="kv-value">${groupDigits(
            state.amount || quote.amount,
          )} ${escapeHtml(quote.symbol)}</span></div>
          <div class="kv"><span class="kv-key">手续费（${escapeHtml(fee?.label ?? "")}）</span><span class="kv-value">${groupDigits(
            fee?.fee ?? "0",
          )} ${escapeHtml(quote.symbol)}</span></div>
          <div class="kv"><span class="kv-key">合计</span><span class="kv-value"><strong>${(
            amount + feeNumber
          ).toFixed(8).replace(/0+$/, "").replace(/\.$/, "")} ${escapeHtml(quote.symbol)}</strong></span></div>
          <div class="kv"><span class="kv-key">可用余额</span><span class="kv-value">${groupDigits(
            quote.balance,
          )} ${escapeHtml(quote.symbol)}</span></div>`;
      };

      const refreshFees = () => {
        const list = view.querySelector("#fee-list");
        if (list) list.innerHTML = feeMarkup(local);
        const summary = view.querySelector("#summary");
        if (summary) summary.innerHTML = summaryMarkup(local);
        bindFees();
        const review = view.querySelector("#review");
        if (review) review.disabled = !(local.quote?.canSend && !local.quoting);
      };

      const runQuote = debounce(async () => {
        const target = view.querySelector("#to");
        const amountInput = view.querySelector("#amount");
        if (target) local.to = target.value.trim();
        if (amountInput) local.amount = amountInput.value.trim();
        if (!local.to) {
          local.quote = null;
          local.quoteError = null;
          refreshFees();
          return;
        }
        local.quoting = true;
        local.quoteError = null;
        refreshFees();
        try {
          local.quote = await api.quoteSend({
            chain: local.chain,
            to: local.to,
            amount: local.amount,
          });
        } catch (error) {
          local.quote = null;
          local.quoteError = error.message;
        } finally {
          local.quoting = false;
          refreshFees();
        }
      }, 420);

      const bindFees = () => {
        view.querySelectorAll("[data-level]").forEach((button) => {
          button.addEventListener("click", () => {
            local.level = button.dataset.level;
            if (local.quote) {
              local.quote.feeOptions = local.quote.feeOptions.map((option) => ({
                ...option,
                selected: option.level === local.level,
              }));
            }
            refreshFees();
          });
        });
      };

      const bind = () => {
        const target = view.querySelector("#to");
        const amount = view.querySelector("#amount");
        target?.addEventListener("input", runQuote);
        target?.addEventListener("change", runQuote);
        amount?.addEventListener("input", (event) => {
          event.target.value = normalizeAmountInput(event.target.value);
          local.amount = event.target.value;
          runQuote();
        });
        view.querySelectorAll("[data-chain]").forEach((tab) => {
          tab.addEventListener("click", () => {
            local.chain = tab.dataset.chain;
            ctx.state.lastSendChain = local.chain;
            local.quote = null;
            local.level = "normal";
            paint();
            if (local.to) runQuote();
          });
        });
        view.querySelector('[data-action="max"]')?.addEventListener("click", () => {
          if (!amount) return;
          amount.value = local.quote?.maxAmount ?? "";
          local.amount = amount.value;
          runQuote();
        });
        view.querySelector('[data-action="go-receive"]')?.addEventListener("click", () =>
          ctx.go("receive", { chain: local.chain }),
        );
        view.querySelector('[data-action="pick-contact"]')?.addEventListener("click", () =>
          void pickContact(),
        );
        view.querySelector("#review")?.addEventListener("click", () => void review());
        bindFees();
      };

      const pickContact = async () => {
        const contacts = wallet.contacts ?? [];
        if (contacts.length === 0) {
          toast("地址簿还是空的，去「地址簿」添加联系人", "info");
          return;
        }
        const list = contacts
          .map(
            (contact) => `<button class="btn btn-ghost btn-block" data-contact="${escapeHtml(
              contact.id,
            )}" type="button" style="justify-content:flex-start">
              <span class="dot" style="--accent:${accentFor(contact.chain)}"></span>
              <span>${escapeHtml(contact.name)} · ${escapeHtml(shortAddress(contact.address, 10, 6))}</span>
            </button>`,
          )
          .join("");
        openModal(
          `<h2>选择联系人</h2><div class="stack-sm" style="margin-top:12px">${list}</div>
           <div class="modal-foot"><button class="btn btn-ghost" data-close type="button">取消</button></div>`,
          {
            onMount(element, close) {
              element.querySelectorAll("[data-contact]").forEach((button) => {
                button.addEventListener("click", () => {
                  const contact = contacts.find((item) => item.id === button.dataset.contact);
                  close();
                  if (!contact) return;
                  if (contact.chain !== local.chain) {
                    local.chain = contact.chain;
                    ctx.state.lastSendChain = contact.chain;
                  }
                  local.to = contact.address;
                  paint();
                  runQuote();
                });
              });
            },
          },
        );
      };

      const review = async () => {
        if (!local.quote?.canSend) return;
        const fee = local.quote.feeOptions.find((option) => option.level === local.level);
        const confirmed = await confirmDialog({
          title: "确认转账",
          confirmText: "签名并广播",
          body: `
            <div class="kv"><span class="kv-key">网络</span><span class="kv-value">${escapeHtml(
              local.quote.chain,
            )}</span></div>
            <div class="kv"><span class="kv-key">收款地址</span><span class="kv-value mono">${escapeHtml(
              local.to,
            )}</span></div>
            <div class="kv"><span class="kv-key">金额</span><span class="kv-value">${groupDigits(
              local.amount || local.quote.amount,
            )} ${escapeHtml(local.quote.symbol)}</span></div>
            <div class="kv"><span class="kv-key">手续费（${escapeHtml(
              fee?.label ?? "",
            )}）</span><span class="kv-value">${groupDigits(fee?.fee ?? "0")} ${escapeHtml(
              local.quote.symbol,
            )}</span></div>
            <div class="note note-warn" style="margin-top:10px">${icon("warn")}<div>交易一旦广播无法撤销，请确认地址无误。</div></div>`,
        });
        if (!confirmed) return;
        const button = view.querySelector("#review");
        setBusy(button, true, "正在签名…");
        local.sending = true;
        try {
          const result = await api.sendTransaction({
            chain: local.chain,
            to: local.to,
            amount: local.amount || local.quote.maxAmount,
            level: local.level,
          });
          local.result = result;
          ctx.state.portfolio = null;
          void ctx.refreshPortfolio({ force: true }).catch(() => {});
          paint();
        } catch (error) {
          toast(error.message, "error");
          setBusy(button, false);
          local.sending = false;
        }
      };

      const successMarkup = (state) => `
        <div class="page-head"><div><h1>已广播</h1>
          <div class="sub">交易已发送到 ${escapeHtml(state.result.chain)} 网络，等待打包确认。</div></div></div>
        <div class="card card-pad stack" style="max-width:640px">
          <div class="note note-ok">${icon("check")}<div>${escapeHtml(
            state.result.amount,
          )} ${escapeHtml(state.result.symbol)} 已发送到 ${escapeHtml(
            shortAddress(state.result.to, 14, 8),
          )}</div></div>
          <div class="kv"><span class="kv-key">交易哈希</span><span class="kv-value mono">${escapeHtml(
            state.result.txid,
          )}</span></div>
          <div class="kv"><span class="kv-key">手续费</span><span class="kv-value">${groupDigits(
            state.result.fee,
          )} ${escapeHtml(state.result.symbol)}</span></div>
          <div class="row" style="flex-wrap:wrap">
            <button class="btn btn-primary" data-action="copy-tx" type="button">${icon("copy")}复制交易哈希</button>
            <button class="btn" data-action="explorer" type="button">${icon("external")}在浏览器中查看</button>
            <button class="btn btn-ghost" data-action="again" type="button">${icon("send")}再转一笔</button>
            <button class="btn btn-ghost" data-action="activity" type="button">${icon("activity")}查看记录</button>
          </div>
        </div>`;

      const bindSuccess = (node) => {
        node.querySelector('[data-action="copy-tx"]').addEventListener("click", async () => {
          const ok = await copyText(local.result.txid);
          toast(ok ? "交易哈希已复制" : "复制失败", ok ? "success" : "error");
        });
        node.querySelector('[data-action="explorer"]').addEventListener("click", async () => {
          try {
            await api.openExternal(local.result.explorerUrl);
          } catch (error) {
            toast(error.message, "error");
          }
        });
        node.querySelector('[data-action="again"]').addEventListener("click", () => {
          local.result = null;
          local.to = "";
          local.amount = "";
          local.quote = null;
          local.quoteError = null;
          paint();
        });
        node.querySelector('[data-action="activity"]').addEventListener("click", () => ctx.go("activity"));
      };

      paint();
      if (local.to) runQuote();

      return () => disposeScroll();
    },
  };
}

function placeholderFor(chain) {
  switch (chain) {
    case "bitcoin":
      return "bc1q…";
    case "litecoin":
      return "ltc1q…";
    case "dogecoin":
      return "D…（Base58）";
    case "bitcoincash":
      return "bitcoincash:q…";
    case "solana":
      return "Base58 地址";
    default:
      return "0x…";
  }
}

function accentFor(chain) {
  const table = {
    bitcoin: "#F7931A",
    litecoin: "#345D9D",
    dogecoin: "#C2A633",
    bitcoincash: "#8DC351",
    ethereum: "#627EEA",
    "ethereum-classic": "#3AB83A",
    bsc: "#F3BA2F",
    polygon: "#8247E5",
    solana: "#14F195",
    monero: "#FF6600",
  };
  return table[chain] ?? "var(--ink-3)";
}
