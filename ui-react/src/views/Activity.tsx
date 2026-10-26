/** Activity: transactions recorded on this device, with filters and a detail
 *  dialog. Ported from ui/js/views/activity.js. */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { api } from "../api";
import { useSession } from "../shell/session";
import { CopyField, Modal, useTabScroll, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { formatDateTime, groupDigits, shortAddress } from "../format";
import type { ActivityItem } from "../types";

const STATUS: Record<string, { label: string; cls: string }> = {
  pending: { label: "待确认", cls: "badge badge-warn" },
  confirmed: { label: "已确认", cls: "badge badge-ok" },
  failed: { label: "失败", cls: "badge badge-danger" },
};

const EXPLORERS: Record<string, string> = {
  bitcoin: "https://mempool.space/tx/",
  litecoin: "https://litecoinspace.org/tx/",
  ethereum: "https://etherscan.io/tx/",
  bsc: "https://bscscan.com/tx/",
  polygon: "https://polygonscan.com/tx/",
  solana: "https://solscan.io/tx/",
};

function explorerUrl(item: ActivityItem): string {
  const withUrl = item as ActivityItem & { explorerUrl?: string };
  if (withUrl.explorerUrl?.startsWith("http")) return withUrl.explorerUrl;
  return `${EXPLORERS[item.chain] ?? ""}${item.txid}`;
}

function dayLabel(timestamp: number): string {
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const sameDay = (a: Date, b: Date) => a.toDateString() === b.toDateString();
  if (sameDay(date, now)) return "今天";
  const yesterday = new Date(now.getTime() - 86_400_000);
  if (sameDay(date, yesterday)) return "昨天";
  const month = date.getMonth() + 1;
  const day = date.getDate();
  return date.getFullYear() === now.getFullYear()
    ? `${month} 月 ${day} 日`
    : `${date.getFullYear()} 年 ${month} 月 ${day} 日`;
}

function ActivityRow({ item, onOpen }: { item: ActivityItem; onOpen: () => void }) {
  const meta = STATUS[item.status] ?? STATUS.pending;
  const send = item.direction === "send";
  return (
    <div
      className="activity-item"
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(event) => {
        if (event.key === "Enter" || event.key === " ") {
          event.preventDefault();
          onOpen();
        }
      }}
    >
      <span className={`dir-mark ${send ? "dir-send" : "dir-receive"}`}>
        <Icon name={send ? "send" : "receive"} />
      </span>
      <div className="activity-main">
        <div className="title">
          {item.symbol}
          {send ? "发送" : "接收"} <span className={meta.cls}>{meta.label}</span>
        </div>
        <div className="sub">
          {shortAddress(item.counterparty, 16, 8)} · {shortAddress(item.txid, 12, 6)}
        </div>
      </div>
      <div className="activity-amount">
        <div style={send ? undefined : { color: "var(--ok)" }}>
          {send ? "−" : "+"}
          {groupDigits(item.amount)} <span className="muted small">{item.symbol}</span>
        </div>
        <div className="fee">{formatDateTime(item.timestamp)}</div>
      </div>
    </div>
  );
}

function DetailDialog({ item, onClose }: { item: ActivityItem; onClose: () => void }) {
  const meta = STATUS[item.status] ?? STATUS.pending;
  const send = item.direction === "send";
  const toast = useToast();
  const itemAny = item as ActivityItem & { fee?: string };

  return (
    <Modal title={`${item.symbol}${send ? "发送" : "接收"}详情`} onClose={onClose}>
      <div className="stack-sm" style={{ marginTop: 10 }}>
        <div className="kv">
          <span className="kv-key">状态</span>
          <span className="kv-value">
            <span className={meta.cls}>{meta.label}</span>
          </span>
        </div>
        <div className="kv">
          <span className="kv-key">金额</span>
          <span className="kv-value">
            {send ? "−" : "+"}
            {groupDigits(item.amount)} {item.symbol}
          </span>
        </div>
        {itemAny.fee ? (
          <div className="kv">
            <span className="kv-key">手续费</span>
            <span className="kv-value">
              {groupDigits(itemAny.fee)} {item.symbol}
            </span>
          </div>
        ) : null}
        <div className="kv">
          <span className="kv-key">时间</span>
          <span className="kv-value">{formatDateTime(item.timestamp)}</span>
        </div>
        <span className="field-label">{send ? "收款地址" : "来源地址"}</span>
        <CopyField value={item.counterparty} />
        <span className="field-label">交易哈希</span>
        <CopyField value={item.txid} />
        <div className="note">
          <Icon name="info" />
          <div>这条记录只保存在本机，状态不会自动从网络更新。最终状态请以区块浏览器为准。</div>
        </div>
      </div>
      <div className="modal-foot">
        <button className="btn btn-ghost" type="button" onClick={onClose}>
          关闭
        </button>
        <button
          className="btn btn-primary"
          type="button"
          onClick={async () => {
            try {
              await api.openExternal(explorerUrl(item));
            } catch (error) {
              toast((error as Error).message, "error");
            }
          }}
        >
          <Icon name="external" />
          区块浏览器
        </button>
      </div>
    </Modal>
  );
}

