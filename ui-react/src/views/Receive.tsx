/** Receive: chain picker, QR code, copyable address, wallet-path details.
 *  Ported from ui/js/views/receive.js. */

import { useEffect, useState } from "react";

import { api } from "../api";
import { useSession } from "../shell/session";
import { useTabScroll, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";
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
  const { wallet, params, go } = useSession();
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
                  await api.openExternal(account.address ? `explorer:${account.chain}` : "");
                } catch {
                  // Explorer links are best-effort in the demo.
                }
              }}
            >
              <Icon name="external" />
              <span>在浏览器中查看</span>
            </button>
          </div>

          <div className="card card-pad">
            <div className="kv">
              <span className="kv-key">钱包</span>
              <span className="kv-value">{wallet.label}</span>
            </div>
            <div className="kv">
              <span className="kv-key">派生路径</span>
              <span className="kv-value mono">{account.path}</span>
            </div>
            <div className="kv">
              <span className="kv-key">地址格式</span>
              <span className="kv-value">{account.addressFormat}</span>
            </div>
            <div className="kv">
              <span className="kv-key">哈希算法</span>
              <span className="kv-value">{account.hashAlgorithm}</span>
            </div>
            <p className="hint" style={{ marginTop: 12 }}>
              地址由当前选中账户派生；如需使用派生地址收款，请先到资产页切换账户。
            </p>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => go("asset", { chain: chainId })}>
              前往 {account.name} 资产页
              <Icon name="chevronRight" />
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}
