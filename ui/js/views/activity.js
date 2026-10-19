/** Activity: transactions recorded on this device, with filters and a detail
 *  dialog. Records are written by the Rust side when a transfer is broadcast;
 *  status is not re-fetched from the network yet. */

import { api } from "../api.js?v=8";
import { escapeHtml, formatDateTime, groupDigits, shortAddress } from "../format.js?v=8";
import { bindCopy, copyField, debounce, icon, openModal, toast } from "../ui.js?v=8";

const STATUS = {
  pending: { label: "待确认", cls: "badge badge-warn" },
  confirmed: { label: "已确认", cls: "badge badge-ok" },
  failed: { label: "失败", cls: "badge badge-danger" },
};

const EXPLORERS = {
  bitcoin: "https://mempool.space/tx/",
  litecoin: "https://litecoinspace.org/tx/",
  ethereum: "https://etherscan.io/tx/",
  bsc: "https://bscscan.com/tx/",
  polygon: "https://polygonscan.com/tx/",
  solana: "https://solscan.io/tx/",
};

export function render(ctx) {
  const wallet = ctx.state.wallet;
  const all = [...(wallet.activity ?? [])].sort((a, b) => b.timestamp - a.timestamp);
  const local = { chain: "all", dir: "all", query: "" };

  const matches = () => {
    const query = local.query.toLowerCase();
    return all.filter((item) => {
      if (local.chain !== "all" && item.chain !== local.chain) return false;
      if (local.dir !== "all" && item.direction !== local.dir) return false;
      if (!query) return true;
      return (
        item.txid.toLowerCase().includes(query) ||
        item.counterparty.toLowerCase().includes(query) ||
        item.symbol.toLowerCase().includes(query)
      );
    });
  };

  const listMarkup = () => {
    const items = matches();
    if (all.length === 0) {
      return `<div class="empty">
        <div class="mark">${icon("activity")}</div>
        还没有交易记录。<br />在本机发起的每一笔转账都会记录在这里。
      </div>`;
    }
    if (items.length === 0) {
      return `<div class="empty">没有符合筛选条件的记录。</div>`;
    }
    const groups = new Map();
    for (const item of items) {
      const key = dayKey(item.timestamp);
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(item);
    }
    return [...groups.entries()]
      .map(
        ([key, group]) =>
          `<div class="day-label">${escapeHtml(dayLabel(group[0].timestamp))}</div>` +
          group.map(itemMarkup).join(""),
      )
      .join("");
  };

  return {
    html: `<section class="view">
      <div class="page-head">
        <div>
          <h1>交易记录</h1>
          <div class="sub">目前只记录本机发起的交易；链上状态请以区块浏览器为准。</div>
        </div>
        <button class="btn" data-action="go-send" type="button">${icon("send")}去转账</button>
      </div>

      <div class="card card-pad">
        <div class="row-between" style="flex-wrap:wrap;gap:10px;margin-bottom:10px">
          <div class="tabs" id="activity-chains">
            <button class="tab" type="button" data-chain="all" aria-selected="true">全部</button>
            ${wallet.accounts
              .map(
                (account) =>
                  `<button class="tab" type="button" data-chain="${account.chain}" aria-selected="false"><span class="dot" style="--accent:${account.accent}"></span>${escapeHtml(
                    account.symbol,
                  )}</button>`,
              )
              .join("")}
          </div>
          <div class="row" style="gap:8px;flex-wrap:wrap">
            <div class="seg" id="activity-dir" role="group" aria-label="方向">
              <button type="button" data-dir="all" aria-pressed="true">全部</button>
              <button type="button" data-dir="send" aria-pressed="false">发送</button>
              <button type="button" data-dir="receive" aria-pressed="false">接收</button>
            </div>
            <input class="input" id="activity-search" style="width:210px" placeholder="搜索地址或交易哈希"
              autocomplete="off" spellcheck="false" />
          </div>
        </div>
        <div id="activity-list">${listMarkup()}</div>
      </div>
    </section>`,

    mount(root) {
      const list = root.querySelector("#activity-list");
      const chains = root.querySelector("#activity-chains");
      const dirs = root.querySelector("#activity-dir");
      const search = root.querySelector("#activity-search");

      const paint = () => {
        list.innerHTML = listMarkup();
      };
      const paintLater = debounce(paint, 160);

      chains.addEventListener("click", (event) => {
        const button = event.target.closest("[data-chain]");
        if (!button) return;
        local.chain = button.dataset.chain;
        chains
          .querySelectorAll("[data-chain]")
          .forEach((item) => item.setAttribute("aria-selected", String(item === button)));
        paint();
      });

      dirs.addEventListener("click", (event) => {
        const button = event.target.closest("[data-dir]");
        if (!button) return;
        local.dir = button.dataset.dir;
        dirs
          .querySelectorAll("[data-dir]")
          .forEach((item) => item.setAttribute("aria-pressed", String(item === button)));
        paint();
      });

      search.addEventListener("input", () => {
        local.query = search.value.trim();
        paintLater();
      });

      const openRow = (row) => {
        const item = all.find((entry) => entry.id === row.dataset.id);
        if (item) openDetail(item);
      };

      list.addEventListener("click", (event) => {
        const row = event.target.closest(".activity-item");
        if (row) openRow(row);
      });
      list.addEventListener("keydown", (event) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        const row = event.target.closest(".activity-item");
        if (!row) return;
        event.preventDefault();
        openRow(row);
      });

      root.querySelector('[data-action="go-send"]')?.addEventListener("click", () => ctx.go("send"));
    },
  };
}

