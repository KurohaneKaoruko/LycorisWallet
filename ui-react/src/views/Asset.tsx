/** 资产详情：总览的子页面。显示单个资产的余额、价格，以及该链在
 * 基础账户与派生账户下的地址（单链模型：派生只作用于本链）。
 * Ported from ui/js/views/asset.js. */

import { useEffect, useState } from "react";

import { api } from "../api";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";
import { formatFiat, groupDigits, shortAddress } from "../format";
import type { AccountDetailView, BatchStrategy, ChainAddressView, ChainId, DerivedAccountView, WalletView } from "../types";

const STRATEGY_LABELS: Record<string, { title: string; desc: string }> = {
  path: { title: "路径派生", desc: "BIP-32 地址索引递增（助记词）" },
  salted: { title: "加盐派生", desc: "H(域名‖密语‖#序号)" },
  iterated: { title: "迭代哈希", desc: "基于基础私钥连续哈希 N 次" },
};

/** 派生策略按钱包模式过滤：助记词走 BIP-32 路径（+迭代哈希）；
 * 加盐派生以口令为哈希输入，属于口令钱包专属，助记词不提供。 */
function strategiesFor(wallet: WalletView | null): Array<[string, { title: string; desc: string }]> {
  const all = Object.entries(STRATEGY_LABELS);
  if (!wallet || wallet.mode === "mnemonic") {
    return all.filter(([key]) => key !== "salted");
  }
  return all.filter(([key]) => key !== "path");
}

function maxCount(strategy: string): number {
  return strategy === "iterated" ? 50 : 100;
}

/* ------------------------------------------------------------- detail dialog */

function KeyChain({ detail }: { detail: AccountDetailView }) {
  return (
    <div className="key-chain">
      <span className="key-chain-box mono">{detail.sourcePreview}</span>
      <span className="key-chain-arrow">↓</span>
      <span className="key-chain-box key-chain-formula mono">{detail.formula}</span>
      <span className="key-chain-arrow">↓</span>
      <span className="key-chain-box key-chain-final mono">{detail.privateKey}</span>
    </div>
  );
}

function DetailDialog({
  detail,
  onClose,
}: {
  detail: AccountDetailView;
  onClose: () => void;
}) {
  const toast = useToast();

  return (
    <Modal title="地址详情" onClose={onClose}>
      <p className="hint" style={{ margin: "4px 0 12px" }}>
        {detail.chainName}（{detail.symbol}）· {detail.label}
      </p>
      <div className="detail-card">
        <div className="detail-row">
          <span className="detail-key">派生账户</span>
          <span className="detail-value">{detail.label}</span>
        </div>
        <div className="detail-row">
          <span className="detail-key">派生路径</span>
          <span className="detail-value mono">{detail.path}</span>
        </div>
        <div className="detail-row">
          <span className="detail-key">派生方式</span>
          <span className="detail-value">{detail.derivation}</span>
        </div>
        <div className="detail-row">
          <span className="detail-key">哈希算法</span>
          <span className="detail-value">{detail.hashAlgorithm}</span>
        </div>
        <div className="detail-row">
          <span className="detail-key">地址格式</span>
          <span className="detail-value">{detail.addressFormat}</span>
        </div>
        <div className="detail-row">
          <span className="detail-key">地址</span>
          <span className="detail-value mono">{detail.address}</span>
        </div>
        <div className="detail-row">
          <span className="detail-key">公钥</span>
          <span className="detail-value mono">{detail.publicKey}</span>
        </div>
      </div>
      <details className="detail-source" style={{ marginTop: 12 }}>
        <summary>
          <Icon name="info" />
          私钥信息<span className="hint">（点击展开）</span>
        </summary>
        <KeyChain detail={detail} />
      </details>
      <div className="note note-warn" style={{ marginTop: 12 }}>
        <Icon name="warn" />
        <div>{detail.warning}</div>
      </div>
      <div className="modal-foot">
        <button
          className="btn btn-ghost"
          type="button"
          onClick={async () => {
            const ok = await copyText(detail.privateKey);
            toast(ok ? "已复制" : "复制失败", ok ? "success" : "error");
          }}
        >
          <Icon name="copy" />
          复制私钥
        </button>
        <button className="btn" data-close type="button" onClick={onClose}>
          关闭
        </button>
      </div>
    </Modal>
  );
}

