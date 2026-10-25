/** Wallet management: the vault's wallet list, switching, rename, derived
 *  account overview and batch creation. Ported from ui/js/views/wallets.js. */

import { useEffect, useState } from "react";

import { api } from "../api";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import type { BatchRequest, BatchStrategy, DerivationPreview, WalletView } from "../types";

const STRATEGY_LABELS: Record<string, { title: string; desc: string }> = {
  path: { title: "路径派生", desc: "BIP-32 地址索引递增（助记词）" },
  salted: { title: "加盐派生", desc: "H(域名‖密语‖#序号)" },
  iterated: { title: "迭代哈希", desc: "基于基础私钥连续哈希 N 次" },
};

/** 派生策略按钱包模式过滤：助记词走 BIP-32 路径（+迭代哈希）；
 * 加盐派生以口令为哈希输入，属于口令钱包专属，助记词不提供。 */
function strategiesFor(wallet: WalletView | null) {
  const all = Object.entries(STRATEGY_LABELS);
  if (!wallet || wallet.mode === "mnemonic") {
    return all.filter(([key]) => key !== "salted");
  }
  return all.filter(([key]) => key !== "path");
}

function BatchDialog({
  wallet,
  onClose,
  onDone,
}: {
  wallet: WalletView;
  onClose: () => void;
  onDone: (view: WalletView) => void;
}) {
  const strategies = strategiesFor(wallet);
  const [strategy, setStrategy] = useState<BatchStrategy>(strategies[0][0] as BatchStrategy);
  const [count, setCount] = useState(1);
  const [start, setStart] = useState(1);
  const [name, setName] = useState("");
  const [previews, setPreviews] = useState<DerivationPreview[] | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const toast = useToast();

  const minStart = strategy === "iterated" ? 1 : 0;
  const effectiveStart = Math.max(start, minStart);
  const max = strategy === "iterated" ? 50 : 100;

  const request: BatchRequest = { strategy, chain: "bitcoin", count, start: effectiveStart };

  const preview = async () => {
    setBusy(true);
    setError("");
    try {
      setPreviews(await api.previewBatch(request));
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const create = async () => {
    setBusy(true);
    setError("");
    try {
      const view = await api.addWalletsBatch({ batch: request, name: name.trim() || null });
      onDone(view);
      onClose();
      toast(`已添加 ${count} 个派生账户`, "success");
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <Modal title="批量派生账户" onClose={onClose}>
      <p className="hint" style={{ margin: "6px 0 12px" }}>
        在 Bitcoin 上按策略批量派生账户（资产页可为其他币种单独添加）。只保存配方，不保存密钥。
      </p>
      <div className="field" style={{ marginBottom: 16 }}>
        <span className="field-label">派生方式</span>
        <div className="seg seg-col" role="group">
          {strategies.map(([key, meta]) => (
            <button
              key={key}
              type="button"
              aria-pressed={strategy === key}
              onClick={() => {
                setStrategy(key as BatchStrategy);
                setPreviews(null);
                if (key === "iterated") setStart((v) => Math.max(v, 1));
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
          <label htmlFor="batch-count">数量（1–{max}）</label>
          <input
            className="input"
            id="batch-count"
            type="number"
            min={1}
            max={max}
            value={count}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value)) {
                setCount(Math.max(1, Math.min(max, Math.floor(value))));
                setPreviews(null);
              }
            }}
          />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label htmlFor="batch-start">起始序号{minStart > 0 ? "（≥1）" : ""}</label>
          <input
            className="input"
            id="batch-start"
            type="number"
            min={minStart}
            max={max - 1}
            value={effectiveStart}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value)) {
                setStart(Math.max(minStart, Math.min(max - 1, Math.floor(value))));
                setPreviews(null);
              }
            }}
          />
        </div>
        <div className="field" style={{ flex: 1 }}>
          <label htmlFor="batch-name">名称前缀（可选）</label>
          <input
            className="input"
            id="batch-name"
            type="text"
            maxLength={40}
            placeholder={`默认「${wallet.label} #序号」`}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </div>
      </div>
      {previews ? (
        <div className="stack-sm" style={{ margin: "12px 0" }}>
          {previews.map((item, index) => (
            <div className="kv" key={index}>
              <span className="kv-key">#{effectiveStart + index}</span>
              <span className="kv-value mono">
                {shortAddr(item.accounts[0]?.address)} · 指纹 {item.fingerprint}
              </span>
            </div>
          ))}
        </div>
      ) : null}
      {error ? <p className="error-text">{error}</p> : null}
      <div className="modal-foot">
        <button className="btn btn-ghost" type="button" onClick={onClose}>
          取消
        </button>
        {!previews ? (
          <button className="btn btn-primary" type="button" disabled={busy} onClick={() => void preview()}>
            {busy ? "预览中…" : "预览"}
          </button>
        ) : (
          <button className="btn btn-primary" type="button" disabled={busy} onClick={() => void create()}>
            {busy ? "创建中…" : `添加 ${count} 个`}
          </button>
        )}
      </div>
    </Modal>
  );
}

