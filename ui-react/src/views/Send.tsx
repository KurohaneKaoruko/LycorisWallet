/** Send: chain picker, recipient, amount, fee selection, confirm + result.
 *  Ported from ui/js/views/send.js (quote → confirm → broadcast). */

import { useEffect, useState } from "react";

import { api } from "../api";
import { copyText } from "../clipboard";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useTabScroll, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { groupDigits, shortAddress } from "../format";
import type { ChainId, FeeOption, SendQuote, SendResult } from "../types";

export function SendView() {
  const { wallet, params, go, reloadWallet } = useSession();
  const toast = useToast();
  const { confirm } = useDialogs();
  const tabsRef = useTabScroll<HTMLDivElement>();

  const [chainId, setChainId] = useState<ChainId>(((params.chain as ChainId) ?? "bitcoin"));
  const presetTo = (params.to as string | undefined) ?? "";
  const [to, setTo] = useState(presetTo);
  const [amount, setAmount] = useState("");
  const [fees, setFees] = useState<FeeOption[]>([]);
  const [selectedFee, setSelectedFee] = useState<string>("");
  const [quote, setQuote] = useState<SendQuote | null>(null);
  const [quoting, setQuoting] = useState(false);
  const [sending, setSending] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<SendResult | null>(null);
  const [pickingContact, setPickingContact] = useState(false);

  const account = wallet?.accounts.find((item) => item.chain === chainId);

  // Reset the form when switching chains; keep the preset recipient.
  useEffect(() => {
    setTo(presetTo);
    setAmount("");
    setFees([]);
    setSelectedFee("");
    setQuote(null);
    setError("");
  }, [chainId]); // eslint-disable-line react-hooks/exhaustive-deps

  // 旧版为输入防抖自动询价：地址+金额齐了就自动 requestQuote。
  useEffect(() => {
    if (!to.trim() || !amount.trim()) return;
    const timer = window.setTimeout(() => void requestQuote(), 500);
    return () => window.clearTimeout(timer);
  }, [to, amount]); // eslint-disable-line react-hooks/exhaustive-deps

  const requestQuote = async () => {
    if (!account || !to.trim() || !amount.trim()) return;
    setQuoting(true);
    setError("");
    try {
      const q = await api.quoteSend({
        chain: chainId,
        accountIndex: 0,
        to: to.trim(),
        amount: amount.trim(),
      });
      setQuote(q);
      setFees(q.feeOptions ?? []);
      const preferred = q.feeOptions?.find((f) => f.selected) ?? q.feeOptions?.[0];
      setSelectedFee(preferred?.level ?? "");
    } catch (e) {
      setQuote(null);
      setFees([]);
      setError((e as Error).message);
    } finally {
      setQuoting(false);
    }
  };

  const send = async () => {
    if (!quote || !selectedFee || sending) return;
    const fee = fees.find((f) => f.level === selectedFee);
    const ok = await confirm({
      title: "确认转账",
      body: `<div class="stack-sm">
        <div class="kv"><span class="kv-key">金额</span><span class="kv-value">${groupDigits(quote.amount)} ${quote.symbol}</span></div>
        <div class="kv"><span class="kv-key">收款方</span><span class="kv-value mono">${shortAddress(quote.to, 18, 10)}</span></div>
        <div class="kv"><span class="kv-key">手续费（${fee?.label ?? ""}）</span><span class="kv-value">${fee?.fee ?? "—"} ${fee?.feeFiat ? `(${fee.feeFiat})` : ""}</span></div>
      </div>
      <p class="hint" style="margin-top:10px">广播后无法撤回，请再次确认收款地址。</p>`,
      confirmText: "广播交易",
    });
    if (!ok) return;
    setSending(true);
    try {
      const receipt = await api.sendTransaction({
        chain: chainId,
        accountIndex: 0,
        to: quote.to,
        amount: quote.amount,
        feeLevel: selectedFee,
      });
      setResult(receipt);
      setQuote(null);
      setTo("");
      setAmount("");
      setFees([]);
      await reloadWallet();
      toast("交易已广播", "success");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setSending(false);
    }
  };

  if (!wallet || !account) {
    return (
      <section className="view">
        <p className="hint">该网络未启用。</p>
      </section>
    );
  }

  /* 成功后整页替换为回执视图（与旧版 successMarkup 一致）。 */
  if (result) {
    return (
      <section className="view">
        <div className="page-head">
          <div>
            <h1>已广播</h1>
            <div className="sub">交易已发送到 {result.chain} 网络，等待打包确认。</div>
          </div>
        </div>
        <div className="card card-pad stack" style={{ maxWidth: 640 }}>
          <div className="note note-ok">
            <Icon name="check" />
            <div>
              {result.amount} {result.symbol} 已发送到 {shortAddress(result.to, 14, 8)}
            </div>
          </div>
          <div className="kv">
            <span className="kv-key">交易哈希</span>
            <span className="kv-value mono">{result.txid}</span>
          </div>
          <div className="kv">
            <span className="kv-key">手续费</span>
            <span className="kv-value">
              {groupDigits(result.fee)} {result.symbol}
            </span>
          </div>
          <div className="row" style={{ flexWrap: "wrap" }}>
            <button
              className="btn btn-primary"
              type="button"
              data-action="copy-tx"
              onClick={async () => {
                const ok = await copyText(result.txid);
                toast(ok ? "交易哈希已复制" : "复制失败", ok ? "success" : "error");
              }}
            >
              <Icon name="copy" />
              复制交易哈希
            </button>
            <button
              className="btn"
              type="button"
              data-action="explorer"
              onClick={async () => {
                try {
                  await api.openExternal(result.explorerUrl);
                } catch (e) {
                  toast((e as Error).message, "error");
                }
              }}
            >
              <Icon name="external" />
              在浏览器中查看
            </button>
            <button
              className="btn btn-ghost"
              type="button"
              data-action="again"
              onClick={() => setResult(null)}
            >
              <Icon name="send" />
              再转一笔
            </button>
            <button
              className="btn btn-ghost"
              type="button"
              data-action="activity"
              onClick={() => {
                setResult(null);
                go("activity");
              }}
            >
              <Icon name="activity" />
              查看记录
            </button>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>转账</h1>
          <div className="sub">金额与手续费都由本地 Rust 引擎计算，签名永远不离开设备。</div>
        </div>
        <button className="btn" type="button" onClick={() => go("receive", { chain: chainId })}>
          <Icon name="receive" />
          收款
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

      <div className="send-layout">
        <div className="card card-pad stack">
          <div className="field">
            <div className="row-between">
              <label htmlFor="send-to">收款地址</label>
              <button
                className="btn btn-ghost btn-sm"
                type="button"
                data-action="pick-contact"
                onClick={() => {
                  const contacts = wallet.contacts ?? [];
                  if (contacts.length === 0) {
                    toast("地址簿还是空的，去「联系人」添加", "info");
                    return;
                  }
                  setPickingContact(true);
                }}
              >
                <Icon name="contacts" />
                从联系人选择
              </button>
            </div>
            <input
              className="input mono"
              id="send-to"
              spellCheck={false}
              autoComplete="off"
              placeholder={`粘贴 ${account.name} 地址`}
              value={to}
              onChange={(event) => {
                setTo(event.target.value);
                setQuote(null);
              }}
            />
          </div>

          <div className="field">
            <div className="row-between">
              <label htmlFor="send-amount">金额</label>
              <span className="hint">
                可用 {groupDigits(quote?.balance ?? "0")} {account.symbol}
              </span>
            </div>
            <div className="amount-input">
              <input
                id="send-amount"
                inputMode="decimal"
                placeholder="0.0"
                autoComplete="off"
                value={amount}
                onChange={(event) => {
                  setAmount(event.target.value);
                  setQuote(null);
                }}
              />
              <span className="unit">{account.symbol}</span>
              <button
                className="btn btn-ghost btn-sm"
                type="button"
                onClick={() => {
                  if (quote?.maxAmount) {
                    setAmount(String(quote.maxAmount));
                    setQuote(null);
                  }
                }}
              >
                全部
              </button>
            </div>
          </div>

          <div className="field">
            <span className="field-label">网络费</span>
            <div className="fee-list">
              {quoting ? (
                <>
                  <div className="skeleton" style={{ height: 56 }} />
                  <div className="skeleton" style={{ height: 56 }} />
                  <div className="skeleton" style={{ height: 56 }} />
                </>
              ) : fees.length === 0 ? (
                <p className="hint">填入地址和金额后自动获取费率。</p>
              ) : (
                fees.map((fee) => (
                  <button
                    key={fee.level}
                    className="fee-option"
                    type="button"
                    data-level={fee.level}
                    aria-pressed={selectedFee === fee.level}
                    onClick={() => setSelectedFee(fee.level)}
                  >
                    <span className="fee-radio" />
                    <span>
                      <span className="fee-name">{fee.label}</span>
                      <span className="fee-note">
                        {fee.note ?? ""} → {fee.eta}
                      </span>
                    </span>
                    <span className="fee-value">
                      {groupDigits(fee.fee)} <span className="muted small">{account.symbol}</span>
                      {fee.feeFiat ? <div className="fiat">{fee.feeFiat}</div> : null}
                    </span>
                  </button>
                ))
              )}
            </div>
            <p className="hint">{quote?.note ?? ""}</p>
          </div>

          {error ? <div className="note note-danger"><Icon name="warn" /><div>{error}</div></div> : null}
          {quote?.warning ? <div className="note note-warn"><Icon name="warn" /><div>{quote.warning}</div></div> : null}

          <div className="row" style={{ justifyContent: "flex-end" }}>
            <button
              className="btn btn-primary"
              type="button"
              disabled={!quote?.canSend || !selectedFee || sending}
              onClick={() => void send()}
            >
              <Icon name="send" />
              {sending ? "广播中…" : "确认转账"}
            </button>
          </div>
        </div>

        <div className="stack">
          <div className="card card-pad">
            <div className="section-title"><h2>摘要</h2></div>
            {!quote ? (
              <p className="hint">等待报价。</p>
            ) : (
              <div className="summary stack-sm">
                <div className="kv">
                  <span className="kv-key">金额</span>
                  <span className="kv-value">{groupDigits(amount || quote.amount)} {quote.symbol}</span>
                </div>
                <div className="kv">
                  <span className="kv-key">网络费（{fees.find((f) => f.level === selectedFee)?.label ?? ""}）</span>
                  <span className="kv-value">
                    {groupDigits(fees.find((f) => f.level === selectedFee)?.fee ?? "0")} {quote.symbol}
                  </span>
                </div>
                <div className="kv">
                  <span className="kv-key">转账后余额</span>
                  <span className="kv-value">{groupDigits(quote.balance)} {quote.symbol}</span>
                </div>
              </div>
            )}
          </div>
          <div className="card card-pad">
            <div className="section-title"><h2>当前账户</h2></div>
            <div className="kv"><span className="kv-key">地址</span><span className="kv-value mono">{shortAddress(account.address, 14, 8)}</span></div>
            <div className="kv"><span className="kv-key">派生</span><span className="kv-value">{account.derivation}</span></div>
            <div className="kv"><span className="kv-key">路径</span><span className="kv-value mono">{account.path}</span></div>
          </div>
          <div className="note">
            <Icon name="info" />
            <div>转账前请再次核对地址和网络。发错网络的钱无法找回。</div>
          </div>
        </div>
      </div>

      {pickingContact ? (
        <Modal title="选择联系人" onClose={() => setPickingContact(false)}>
          <div className="stack-sm" style={{ marginTop: 12 }}>
            {(wallet.contacts ?? []).map((contact) => (
              <button
                key={contact.id}
                className="btn btn-ghost btn-block"
                type="button"
                style={{ justifyContent: "flex-start" }}
                onClick={() => {
                  setPickingContact(false);
                  if (contact.chain !== chainId) setChainId(contact.chain);
                  setTo(contact.address);
                  setQuote(null);
                }}
              >
                <span className="dot" style={{ ["--accent" as string]: accentFor(contact.chain) }} />
                {contact.name} · {shortAddress(contact.address, 10, 6)}
              </button>
            ))}
          </div>
          <div className="modal-foot">
            <button className="btn btn-ghost" type="button" onClick={() => setPickingContact(false)}>
              取消
            </button>
          </div>
        </Modal>
      ) : null}
    </section>
  );
}

/** 联系人列表点的颜色（与 dashboard accentFor 同源）。 */
function accentFor(chain: ChainId): string {
  return (
    ({
      bitcoin: "#f7931a",
      litecoin: "#345d9d",
      dogecoin: "#c2a633",
      bitcoincash: "#0ac18e",
      monero: "#ff6600",
      solana: "#9945ff",
      bitcoinsv: "#eab300",
      namecoin: "#186c9d",
      litecoincash: "#bebebe",
      feathercoin: "#c6e4ee",
      viacoin: "#404e82",
      potcoin: "#3f8c48",
      egulden: "#f4812f",
      dash: "#008de4",
      groestlcoin: "#36855a",
      vertcoin: "#5bb5a0",
    } as Record<string, string>)[chain] ?? "#8a7cff"
  );
}