/* -------------------------------------------------------- add derived dialog */

function AddDerivedDialog({
  chain,
  chainName,
  wallet,
  onClose,
  onAdded,
}: {
  chain: ChainId;
  chainName: string;
  wallet: WalletView | null;
  onClose: () => void;
  onAdded: (view: WalletView) => void;
}) {
  const strategies = strategiesFor(wallet);
  const [strategy, setStrategy] = useState<BatchStrategy>(strategies[0][0] as BatchStrategy);
  const [count, setCount] = useState(1);
  const [start, setStart] = useState(1);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const toast = useToast();

  // 迭代哈希基于基础私钥,0 号会与基础账户重合,从 1 起。
  const minStart = strategy === "iterated" ? 1 : 0;
  const effectiveStart = Math.max(start, minStart);
  const max = maxCount(strategy);

  const submit = async () => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const view = await api.addWalletsBatch({
        batch: { strategy, chain, count, start: effectiveStart },
        name: null,
      });
      onAdded(view);
      onClose();
      toast(`已添加 ${count} 个派生地址`, "success");
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <Modal title="添加派生地址" onClose={onClose}>
      <p className="hint" style={{ margin: "6px 0 12px" }}>
        为当前钱包追加 <strong>{chainName}</strong>{" "}
        派生地址；只记录「派生方式 + 序号」，不保存额外密钥。其他币种不受影响，仍使用基础地址。
      </p>
      <div className="field" style={{ marginBottom: 16 }}>
        <span className="field-label">派生方式</span>
        <div className="seg seg-col" role="group">
          {strategies.map(([key, meta]) => (
            <button
              key={key}
              type="button"
              data-strategy={key}
              aria-pressed={strategy === key}
              onClick={() => {
                setStrategy(key as BatchStrategy);
                if (key === "iterated") setStart((value) => Math.max(value, 1));
              }}
            >
              <span className="seg-title">{meta.title}</span>
              <span className="seg-desc">{meta.desc}</span>
            </button>
          ))}
        </div>
      </div>
      <div className="row" style={{ gap: 12 }}>
        <div className="field" style={{ flex: 1 }}>
          <label htmlFor="derived-count">数量（1–{max}）</label>
          <input
            className="input"
            id="derived-count"
            type="number"
            min={1}
            max={max}
            value={count}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value)) setCount(Math.max(1, Math.min(max, Math.floor(value))));
            }}
          />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label htmlFor="derived-start">起始序号{minStart > 0 ? "（≥1）" : ""}</label>
          <input
            className="input"
            id="derived-start"
            type="number"
            min={minStart}
            max={max - 1}
            value={effectiveStart}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value)) {
                setStart(Math.max(minStart, Math.min(max - 1, Math.floor(value))));
              }
            }}
          />
        </div>
      </div>
      {error ? <p className="error-text">{error}</p> : null}
      <div className="modal-foot">
        <button className="btn btn-ghost" type="button" onClick={onClose}>
          取消
        </button>
        <button className="btn btn-primary" type="button" disabled={busy} onClick={() => void submit()}>
          {busy ? "派生中…" : "添加"}
        </button>
      </div>
    </Modal>
  );
}

/* ------------------------------------------------------- manage derived acct */