function shortAddr(value: string | undefined): string {
  if (!value) return "—";
  return value.length <= 22 ? value : `${value.slice(0, 12)}…${value.slice(-6)}`;
}

export function WalletsView() {
  const { wallet, setWallet } = useSession();
  const toast = useToast();
  const { askPassword } = useDialogs();
  const [renaming, setRenaming] = useState<{ id: string; label: string } | null>(null);
  const [adding, setAdding] = useState(false);
  const [creating, setCreating] = useState(false);

  if (!wallet) return null;

  const switchTo = async (id: string) => {
    if (id === wallet.id) return;
    try {
      setWallet(await api.switchWallet(id));
      toast("已切换钱包", "success");
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  const rename = async () => {
    if (!renaming) return;
    try {
      setWallet(await api.renameWallet(renaming.id, renaming.label.trim()));
      setRenaming(null);
      toast("已重命名", "success");
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  const remove = async (id: string, label: string) => {
    const password = await askPassword({
      title: `删除钱包「${label}」`,
      description: "需要验证本机密码。删除后该钱包的派生账户配置一并移除；助记词本身不受影响（如有备份仍可恢复）。",
      confirmText: "删除",
    });
    if (!password) return;
    try {
      setWallet(await api.deleteWallet(id, password));
      toast("钱包已删除", "success");
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>钱包管理</h1>
          <div className="sub">一个加密文件保管多个钱包，共享联系人与设置，交易历史各自独立。</div>
        </div>
        <div className="row" style={{ gap: 8 }}>
          <button className="btn btn-primary" type="button" onClick={() => setCreating(true)}>
            <Icon name="plus" />
            新建钱包
          </button>
          <button className="btn" type="button" onClick={() => setAdding(true)}>
            <Icon name="layers" />
            批量派生
          </button>
        </div>
      </div>

      <div className="card card-pad">
        <div className="section-title">
          <h2>全部钱包</h2>
          <span className="hint">
            共 {wallet.wallets.length} 个 · 每个钱包有独立的账户、资产与记录
          </span>
        </div>
        <div className="wallet-list">
          {wallet.wallets.map((item) => (
            <div key={item.id} className={`wallet-row${item.active ? " is-active" : ""}`}>
              <div className="wallet-main" role="button" tabIndex={0} onClick={() => void switchTo(item.id)}>
                <div className="wallet-title">
                  {item.label}
                  {item.active ? <span className="chip chip-ok">当前</span> : null}
                  {item.mode === "passphrase" ? <span className="chip">口令</span> : <span className="chip">助记词</span>}
                </div>
                <div className="wallet-sub muted small">
                  指纹 {item.fingerprint} · {(item.activityCount ?? 0)} 条记录
                </div>
              </div>
              <div className="wallet-actions">
                <button
                  className="btn btn-ghost btn-sm"
                  type="button"
                  title="重命名"
                  onClick={() => setRenaming({ id: item.id, label: item.label })}
                >
                  <Icon name="edit" />
                </button>
                {!item.active ? (
                  <button
                    className="btn btn-ghost btn-sm"
                    type="button"
                    title="删除钱包"
                    style={{ color: "var(--danger)" }}
                    onClick={() => void remove(item.id, item.label)}
                  >
                    <Icon name="trash" />
                  </button>
                ) : null}
              </div>
            </div>
          ))}
        </div>

        <p className="hint" style={{ marginTop: 12 }}>
          所有钱包共享同一个本机密码。派生账户只保存「配方公式 + 序号」，不占空间；只要记住创建/导入时的密码，随时能在全新设备上完整恢复。账户切换入口在「资产」页中。
        </p>
      </div>

      {renaming ? (
        <Modal title="重命名钱包" onClose={() => setRenaming(null)}>
          <div className="field">
            <label htmlFor="rename-label">名称</label>
            <input
              className="input"
              id="rename-label"
              maxLength={40}
              value={renaming.label}
              onChange={(event) => setRenaming({ ...renaming, label: event.target.value })}
            />
          </div>
          <div className="modal-foot">
            <button className="btn btn-ghost" type="button" onClick={() => setRenaming(null)}>
              取消
            </button>
            <button className="btn btn-primary" type="button" onClick={() => void rename()}>
              保存
            </button>
          </div>
        </Modal>
      ) : null}
      {creating ? (
        <AddWalletDialog
          onClose={() => setCreating(false)}
          onDone={(view) => {
            setCreating(false);
            setWallet(view);
          }}
        />
      ) : null}
      {adding ? <BatchDialog wallet={wallet} onClose={() => setAdding(false)} onDone={setWallet} /> : null}
    </section>
  );
}

/** 新建钱包弹窗：口令（哈希派生）或助记词（BIP-39），实时派生预览。
 *  移植自 ui/js/views/wallets.js addFlow。 */
function AddWalletDialog({
  onClose,
  onDone,
}: {
  onClose: () => void;
  onDone: (view: WalletView) => void;
}) {
  const toast = useToast();
  const [mode, setMode] = useState<"passphrase" | "mnemonic">("passphrase");
  const [phrase, setPhrase] = useState("");
  const [derivation, setDerivation] = useState<"isolated" | "strict">("isolated");
  const [mnemonic, setMnemonic] = useState("");
  const [name, setName] = useState("");
  const [preview, setPreview] = useState<DerivationPreview | null>(null);
  const [previewing, setPreviewing] = useState(false);
  const [previewError, setPreviewError] = useState("");
  const [busy, setBusy] = useState(false);

  /* 实时派生预览：380ms 防抖（与旧版一致）。 */
  useEffect(() => {
    const value = mode === "passphrase" ? phrase : mnemonic;
    if (!value.trim()) {
      setPreview(null);
      setPreviewError("");
      setPreviewing(false);
      return;
    }
    setPreviewing(true);
    const timer = window.setTimeout(async () => {
      try {
        setPreview(
          await api.previewDerivation(
            mode === "passphrase"
              ? { mode: "passphrase", phrase, derivation }
              : { mode: "mnemonic", mnemonic: mnemonic.trim() },
          ),
        );
        setPreviewError("");
      } catch (e) {
        setPreview(null);
        setPreviewError((e as Error).message);
      } finally {
        setPreviewing(false);
      }
    }, 380);
    return () => window.clearTimeout(timer);
  }, [mode, phrase, derivation, mnemonic]);

  const save = async () => {
    if (busy) return;
    setBusy(true);
    try {
      const view = await api.addWallet({
        name: name.trim() || null,
        secret:
          mode === "passphrase"
            ? { mode: "passphrase" as const, phrase, derivation }
            : { mode: "mnemonic" as const, mnemonic: mnemonic.trim() },
      });
      onDone(view);
      toast(`已创建并切换到「${view.label}」`, "success");
    } catch (e) {
      setBusy(false);
      toast((e as Error).message, "error");
    }
  };

  const inputFilled = mode === "passphrase" ? Boolean(phrase.trim()) : Boolean(mnemonic.trim());

  return (
    <Modal title="新建钱包" onClose={onClose}>
      <p className="hint" style={{ marginTop: -6, marginBottom: 12 }}>
        在当前保险库里添加一个钱包，并立即切换过去；其他钱包不受影响。
      </p>

      <div className="seg" role="group" aria-label="钱包类型" style={{ marginBottom: 14 }}>
        <button type="button" data-mode="passphrase" aria-pressed={mode === "passphrase"} onClick={() => setMode("passphrase")}>
          口令（哈希派生）
        </button>
        <button type="button" data-mode="mnemonic" aria-pressed={mode === "mnemonic"} onClick={() => setMode("mnemonic")}>
          助记词（BIP-39）
        </button>
      </div>

      {mode === "passphrase" ? (
        <>
          <div className="field">
            <label htmlFor="wallet-phrase">口令</label>
            <textarea
              className="textarea"
              id="wallet-phrase"
              placeholder="例如：a quiet garden at dusk"
              autoComplete="off"
              spellCheck={false}
              value={phrase}
              onChange={(event) => setPhrase(event.target.value)}
            />
          </div>
          <div className="field">
            <span className="field-label">派生方式</span>
            <div className="seg" role="group">
              <button
                type="button"
                data-derivation="isolated"
                aria-pressed={derivation === "isolated"}
                onClick={() => setDerivation("isolated")}
              >
                隔离（推荐）
              </button>
              <button
                type="button"
                data-derivation="strict"
                aria-pressed={derivation === "strict"}
                onClick={() => setDerivation("strict")}
              >
                严格 H(phrase)
              </button>
            </div>
          </div>
        </>
      ) : (
        <div className="field">
          <label htmlFor="wallet-mnemonic">助记词</label>
          <textarea
            className="textarea"
            id="wallet-mnemonic"
            placeholder="粘贴已有的 12 / 24 个英文单词"
            spellCheck={false}
            autoComplete="off"
            value={mnemonic}
            onChange={(event) => setMnemonic(event.target.value)}
          />
        </div>
      )}

      <div className="field">
        <label htmlFor="wallet-name">名称（可选）</label>
        <input
          className="input"
          id="wallet-name"
          placeholder={mode === "passphrase" ? "口令钱包" : "助记词钱包"}
          maxLength={40}
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
      </div>

      {preview || previewing || previewError ? (
        <div className="stack-sm" style={{ margin: "10px 0" }}>
          {previewing ? (
            <div className="skeleton" style={{ height: 64 }} />
          ) : previewError ? (
            <div className="note note-warn">
              <Icon name="warn" />
              <div>{previewError}</div>
            </div>
          ) : preview ? (
            <p className="hint">
              指纹 <span className="fingerprint">{preview.fingerprint}</span> ·{" "}
              {preview.formula}
            </p>
          ) : null}
        </div>
      ) : null}

      <div className="modal-foot">
        <button className="btn btn-ghost" type="button" onClick={onClose}>
          取消
        </button>
        <button className="btn btn-primary" type="button" disabled={!inputFilled || busy} onClick={() => void save()}>
          {busy ? "添加中…" : "创建钱包"}
        </button>
      </div>
    </Modal>
  );
}
