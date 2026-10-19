/** Receive: chain picker, QR code, copyable address, wallet-path details. */

import { api } from "../api.js?v=10";
import { copyText, escapeHtml, groupDigits, shortAddress } from "../format.js?v=10";
import { bindCopy, enhanceTabScroll, icon, toast } from "../ui.js?v=10";

export function render(ctx, params = {}) {
  const wallet = ctx.state.wallet;
  let chainId = params.chain ?? ctx.state.lastReceiveChain ?? "bitcoin";
  let qr = "";
  let qrToken = 0;

  const accountFor = (id) => wallet.accounts.find((account) => account.chain === id);

  const shell = (account) => `
    <div class="page-head">
      <div>
        <h1>收款</h1>
        <div class="sub">选择网络后展示地址与二维码。请务必确认对方使用的网络与这里一致。</div>
      </div>
      <button class="btn" data-action="go-send" type="button">${icon("send")}去转账</button>
    </div>

    <div class="tabs" role="tablist" style="margin-bottom:16px">
      ${wallet.accounts
        .map(
          (item) => `<button class="tab" role="tab" data-chain="${item.chain}" aria-selected="${
            item.chain === account.chain
          }"><span class="dot" style="--accent:${item.accent}"></span>${escapeHtml(item.symbol)}</button>`,
        )
        .join("")}
    </div>

    <div class="receive-layout">
      <div class="qr-frame">
        <div class="qr-skeleton skeleton" id="qr-holder" style="display:${qr ? "none" : "grid"}"></div>
        <div id="qr-target" style="display:${qr ? "grid" : "none"}">${qr}</div>
      </div>

      <div class="stack">
        <div class="card card-pad stack-sm">
          <div class="row-between">
            <div class="row" style="gap:9px">
              <span class="asset-mark" style="--accent:${account.accent}">${escapeHtml(
                account.symbol.slice(0, 2),
              )}</span>
              <div>
                <div class="asset-name">${escapeHtml(account.name)}</div>
                <div class="asset-sub">${escapeHtml(account.addressFormat)} · ${escapeHtml(
                  account.hashAlgorithm,
                )}</div>
              </div>
            </div>
            <span class="chip">${escapeHtml(account.symbol)}</span>
          </div>
          <div class="copy-field" id="address-field">
            <span class="value" data-copy-value>${escapeHtml(account.address)}</span>
            <button class="btn btn-ghost btn-sm" data-copy type="button">${icon("copy")}复制</button>
          </div>
          <div class="row" style="gap:8px">
            <button class="btn btn-ghost btn-sm" data-action="open-explorer" type="button">
              ${icon("external")}在浏览器中查看
            </button>
            <button class="btn btn-ghost btn-sm" data-action="copy-amount" type="button">复制地址 + 金额</button>
          </div>
        </div>

        <div class="card card-pad">
          <div class="section-title"><h2>地址信息</h2></div>
          <div class="kv"><span class="kv-key">派生方式</span><span class="kv-value">${escapeHtml(
            account.derivation,
          )}</span></div>
          <div class="kv"><span class="kv-key">派生路径</span><span class="kv-value mono">${escapeHtml(
            account.path,
          )}</span></div>
          <div class="kv"><span class="kv-key">哈希算法</span><span class="kv-value">${escapeHtml(
            account.hashAlgorithm,
          )}<br /><span class="hint">${escapeHtml(account.hashDetail)}</span></span></div>
          <div class="kv"><span class="kv-key">公钥</span><span class="kv-value mono">${escapeHtml(
            shortAddress(account.publicKey, 18, 10),
          )}</span></div>
          <div class="kv"><span class="kv-key">当前余额</span><span class="kv-value">${
            (() => {
              const asset = ctx.state.portfolio?.assets.find((item) => item.chain === account.chain);
              return asset ? `${groupDigits(asset.amount)} ${escapeHtml(asset.symbol)}` : "—";
            })()
          }</span></div>
        </div>

        <div class="note">${icon("info")}
          <div>只接受 ${escapeHtml(account.name)} 网络上的资产。向其他网络的地址转入可能永久丢失。</div>
        </div>
      </div>
    </div>`;

  return {
    html: `<section class="view" id="receive-view"></section>`,

    mount(root) {
      const view = root.querySelector("#receive-view");
      let disposeScroll = () => {};
      const draw = async () => {
        const account = accountFor(chainId);
        view.innerHTML = shell(account);
        bind(view);
        disposeScroll();
        disposeScroll = enhanceTabScroll(view.querySelector(".tabs"));
        const token = ++qrToken;
        try {
          const svg = await api.qrSvg(account.address, "#1b1a19", "#f7f1e8");
          if (token !== qrToken) return;
          qr = svg;
          const target = view.querySelector("#qr-target");
          const holder = view.querySelector("#qr-holder");
          target.innerHTML = svg;
          target.style.display = "grid";
          holder.style.display = "none";
        } catch (error) {
          toast(error.message, "error");
        }
      };

      const bind = (node) => {
        node.querySelectorAll("[data-chain]").forEach((tab) => {
          tab.addEventListener("click", () => {
            chainId = tab.dataset.chain;
            ctx.state.lastReceiveChain = chainId;
            void draw();
          });
        });
        bindCopy(node, (ok) => toast(ok ? "地址已复制" : "复制失败", ok ? "success" : "error"));
        node.querySelector('[data-action="open-explorer"]').addEventListener("click", async () => {
          const account = accountFor(chainId);
          const url = explorerUrlFor(chainId, account.address, account.explorerUrl);
          try {
            await api.openExternal(url);
          } catch (error) {
            toast(error.message, "error");
          }
        });
        node.querySelector('[data-action="copy-amount"]').addEventListener("click", async () => {
          const account = accountFor(chainId);
          const text = `${chainId}:${account.address}`;
          const ok = await copyText(text);
          toast(ok ? "已复制「网络:地址」" : "复制失败", ok ? "success" : "error");
        });
        node.querySelector('[data-action="go-send"]').addEventListener("click", () =>
          ctx.go("send", { chain: chainId }),
        );
      };

      void draw();

      return () => disposeScroll();
    },
  };
}

function explorerUrlFor(chain, address, accountExplorer) {
  const table = {
    bitcoin: "https://mempool.space/address/",
    litecoin: "https://litecoinspace.org/address/",
    dogecoin: "https://blockchair.com/dogecoin/address/",
    bitcoincash: "https://blockchair.com/bitcoin-cash/address/",
    ethereum: "https://etherscan.io/address/",
    "ethereum-classic": "https://blockscout.com/etc/mainnet/address/",
    bsc: "https://bscscan.com/address/",
    polygon: "https://polygonscan.com/address/",
    solana: "https://solscan.io/account/",
    monero: "https://xmrchain.net/search?value=",
  };
  const base = accountExplorer?.startsWith("http") ? accountExplorer : table[chain] ?? table.bitcoin;
  // xmrchain search is a query URL; every other explorer is a path prefix.
  if (chain === "monero") {
    return `${base}${address}`;
  }
  return `${base.endsWith("/") ? base : `${base}/`}${address}`;
}
