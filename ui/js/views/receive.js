/** Receive: chain picker, QR code, copyable address, wallet-path details. */

import { api } from "../api.js?v=22";
import { copyText, escapeHtml, groupDigits, shortAddress } from "../format.js?v=22";
import { bindCopy, enhanceTabScroll, icon, toast } from "../ui.js?v=22";

/* 二维码配色：始终是「浅底深码」。
   深色主题下用反色（浅码深底）会让部分扫码器读不出来，
   所以深色主题里仍然给二维码一块浅色纸片，只是纸片颜色更贴近暖白。 */
const QR_PALETTE = {
  light: { dark: "#201e1b", light: "#fdfaf4" },
  dark: { dark: "#201e1b", light: "#f6f3ea" },
};

/** The QR follows the app theme: light modules on the paper colour in dark
 *  mode, ink on paper in light mode. */
function qrPalette() {
  const dark = document.documentElement.dataset.theme === "dark";
  return dark ? QR_PALETTE.dark : QR_PALETTE.light;
}

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
      <div class="stack">
        <div class="qr-card">
          <div class="qr-card-head">
            <span class="asset-mark" style="--accent:${account.accent}">${escapeHtml(
              account.symbol.slice(0, 2),
            )}</span>
            <div class="qr-card-title">
              <div class="name">${escapeHtml(account.name)}</div>
              <div class="sub">${escapeHtml(account.addressFormat)} · ${escapeHtml(
                account.hashAlgorithm,
              )}</div>
            </div>
            <span class="chip">${escapeHtml(account.symbol)}</span>
          </div>
          <div class="qr-card-body">
            <div class="qr-skeleton skeleton" id="qr-holder" style="display:${qr ? "none" : "grid"}"></div>
            <div id="qr-target" style="display:${qr ? "grid" : "none"}">${qr}</div>
          </div>
          <div class="qr-address" id="qr-address" data-copy-value role="button" tabindex="0"
               title="点击复制">${escapeHtml(account.address)}</div>
        </div>
      </div>

      <div class="stack">
        <div class="action-tiles">
          <button class="action-tile" data-action="copy-address" type="button">
            ${icon("copy")}<span>复制地址</span>
          </button>
          <button class="action-tile" data-action="open-explorer" type="button">
            ${icon("external")}<span>在浏览器中查看</span>
          </button>
        </div>
        <button class="btn btn-ghost btn-block" data-action="copy-amount" type="button">
          ${icon("qr")}复制地址 + 金额
        </button>

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
      </div>
    </div>

    <div class="note note-warn" style="margin-top:16px">${icon("warn")}
      <div>只接受 ${escapeHtml(account.name)} 网络上的资产。向其他网络的地址转入可能永久丢失。</div>
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
          const palette = qrPalette();
          const svg = await api.qrSvg(account.address, palette.dark, palette.light);
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

        // 地址本身可点（也支持键盘），点一下就复制
        const addressBox = node.querySelector("#qr-address");
        const copyAddress = async () => {
          const account = accountFor(chainId);
          const ok = await copyText(account.address);
          toast(ok ? "地址已复制" : "复制失败", ok ? "success" : "error");
        };
        addressBox?.addEventListener("click", () => void copyAddress());
        addressBox?.addEventListener("keydown", (event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            void copyAddress();
          }
        });
        node.querySelector('[data-action="copy-address"]')?.addEventListener("click", () => {
          void copyAddress();
        });

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

      // 主题切换后重画二维码（颜色烤在 SVG 里）
      const onTheme = () => void draw();
      document.addEventListener("lycoris:theme", onTheme);

      return () => {
        document.removeEventListener("lycoris:theme", onTheme);
        disposeScroll();
      };
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