export function ActivityView() {
  const { wallet, reloadWallet, go } = useSession();
  const chainsRef = useTabScroll<HTMLDivElement>();
  const [chain, setChain] = useState<string>("all");
  const [dir, setDir] = useState<string>("all");
  const [query, setQuery] = useState("");
  const [detail, setDetail] = useState<ActivityItem | null>(null);
  const [syncing, setSyncing] = useState(false);
  const [syncHint, setSyncHint] = useState("");
  const syncingRef = useRef(false);

  const sync = useCallback(async () => {
    if (syncingRef.current) return;
    syncingRef.current = true;
    setSyncing(true);
    try {
      const report = await api.refreshActivity();
      const parts: string[] = [];
      if (report.newItems > 0) parts.push(`新收款 ${report.newItems} 笔`);
      if (report.confirmedItems > 0) parts.push(`${report.confirmedItems} 笔已确认`);
      if (parts.length === 0) parts.push("已同步");
      if (report.failedChains.length > 0) {
        parts.push(`${report.failedChains.join("、")} 暂不可用`);
      }
      setSyncHint(parts.join(" · "));
      if (report.newItems > 0 || report.confirmedItems > 0) await reloadWallet();
    } catch {
      setSyncHint("同步失败，稍后重试");
    } finally {
      syncingRef.current = false;
      setSyncing(false);
    }
  }, [reloadWallet]);

  // Pull on-chain history when the tab opens, then every minute while open.
  useEffect(() => {
    if (!wallet) return;
    void sync();
    const timer = window.setInterval(() => void sync(), 60_000);
    return () => window.clearInterval(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [wallet?.id]);

  const all = useMemo(
    () => [...(wallet?.activity ?? [])].sort((a, b) => b.timestamp - a.timestamp),
    [wallet?.activity],
  );

  const chains = useMemo(() => {
    const seen = new Map<string, number>();
    for (const item of all) seen.set(item.chain, (seen.get(item.chain) ?? 0) + 1);
    return [...seen.entries()];
  }, [all]);

  const matches = useMemo(() => {
    const q = query.toLowerCase();
    return all.filter((item) => {
      if (chain !== "all" && item.chain !== chain) return false;
      if (dir !== "all" && item.direction !== dir) return false;
      if (!q) return true;
      return (
        item.txid.toLowerCase().includes(q) ||
        item.counterparty.toLowerCase().includes(q) ||
        item.symbol.toLowerCase().includes(q)
      );
    });
  }, [all, chain, dir, query]);

  // Group by day with day headers, matching the old renderer.
  const groups = useMemo(() => {
    const out: Array<{ key: string; label: string; items: ActivityItem[] }> = [];
    let lastKey = "";
    for (const item of matches) {
      const date = new Date(item.timestamp * 1000);
      const key = `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
      if (key !== lastKey) {
        out.push({ key, label: dayLabel(item.timestamp), items: [] });
        lastKey = key;
      }
      out[out.length - 1].items.push(item);
    }
    return out;
  }, [matches]);

  if (all.length === 0) {
    return (
      <section className="view">
        <div className="page-head">
          <div>
            <h1>交易记录</h1>
            <div className="sub">本机发起的转账自动记录；打开本页会从链上同步收款记录。</div>
          </div>
          <button className="btn" type="button" onClick={() => go("send")}>
            <Icon name="send" />
            去转账
          </button>
        </div>
        <div className="card card-pad">
          <div className="empty">
            <div className="mark">
              <Icon name="activity" />
            </div>
            还没有交易记录。
            <br />
            收到的转账会在打开本页时自动同步；发送后也会立即记录。
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>交易记录</h1>
          <div className="sub">本机发起的转账自动记录；收款记录从链上索引同步。</div>
        </div>
        <div className="row" style={{ gap: 8 }}>
          <button
            className="btn"
            type="button"
            disabled={syncing}
            onClick={() => void sync()}
            title="从链上同步最新交易"
          >
            <Icon name={syncing ? "activity" : "refresh"} />
            {syncing ? "同步中…" : "同步"}
          </button>
          <button className="btn" type="button" onClick={() => go("send")}>
            <Icon name="send" />
            去转账
          </button>
        </div>
      </div>
      {syncHint && (
        <div className="card card-pad" style={{ padding: "8px 14px", marginBottom: 10 }}>
          <span className="muted small">{syncHint}</span>
        </div>
      )}

      <div className="card card-pad">
        <div className="row-between" style={{ flexWrap: "wrap", gap: 10, marginBottom: 10 }}>
          <div className="tabs" role="tablist" id="activity-chains" ref={chainsRef}>
            <button className="tab" type="button" data-chain="all" aria-selected={chain === "all"} onClick={() => setChain("all")}>
              全部
            </button>
            {chains.map(([id, count]) => (
              <button
                key={id}
                className="tab"
                type="button"
                data-chain={id}
                aria-selected={chain === id}
                onClick={() => setChain(id)}
              >
                <span className="dot" />
                {id} {count}
              </button>
            ))}
          </div>
          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            <div className="seg" role="group" id="activity-dir" aria-label="方向">
              <button type="button" data-dir="all" aria-pressed={dir === "all"} onClick={() => setDir("all")}>
                全部
              </button>
              <button type="button" data-dir="send" aria-pressed={dir === "send"} onClick={() => setDir("send")}>
                发送
              </button>
              <button type="button" data-dir="receive" aria-pressed={dir === "receive"} onClick={() => setDir("receive")}>
                接收
              </button>
            </div>
            <input
              className="input"
              id="activity-search"
              placeholder="搜索地址或交易哈希"
              autoComplete="off"
              spellCheck={false}
              style={{ width: 210 }}
              value={query}
              onChange={(event) => setQuery(event.target.value.trim())}
            />
          </div>
        </div>
        <div id="activity-list">
          {matches.length === 0 ? (
            <div className="empty">没有符合筛选条件的记录。</div>
          ) : (
            groups.map((group) => (
              <div key={group.key}>
                <div className="day-label">{group.label}</div>
                {group.items.map((item) => (
                  <ActivityRow key={item.id} item={item} onOpen={() => setDetail(item)} />
                ))}
              </div>
            ))
          )}
        </div>
      </div>
      {detail ? <DetailDialog item={detail} onClose={() => setDetail(null)} /> : null}
    </section>
  );
}