/* ------------------------------------------------------------------ markup */

function itemMarkup(item) {
  const meta = STATUS[item.status] ?? STATUS.pending;
  const send = item.direction === "send";
  return `<div class="activity-item" data-id="${escapeHtml(item.id)}" role="button" tabindex="0">
    <span class="dir-mark ${send ? "dir-send" : "dir-receive"}">${icon(send ? "send" : "receive")}</span>
    <div class="activity-main">
      <div class="title">${escapeHtml(item.symbol)}${send ? "发送" : "接收"}
        <span class="${meta.cls}">${escapeHtml(meta.label)}</span></div>
      <div class="sub">${escapeHtml(shortAddress(item.counterparty, 16, 8))} · ${escapeHtml(
        shortAddress(item.txid, 12, 6),
      )}</div>
    </div>
    <div class="activity-amount">
      <div style="${send ? "" : "color:var(--ok)"}">${send ? "−" : "+"}${groupDigits(
        item.amount,
      )} <span class="muted small">${escapeHtml(item.symbol)}</span></div>
      <div class="fee">${escapeHtml(formatDateTime(item.timestamp))}</div>
    </div>
  </div>`;
}

function dayKey(timestamp) {
  const date = new Date(timestamp * 1000);
  return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
}

function dayLabel(timestamp) {
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const sameDay = (a, b) => a.toDateString() === b.toDateString();
  if (sameDay(date, now)) return "今天";
  const yesterday = new Date(now.getTime() - 86_400_000);
  if (sameDay(date, yesterday)) return "昨天";
  const month = date.getMonth() + 1;
  const day = date.getDate();
  return date.getFullYear() === now.getFullYear()
    ? `${month} 月 ${day} 日`
    : `${date.getFullYear()} 年 ${month} 月 ${day} 日`;
}

function explorerUrl(item) {
  if (item.explorerUrl?.startsWith("http")) return item.explorerUrl;
  return `${EXPLORERS[item.chain] ?? ""}${item.txid}`;
}

/* ------------------------------------------------------------------ detail */

function openDetail(item) {
  const meta = STATUS[item.status] ?? STATUS.pending;
  const send = item.direction === "send";
  openModal(
    `<h2>${escapeHtml(item.symbol)}${send ? "发送" : "接收"}详情</h2>
     <div class="stack-sm" style="margin-top:10px">
       <div class="kv"><span class="kv-key">状态</span><span class="kv-value"><span class="${meta.cls}">${escapeHtml(
         meta.label,
       )}</span></span></div>
       <div class="kv"><span class="kv-key">金额</span><span class="kv-value">${send ? "−" : "+"}${groupDigits(
         item.amount,
       )} ${escapeHtml(item.symbol)}</span></div>
       ${
         item.fee
           ? `<div class="kv"><span class="kv-key">手续费</span><span class="kv-value">${groupDigits(
               item.fee,
             )} ${escapeHtml(item.symbol)}</span></div>`
           : ""
       }
       <div class="kv"><span class="kv-key">时间</span><span class="kv-value">${escapeHtml(
         formatDateTime(item.timestamp),
       )}</span></div>
       <span class="field-label">${send ? "收款地址" : "来源地址"}</span>
       ${copyField(item.counterparty)}
       <span class="field-label">交易哈希</span>
       ${copyField(item.txid)}
       <div class="note">${icon("info")}<div>这条记录只保存在本机，状态不会自动从网络更新。最终状态请以区块浏览器为准。</div></div>
     </div>
     <div class="modal-foot">
       <button class="btn btn-ghost" data-close type="button">关闭</button>
       <button class="btn btn-primary" data-action="explorer" type="button">${icon("external")}区块浏览器</button>
     </div>`,
    {
      onMount(element) {
        bindCopy(element, (ok) => toast(ok ? "已复制" : "复制失败", ok ? "success" : "error"));
        element.querySelector('[data-action="explorer"]').addEventListener("click", async () => {
          try {
            await api.openExternal(explorerUrl(item));
          } catch (error) {
            toast(error.message, "error");
          }
        });
      },
    },
  );
}
