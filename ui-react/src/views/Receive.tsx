/** Receive: chain picker, QR code, copyable address, wallet-path details.
 *  Ported from ui/js/views/receive.js. */

import { useEffect, useState } from "react";

import { api } from "../api";
import { useSession } from "../shell/session";
import { useTabScroll, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";
import { groupDigits, shortAddress } from "../format";
import type { Account, ChainId } from "../types";


/* 二维码配色：始终是「浅底深码」。深色主题下也保持浅色纸片。 */
const QR_PALETTE = {
  light: { dark: "#201e1b", light: "#fdfaf4" },
  dark: { dark: "#201e1b", light: "#f6f3ea" },
};

function qrPalette() {
  const dark = document.documentElement.dataset.theme === "dark";
  return dark ? QR_PALETTE.dark : QR_PALETTE.light;
}

export function ReceiveView() {
  const { wallet, portfolio, params, go } = useSession();
  const toast = useToast();
  const tabsRef = useTabScroll<HTMLDivElement>();
  const [chainId, setChainId] = useState<ChainId>(
    ((params.chain as ChainId) ?? "bitcoin"),
  );
  const [qr, setQr] = useState<string>("");

  const account: Account | undefined = wallet?.accounts.find((item) => item.chain === chainId);

  useEffect(() => {
    if (!account) return;
    let cancelled = false;
    setQr("");
    const palette = qrPalette();
    api
      .qrSvg(account.address, palette.dark, palette.light)
      .then((svg) => {
        if (!cancelled) setQr(svg);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [account?.address]); // eslint-disable-line react-hooks/exhaustive-deps

  /* 主题切换后重画二维码（颜色烤在 SVG 里），与旧版 lycoris:theme 一致。 */
  useEffect(() => {
    if (!account) return;
    const onTheme = () => {
      const palette = qrPalette();
      api
        .qrSvg(account.address, palette.dark, palette.light)
        .then((svg) => setQr(svg))
        .catch(() => {});
    };
    document.addEventListener("lycoris:theme", onTheme);
    return () => document.removeEventListener("lycoris:theme", onTheme);
  }, [account?.address]); // eslint-disable-line react-hooks/exhaustive-deps

  if (!wallet || !account) {
    return (
      <section className="view">
        <p className="hint">该网络未启用。</p>
      </section>
    );
  }

  const copyAddress = async () => {    const ok = await copyText(account.address);
    toast(ok ? "地址已复制" : "复制失败", ok ? "success" : "error");
  };

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>收款</h1>
          <div className="sub">选择网络后展示地址与二维码。请务必确认对方使用的网络与这里一致。</div>
        </div>
        <button className="btn" type="button" onClick={() => go("send", { chain: chainId })}>
          <Icon name="send" />
          去转账
        </button>
      </div>

      <div className="tabs" role="tablist" style={{ marginBottom: 16 }} ref={tabsRef}>
        {wallet.accounts.map((item) => (
          <button
            key={item.chain}
            className="tab"
            role="tab"
            data-chain={item.chain}
            aria-selected={item.chain === account.chain}
            onClick={() => setChainId(item.chain)}
          >
            <span className="dot" style={{ ["--accent" as string]: item.accent }} />
            {item.symbol}
          </button>
        ))}
      </div>

      <div className="receive-layout">
        <div className="stack">
          <div className="qr-card">
            <div className="qr-card-head">
              <span className="asset-mark" style={{ ["--accent" as string]: account.accent }}>
                {account.symbol.slice(0, 2)}
              </span>
              <div className="qr-card-title">
                <div className="name">{account.name}</div>
                <div className="sub">
                  {account.addressFormat} · {account.hashAlgorithm}
                </div>
              </div>
              <span className="chip">{account.symbol}</span>
            </div>
            <div className="qr-card-body">
              {!qr ? (
                <div className="qr-skeleton skeleton" />
              ) : (
                <div id="qr-target" dangerouslySetInnerHTML={{ __html: qr }} />
              )}
            </div>
            <div
              className="qr-address"
              role="button"
              tabIndex={0}
              title="点击复制"
              onClick={() => void copyAddress()}
              onKeyDown={(event) => {
                if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  void copyAddress();
                }
              }}
            >
              {account.address}
            </div>
          </div>
        </div>

        <div className="stack">
          <div className="action-tiles">
            <button className="action-tile" type="button" onClick={() => void copyAddress()}>
              <Icon name="copy" />
              <span>复制地址</span>
            </button>
            <button
              className="action-tile"
              type="button"
              onClick={async () => {
                try {
                  await api.openExternal(explorerUrlFor(chainId, account.address, account.explorerUrl));
                } catch (e) {
                  toast((e as Error).message, "error");
                }
              }}
            >
              <Icon name="external" />
              <span>在浏览器中查看</span>
            </button>
          </div>
          <button
            className="btn btn-ghost btn-block"
            type="button"
            onClick={async () => {
              const ok = await copyText(`${chainId}:${account.address}`);
              toast(ok ? "已复制「网络:地址」" : "复制失败", ok ? "success" : "error");
            }}
          >
            <Icon name="qr" />
            复制地址 + 金额
          </button>

          <div className="card card-pad">
            <div className="section-title"><h2>地址信息</h2></div>
            <div className="kv">
              <span className="kv-key">派生方式</span>
              <span className="kv-value">{account.derivation}</span>
            </div>
            <div className="kv">
              <span className="kv-key">派生路径</span>
              <span className="kv-value mono">{account.path}</span>
            </div>
            <div className="kv">
              <span className="kv-key">哈希算法</span>
              <span className="kv-value">
                {account.hashAlgorithm}
                <br />
                <span className="hint">{account.hashDetail}</span>
              </span>
            </div>
            <div className="kv">
              <span className="kv-key">公钥</span>
              <span className="kv-value mono">{shortAddress(account.publicKey, 18, 10)}</span>
            </div>
            <div className="kv">
              <span className="kv-key">当前余额</span>
              <span className="kv-value">
                {(() => {
                  const asset = portfolio?.assets.find((item) => item.chain === account.chain);
                  return asset ? `${groupDigits(asset.amount)} ${asset.symbol}` : "—";
                })()}
              </span>
            </div>
          </div>
        </div>
      </div>

      <div className="note note-warn" style={{ marginTop: 16 }}>
        <Icon name="warn" />
        <div>只接受 {account.name} 网络上的资产。向其他网络的地址转入可能永久丢失。</div>
      </div>
    </section>
  );
}

/** 浏览器地址页（与旧版 receive.js explorerUrlFor 一致）。 */
function explorerUrlFor(chain: ChainId, address: string, accountExplorer?: string): string {
  const table: Record<string, string> = {
    bitcoin: "https://mempool.space/address/",
    litecoin: "https://litecoinspace.org/address/",
    dogecoin: "https://doge.trezor.io/address/",
    bitcoincash: "https://bch.trezor.io/address/",
    ethereum: "https://etherscan.io/address/",
    "ethereum-classic": "https://blockscout.com/etc/mainnet/address/",
    ethw: "https://www.oklink.com/ethw/address/",
    bsc: "https://bscscan.com/address/",
    polygon: "https://polygonscan.com/address/",
    solana: "https://solscan.io/account/",
    monero: "https://xmrchain.net/search?value=",
    bitcoinsv: "https://whatsonchain.com/address/",
    namecoin: "https://nmc.blockbook.network/address/",
    litecoincash: "https://explorer.litecoinca.sh/address/",
    feathercoin: "https://ftc.blockbook.network/address/",
    viacoin: "https://via.blockbook.network/address/",
    potcoin: "https://chainz.cryptoid.info/pot/address.dws?",
    egulden: "https://efl.blockbook.network/address/",
    dash: "https://chainz.cryptoid.info/dash/address.dws?",
    groestlcoin: "https://grs.blockbook.network/address/",
    vertcoin: "https://vtc.blockbook.network/address/",
    ravencoin: "https://blockbook.ravencoin.org/address/",
    monacoin: "https://blockbook.electrum-mona.org/address/",
    fujicoin: "https://explorer.fujicoin.org/address/",
    zcash: "https://blockbook.zec.zelcore.io/address/",
    firo: "https://blockbook.firo.org/address/",
    neurai: "https://blockbook.neurai.org/address/",
    clore: "https://blockbook.clore.zelcore.io/address/",
    zephyr: "https://explorer.zephyrprotocol.com/",
    octaspace: "https://explorer.octa.space/address/",
  };
  const base = accountExplorer?.startsWith("http") ? accountExplorer : table[chain] ?? table.bitcoin;
  // xmrchain search is a query URL; every other explorer is a path prefix.
  if (chain === "monero") return `${base}${address}`;
  return `${base.endsWith("/") ? base : `${base}/`}${address}`;
}
