/** Send: chain picker, recipient, amount, fee selection, confirm + result.
 *  Ported from ui/js/views/send.js (quote → confirm → broadcast). */

import { useEffect, useState } from "react";

import { api } from "../api";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { groupDigits, shortAddress } from "../format";
import type { ChainId, FeeOption, SendQuote, SendResult } from "../types";

export function SendView() {
  const { wallet, params, go, reloadWallet } = useSession();
  const toast = useToast();
  const { confirm } = useDialogs();

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

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>转账</h1>
          <div className="sub">所有签名都在本机完成；交易直接广播到对应网络。</div>
        </div>
        <button className="btn" type="button" onClick={() => go("receive", { chain: chainId })}>
          <Icon name="receive" />
          去收款
        </button>
      </div>

      <div className="tabs" role="tablist" style={{ marginBottom: 16 }}>
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

      <div className="card card-pad stack">
        <div className="field">
          <label htmlFor="send-to">收款地址</label>
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
        <div className="row" style={{ gap: 12, alignItems: "flex-end" }}>
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="send-amount">金额（可用 {groupDigits(account.address ? quote?.balance ?? "…" : "…")} {account.symbol}）</label>
            <input
              className="input"
              id="send-amount"
              type="text"
              inputMode="decimal"
              placeholder="0.00"
              value={amount}
              onChange={(event) => {
                setAmount(event.target.value);
                setQuote(null);
              }}
            />
          </div>
          <button className="btn" type="button" disabled={quoting || !to.trim() || !amount.trim()} onClick={() => void requestQuote()}>
            {quoting ? "询价中…" : "获取费率"}
          </button>
        </div>

        {quote?.warning ? (
          <div className="note note-warn">
            <Icon name="warn" />
            <div>{quote.warning}</div>
          </div>
        ) : null}
        {quote?.note ? (
          <div className="note">
            <Icon name="info" />
            <div>{quote.note}</div>
          </div>
        ) : null}

        {fees.length > 0 ? (
          <div className="field">
            <span className="field-label">手续费</span>
            <div className="fee-grid">
              {fees.map((fee) => (
                <button
                  key={fee.level}
                  className={`fee-card${selectedFee === fee.level ? " is-selected" : ""}`}
                  type="button"
                  aria-pressed={selectedFee === fee.level}
                  onClick={() => setSelectedFee(fee.level)}
                >
                  <div className="fee-label">{fee.label}</div>
                  <div className="fee-value">
                    {fee.fee} {account.symbol}
                  </div>
                  {fee.feeFiat ? <div className="fee-fiat">{fee.feeFiat}</div> : null}
                  <div className="fee-eta muted small">{fee.eta}</div>
                  {fee.note ? <div className="fee-note muted small">{fee.note}</div> : null}
                </button>
              ))}
            </div>
          </div>
        ) : null}

        {error ? <p className="error-text">{error}</p> : null}

        <div className="modal-foot" style={{ justifyContent: "flex-end" }}>
          <button
            className="btn btn-primary"
            type="button"
            disabled={!quote?.canSend || !selectedFee || sending}
            onClick={() => void send()}
          >
            {sending ? "广播中…" : "签名并广播"}
          </button>
        </div>
      </div>

      {result ? (
        <Modal title="转账成功" onClose={() => setResult(null)}>
          <div className="stack-sm" style={{ marginTop: 10 }}>
            <div className="kv">
              <span className="kv-key">金额</span>
              <span className="kv-value">
                {groupDigits(result.amount)} {result.symbol}
              </span>
            </div>
            <div className="kv">
              <span className="kv-key">手续费</span>
              <span className="kv-value">
                {groupDigits(result.fee)} {result.symbol}
              </span>
            </div>
            <div className="kv">
              <span className="kv-key">收款方</span>
              <span className="kv-value mono">{shortAddress(result.to, 18, 10)}</span>
            </div>
            <span className="field-label">交易哈希</span>
            <div className="copy-field">
              <span className="value mono">{shortAddress(result.txid, 20, 10)}</span>
              <button
                className="btn btn-ghost btn-sm"
                type="button"
                onClick={async () => {
                  try {
                    await navigator.clipboard.writeText(result.txid);
                    toast("已复制", "success");
                  } catch {
                    toast("复制失败", "error");
                  }
                }}
              >
                <Icon name="copy" />
              </button>
            </div>
            <div className="note">
              <Icon name="check" />
              <div>交易已广播。确认进度可在区块浏览器查看，记录已保存到本机交易历史。</div>
            </div>
          </div>
          <div className="modal-foot">
            <button className="btn" type="button" onClick={() => setResult(null)}>
              完成
            </button>
            <button
              className="btn btn-primary"
              type="button"
              onClick={async () => {
                try {
                  await api.openExternal(result.explorerUrl);
                } catch {
                  // best-effort
                }
              }}
            >
              <Icon name="external" />
              区块浏览器
            </button>
          </div>
        </Modal>
      ) : null}
    </section>
  );
}