function ManageDialog({
  account,
  onClose,
  onChanged,
}: {
  account: DerivedAccountView;
  onClose: () => void;
  onChanged: (view: WalletView) => void;
}) {
  const [label, setLabel] = useState(account.label);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const toast = useToast();
  const { confirm } = useDialogs();

  const rename = async () => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const view = await api.renameBatchAccount(account.id, label.trim() || account.label);
      onChanged(view);
      onClose();
      toast("已重命名", "success");
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  const remove = async () => {
    const ok = await confirm({
      title: `删除派生账户「${account.label}」`,
      body: "<p>删除后该地址不再参与余额查询与总览合计。</p><p class=\"hint\">这只移除「派生配方」，不影响链上资产；需要时可以用相同序号重新添加。</p>",
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      const view = await api.removeBatchAccount(account.id);
      onChanged(view);
      onClose();
      toast("已删除", "success");
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <Modal title={`派生账户「${account.label}」`} onClose={onClose}>
      <p className="hint" style={{ margin: "6px 0 12px" }}>
        {STRATEGY_LABELS[account.strategy]?.title ?? account.strategy} · 序号 {account.index} ·
        仅作用于该币种
        {account.fingerprint ? ` · 指纹 ${account.fingerprint}` : ""}
      </p>
      <div className="field" style={{ marginBottom: 4 }}>
        <label htmlFor="manage-label">名称</label>
        <input
          className="input"
          id="manage-label"
          type="text"
          maxLength={40}
          value={label}
          onChange={(event) => setLabel(event.target.value)}
        />
      </div>
      {error ? <p className="error-text">{error}</p> : null}
      <div className="modal-foot" style={{ justifyContent: "space-between" }}>
        <button
          className="btn btn-ghost"
          type="button"
          style={{ color: "var(--danger)" }}
          onClick={() => void remove()}
        >
          <Icon name="trash" />
          删除
        </button>
        <div className="row" style={{ gap: 8 }}>
          <button className="btn btn-ghost" type="button" onClick={onClose}>
            取消
          </button>
          <button className="btn btn-primary" type="button" disabled={busy} onClick={() => void rename()}>
            {busy ? "保存中…" : "保存"}
          </button>
        </div>
      </div>
    </Modal>
  );
}

/* ----------------------------------------------------------------- the view */

export function AssetView() {
  const { wallet, portfolio, params, go, setWallet } = useSession();
  const toast = useToast();
  const chainId = (params.chain as ChainId | undefined) ?? "bitcoin";
  const [rows, setRows] = useState<ChainAddressView[] | null>(null);
  const [detail, setDetail] = useState<AccountDetailView | null>(null);
  const [adding, setAdding] = useState(false);
  const [managing, setManaging] = useState<DerivedAccountView | null>(null);

  const settings = wallet?.settings;
  const currency = settings?.fiat ?? "usd";
  const hide = Boolean(settings?.hideBalances);
  const asset = portfolio?.assets?.find((item) => item.chain === chainId);
  const chainAccount = wallet?.accounts?.find((item) => item.chain === chainId);
  const name = asset?.name ?? chainAccount?.name ?? chainId;
  const symbol = asset?.symbol ?? chainAccount?.symbol ?? "";
  const accent = asset?.accent ?? chainAccount?.accent ?? "#b3553f";
  const value = asset ? (currency === "cny" ? asset.valueCny : asset.valueUsd) : null;
  const error = asset?.error ?? null;
  const amount = asset?.amount ?? null;

  useEffect(() => {
    let cancelled = false;
    setRows(null);
    api
      .chainAddresses(chainId)
      .then((list) => {
        if (!cancelled) setRows(list);
      })
      .catch((e) => {
        if (!cancelled) {
          toast((e as Error).message, "error");
          setRows([]);
        }
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [chainId, wallet?.activeBatch, wallet?.batchAccounts?.length]);

  const switchTo = async (row: ChainAddressView) => {
    if (row.active) return;
    try {
      const view = await api.selectBatchAccount(row.id);
      setWallet(view);
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  // 与旧版 withAccount 一致:先把该行账户切为当前,再跳转收款/转账。
  const withAccount = async (row: ChainAddressView, target: "receive" | "send") => {
    try {
      if ((wallet?.activeBatch ?? null) !== (row.id ?? null)) {
        setWallet(await api.selectBatchAccount(row.id));
      }
      go(target, { chain: chainId });
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  const dialogs = useDialogs();

  const openDetail = async (row: ChainAddressView) => {
    const password = await dialogs.askPassword({
      title: "查看地址详情",
      description: "详情包含该地址的私钥。验证本机密码后显示，请确认周围没有人。",
      confirmText: "查看",
    });
    if (!password) return;
    try {
      const result = await api.accountDetail(chainId, row.id, password);
      setDetail(result);
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <button className="btn btn-ghost btn-sm" type="button" onClick={() => go("dashboard")}>
            <Icon name="arrowLeft" />
            返回总览
          </button>
        </div>
      </div>

      <div className="card card-pad asset-hero" style={{ marginTop: 10 }}>
        <div className="asset-hero-main">
          <span className="asset-mark asset-mark-lg" style={{ ["--accent" as string]: accent }}>
            {symbol.slice(0, 2)}
          </span>
          <div>
            <div className="asset-name">
              {name} <span className="muted small">{symbol}</span>
            </div>
            <div className="total asset-total">
              <span className={hide ? "hide-balance" : ""}>
                {amount != null ? groupDigits(amount) : "—"} <span className="cur">{symbol}</span>
              </span>
            </div>
            {error ? (
              <div className="asset-error">{error}</div>
            ) : (
              <div className="fiat">{formatFiat(value, currency)}</div>
            )}
          </div>
        </div>
        {asset?.priceUsd ? (
          <div className="asset-price muted small">
            单价 {formatFiat(currency === "cny" ? asset.priceCny : asset.priceUsd, currency)}
          </div>
        ) : null}
      </div>

      <div className="card card-pad" style={{ marginTop: 16 }}>
        <div className="section-title">
          <h2>派生地址</h2>
          <span className="hint">点击行切换账户 · 收款、转账与详情跟随所选账户</span>
        </div>
        <div id="addr-list">
          {rows === null ? (
            <>
              <div className="skeleton" style={{ height: 64 }} />
              <div className="skeleton" style={{ height: 64, marginTop: 8 }} />
            </>
          ) : rows.length === 0 ? (
            <p className="hint">暂无地址</p>
          ) : (
            rows.map((row) => (
              <div
                key={row.id ?? "base"}
                className={`addr-row${row.active ? " is-active" : ""}`}
                role="button"
                tabIndex={0}
                title="点击切换到该账户"
                onClick={() => void switchTo(row)}
                onKeyDown={(event) => {
                  if (event.key === "Enter" || event.key === " ") {
                    event.preventDefault();
                    void switchTo(row);
                  }
                }}
              >
                <div className="addr-main">
                  <div className="addr-label">
                    {row.label}
                    {row.active ? <span className="chip chip-ok">当前</span> : null}
                  </div>
                  <div className="addr-mono">{shortAddress(row.address, 18, 12)}</div>
                  <div className="addr-path muted small">{row.path}</div>
                </div>
                <div className="addr-actions">
                  <button
                    className="btn btn-ghost btn-sm"
                    type="button"
                    title="复制地址"
                    onClick={async (event) => {
                      event.stopPropagation();
                      const ok = await copyText(row.address);
                      toast(ok ? "已复制" : "复制失败", ok ? "success" : "error");
                    }}
                  >
                    <Icon name="copy" />
                  </button>
                  <button
                    className="btn btn-ghost btn-sm"
                    type="button"
                    title="查看地址详情"
                    onClick={(event) => {
                      event.stopPropagation();
                      void openDetail(row);
                    }}
                  >
                    <Icon name="info" />
                  </button>
                  {row.id ? (
                    <button
                      className="btn btn-ghost btn-sm"
                      type="button"
                      title="重命名 / 删除该派生账户"
                      onClick={(event) => {
                        event.stopPropagation();
                        const account = wallet?.batchAccounts?.find((item) => item.id === row.id);
                        if (account) setManaging(account);
                      }}
                    >
                      <Icon name="more" />
                    </button>
                  ) : null}
                  <button
                    className="btn btn-sm"
                    type="button"
                    onClick={(event) => {
                      event.stopPropagation();
                      void withAccount(row, "receive");
                    }}
                  >
                    收款
                  </button>
                  <button
                    className="btn btn-sm"
                    type="button"
                    onClick={(event) => {
                      event.stopPropagation();
                      void withAccount(row, "send");
                    }}
                  >
                    转账
                  </button>
                </div>
              </div>
            ))
          )}
        </div>
        <div className="row" style={{ gap: 8, marginTop: 14 }}>
          <button className="btn" type="button" id="addr-add-derived" onClick={() => setAdding(true)}>
            <Icon name="layers" />
            添加派生地址
          </button>
          <span className="hint">按需派生：只有添加了的地址才会参与余额查询与总览合计</span>
        </div>
      </div>

      {adding ? (
        <AddDerivedDialog
          chain={chainId}
          chainName={name}
          wallet={wallet}
          onClose={() => setAdding(false)}
          onAdded={setWallet}
        />
      ) : null}
      {managing ? (
        <ManageDialog account={managing} onClose={() => setManaging(null)} onChanged={setWallet} />
      ) : null}
      {detail ? <DetailDialog detail={detail} onClose={() => setDetail(null)} /> : null}
    </section>
  );
}
