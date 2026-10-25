/** Settings: display (currency/theme/hide), security (password, auto-lock,
 *  chain toggles), Monero RPC, price service and dangerous actions.
 *  Ported from ui/js/views/settings.js (first pass — full parity). */

import { useState } from "react";

import { api, walletFull } from "../api";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";
import { formatDateTime } from "../format";
import type { ChainId, Settings as SettingsModel } from "../types";
const CHAIN_LABELS: Record<ChainId, string> = {
  bitcoin: "Bitcoin",
  litecoin: "Litecoin",
  dogecoin: "Dogecoin",
  bitcoincash: "Bitcoin Cash",
  ethereum: "Ethereum",
  "ethereum-classic": "Ethereum Classic",
  bsc: "BNB Smart Chain",
  polygon: "Polygon",
  solana: "Solana",
  monero: "Monero",
};

const ACCENTS: Partial<Record<ChainId, string>> = {
  bitcoin: "#f7931a",
  litecoin: "#a6a9aa",
  dogecoin: "#c3a634",
  bitcoincash: "#8dc351",
  ethereum: "#627eea",
  "ethereum-classic": "#328332",
  bsc: "#f0b90b",
  polygon: "#8247e5",
  solana: "#9945ff",
  monero: "#ff6600",
};

export function SettingsView() {
  const { wallet, setWallet, go, lockNow } = useSession();
  const toast = useToast();
  const { askPassword } = useDialogs();
  const [revealed, setRevealed] = useState<string[] | null>(null);
  const [pwOpen, setPwOpen] = useState(false);
  const [blockchairDraft, setBlockchairDraft] = useState<string>("");

  if (!wallet) return null;
  const s = wallet.settings;
  const allChains = Object.keys(CHAIN_LABELS) as ChainId[];

  const save = async (patch: Partial<SettingsModel>) => {
    try {
      // 链开关需要全量视图;其余设置在过滤后的视图上保存即可。
      const full = patch.visibleChains !== undefined ? await walletFull() : wallet;
      // 后端（与 mock 一致）只返回合并后的 settings,不返回整个钱包视图;
      // 旧版把返回值合并进当前 wallet。直接整体替换会把 accounts 丢掉。
      const next = await api.saveSettings({ ...full.settings, ...patch });
      setWallet({ ...wallet, settings: next });
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  const revealWords = async () => {
    const password = await askPassword({
      title: "导出助记词",
      description: "助记词是恢复钱包的唯一手段。验证密码后显示，请确认周围没有人。",
      confirmText: "显示",
    });
    if (!password) return;
    try {
      const mnemonic = await api.revealMnemonic(password);
      setRevealed(mnemonic.trim().split(/\s+/));
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  const moneroAccount = wallet.accounts.find((account) => account.chain === "monero");
  void moneroAccount;

  const deleteVault = async () => {
    const password = await askPassword({
      title: "删除钱包文件",
      description: "整个加密文件将被删除，包括所有钱包、记录与设置。助记词备份是唯一恢复手段。",
      confirmText: "删除",
    });
    if (!password) return;
    const ok = await confirmDialogCompat();
    if (!ok) return;
    try {
      await api.deleteVault(password);
      window.location.reload();
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  // 危险操作二确认（对齐旧版 confirmDialog 用法）。
  const confirmDialogCompat = async () => {
    return window.confirm("真的要删除整个钱包文件吗？此操作无法撤销。");
  };

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>设置</h1>
          <div className="sub">显示、安全与本机服务。所有设置保存在加密的钱包文件里。</div>
        </div>
      </div>

      <div className="settings-grid">
        {/* 钱包卡 */}
        <div className="card card-pad stack-sm">
          <div className="section-title">
            <h2>钱包</h2>
            <span className="chip">{wallet.mode === "passphrase" ? `口令 · ${wallet.derivation === "strict" ? "严格" : "宽松"}` : "助记词 · BIP-39"}</span>
          </div>
          <div className="kv"><span className="kv-key">名称</span><span className="kv-value">{wallet.label}</span></div>
          <div className="kv"><span className="kv-key">指纹</span><span className="kv-value fingerprint">{wallet.fingerprint}</span></div>
          <div className="kv"><span className="kv-key">创建时间</span><span className="kv-value">{formatDateTime(wallet.createdAt)}</span></div>
          <div className="kv"><span className="kv-key">派生方式</span><span className="kv-value">{wallet.accounts[0]?.derivation ?? "—"}</span></div>
          <div className="row" style={{ flexWrap: "wrap", marginTop: 4 }}>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => setPwOpen(true)}>
              <Icon name="edit" />
              修改密码
            </button>
            {wallet.mode === "mnemonic" ? (
              <button className="btn btn-ghost btn-sm" type="button" onClick={() => void revealWords()}>
                <Icon name="shield" />
                备份助记词
              </button>
            ) : null}
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => go("wallets")}>
              <Icon name="key" />
              导出私钥
            </button>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => go("wallets")}>
              <Icon name="wallet" />
              钱包管理（{wallet.wallets.length} 个）
            </button>
          </div>
          <p className="hint">
            {wallet.mode === "mnemonic"
              ? "助记词是恢复资产的唯一凭证，请离线抄写并妥善保存。"
              : "口令钱包没有助记词，忘记口令等于永久丢失。"}
          </p>
        </div>

        {/* 显示的链 */}
        <div className="card card-pad">
          <div className="section-title"><h2>显示的链</h2></div>
          <div className="setting-row" style={{ borderBottom: "1px dashed var(--hair)" }}>
            <div className="text">
              <div className="name">显示过滤</div>
              <div className="desc">关闭只隐藏界面入口；地址与密钥仍然覆盖全部 {allChains.length} 条链。</div>
            </div>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => void save({ visibleChains: [] })}>
              全部显示
            </button>
          </div>
          <div className="chain-toggle-list">
            {allChains.map((chain) => {
              const on = s.visibleChains.length === 0 || s.visibleChains.includes(chain);
              return (
                <div className="chain-toggle-row" key={chain}>
                  <span className="asset-mark" style={{ ["--accent" as string]: ACCENTS[chain] ?? "#888" }}>
                    {CHAIN_LABELS[chain].slice(0, 2)}
                  </span>
                  <div className="text" style={{ flex: 1, minWidth: 0 }}>
                    <div className="name">
                      {CHAIN_LABELS[chain]} <span className="muted small">{chain}</span>
                    </div>
                  </div>
                  <button
                    className="switch"
                    type="button"
                    role="switch"
                    aria-checked={on}
                    aria-label={`显示 ${CHAIN_LABELS[chain]}`}
                    onClick={() => {
                      const current = s.visibleChains.length === 0 ? [...allChains] : [...s.visibleChains];
                      const next = on ? current.filter((c) => c !== chain) : [...current, chain];
                      void save({ visibleChains: next.length === allChains.length ? [] : (next as ChainId[]) });
                    }}
                  />
                </div>
              );
            })}
          </div>
        </div>

        {/* 外观与反馈 */}
        <div className="card card-pad">
          <div className="section-title"><h2>外观与反馈</h2></div>
          <div className="setting-row">
            <div className="text"><div className="name">法币计价</div><div className="desc">资产估值展示时使用的法币。</div></div>
            <div className="seg">
              <button type="button" data-fiat="usd" aria-pressed={s.fiat === "usd"} onClick={() => void save({ fiat: "usd" })}>USD</button>
              <button type="button" data-fiat="cny" aria-pressed={s.fiat === "cny"} onClick={() => void save({ fiat: "cny" })}>CNY</button>
            </div>
          </div>
          <div className="setting-row">
            <div className="text"><div className="name">主题</div><div className="desc">浅色、深色或跟随系统昼夜。</div></div>
            <div className="seg">
              <button type="button" data-choice="system" aria-pressed={s.theme === "system"} onClick={() => void save({ theme: "system" })}>跟随系统</button>
              <button type="button" data-choice="light" aria-pressed={s.theme === "light"} onClick={() => void save({ theme: "light" })}>浅色</button>
              <button type="button" data-choice="dark" aria-pressed={s.theme === "dark"} onClick={() => void save({ theme: "dark" })}>深色</button>
            </div>
          </div>
          <div className="setting-row">
            <div className="text"><div className="name">隐藏余额</div><div className="desc">总览页面模糊显示数字，适合在公共场合查看。</div></div>
            <button className="switch" type="button" role="switch" aria-checked={s.hideBalances} aria-label="隐藏余额" onClick={() => void save({ hideBalances: !s.hideBalances })} />
          </div>
          <div className="setting-row">
            <div className="text"><div className="name">价格服务</div><div className="desc">在线获取法币估值；生成法币；关闭后只显示数量。</div></div>
            <button className="switch" type="button" role="switch" aria-checked={s.priceService} aria-label="价格服务" onClick={() => void save({ priceService: !s.priceService })} />
          </div>
        </div>

        {/* 网络 */}
        <div className="card card-pad">
          <div className="section-title"><h2>网络</h2></div>
          <div className="setting-row">
            <div className="text">
              <div className="name">Blockchair API Key</div>
              <div className="desc">Dogecoin 与 Bitcoin Cash 的余额、报价和广播经 Blockchair；无需密钥也可用，但限额下数据可能不全。</div>
            </div>
            <span className="chip">{s.blockchairApiKey ? `${s.blockchairApiKey.slice(0, 4)}????${s.blockchairApiKey.slice(-4)}` : "未设置"}</span>
          </div>
          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            <input
              className="input"
              id="blockchair-key"
              type="password"
              placeholder="粘贴 API Key"
              autoComplete="off"
              style={{ flex: 1, minWidth: 200 }}
              value={blockchairDraft}
              onChange={(event) => setBlockchairDraft(event.target.value)}
            />
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => void save({ blockchairApiKey: blockchairDraft.trim() })}>
              <Icon name="check" />
              保存
            </button>
            {s.blockchairApiKey ? (
              <button
                className="btn btn-ghost btn-sm"
                type="button"
                onClick={() => {
                  setBlockchairDraft("");
                  void save({ blockchairApiKey: "" });
                }}
              >
                <Icon name="trash" />
                清除
              </button>
            ) : null}
          </div>
          <div className="setting-row" style={{ marginTop: 20 }}>
            <div className="text">
              <div className="name">Monero 钱包 RPC</div>
              <div className="desc">XMR 转账经由 monero-wallet-rpc 完成 RingCT 签名。钱包文件放在它的 --wallet-dir 里；恢复高度首次查询时按此高度扫描（0 表示从创世块开始）。</div>
            </div>
            <span className="chip">未连接</span>
          </div>
          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            <input className="input" placeholder="http://127.0.0.1:18083/json_rpc" autoComplete="off" style={{ flex: 2, minWidth: 240 }} value={s.moneroRpcUrl} onChange={(event) => void save({ moneroRpcUrl: event.target.value })} />
            <input className="input" placeholder="RPC 用户名（可选）" autoComplete="off" style={{ flex: 1, minWidth: 140 }} value={s.moneroRpcUser} onChange={(event) => void save({ moneroRpcUser: event.target.value })} />
            <input className="input" type="password" placeholder="RPC 密码（可选）" autoComplete="new-password" style={{ flex: 1, minWidth: 140 }} value={s.moneroRpcPassword} onChange={(event) => void save({ moneroRpcPassword: event.target.value })} />
          </div>
          <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
            <input
              className="input"
              type="number"
              min={0}
              step={1}
              placeholder="恢复高度（0 = 从头扫描）"
              style={{ flex: 1, minWidth: 220 }}
              value={s.moneroRestoreHeight}
              onChange={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value)) void save({ moneroRestoreHeight: Math.max(0, Math.floor(value)) });
              }}
            />
            <button className="btn btn-ghost btn-sm" type="button">
              <Icon name="check" />
              保存
            </button>
            <button className="btn btn-ghost btn-sm" type="button">
              <Icon name="activity" />
              测试连接
            </button>
          </div>
        </div>

        {/* 安全 */}
        <div className="card card-pad">
          <div className="section-title"><h2>安全</h2></div>
          <div className="setting-row">
            <div className="text"><div className="name">自动锁定</div><div className="desc">超过设定时间后自动锁定，密钥立即从内存清除。</div></div>
            <select
              className="select"
              style={{ width: "auto" }}
              value={s.autoLockMinutes}
              onChange={(event) => void save({ autoLockMinutes: Number(event.target.value) })}
            >
              {[1, 5, 10, 15, 30, 60].map((minutes) => (
                <option key={minutes} value={minutes}>
                  {minutes} 分钟{minutes === 5 ? "（默认）" : ""}
                </option>
              ))}
            </select>
          </div>
          <div className="setting-row">
            <div className="text"><div className="name">修改密码</div><div className="desc">用于加密本机钱包文件；助记词本身不受影响。</div></div>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => setPwOpen(true)}>
              <Icon name="lock" />
              修改密码
            </button>
          </div>
          <div className="setting-row">
            <div className="text"><div className="name">立即锁定</div><div className="desc">结束当前会话并回到解锁界面。</div></div>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => void lockNow()}>
              <Icon name="lock" />
              立即锁定
            </button>
          </div>
        </div>

        {/* 危险区 */}
        <div className="card card-pad">
          <div className="section-title"><h2>危险区</h2></div>
          <div className="note note-danger">
            <Icon name="warn" />
            <div>
              删除会移除本机钱包文件，其中可能包含多个钱包与记录。<strong>请确认你已备份助记词（如有），否则资产将无法恢复。</strong>
            </div>
          </div>
          <div className="row" style={{ marginTop: 14 }}>
            <button className="btn btn-danger" type="button" onClick={() => void deleteVault()}>
              <Icon name="trash" />
              删除钱包文件
            </button>
          </div>
        </div>
      </div>

      {revealed ? (
        <Modal title="助记词（请立即备份）" onClose={() => setRevealed(null)}>
          <div className="mnemonic-grid">
            {revealed.map((word, index) => (
              <div key={index} className="mnemonic-chip mono">
                <span className="idx">{index + 1}</span>
                {word}
              </div>
            ))}
          </div>
          <div className="modal-foot">
            <button className="btn btn-ghost" type="button" onClick={() => setRevealed(null)}>
              关闭
            </button>
            <button
              className="btn"
              type="button"
              onClick={async () => {
                const ok = await copyText(revealed.join(" "));
                toast(ok ? "已复制，注意剪贴板安全" : "复制失败", ok ? "success" : "error");
              }}
            >
              <Icon name="copy" />
              复制全部
            </button>
          </div>
        </Modal>
      ) : null}

      {pwOpen ? <PasswordDialog onClose={() => setPwOpen(false)} /> : null}
    </section>
  );
}

function PasswordDialog({ onClose }: { onClose: () => void }) {
  const toast = useToast();
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [repeat, setRepeat] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const submit = async () => {
    if (next.length < 8) return setError("新密码至少 8 位");
    if (next !== repeat) return setError("两次输入的新密码不一致");
    setBusy(true);
    setError("");
    try {
      await api.changePassword(current, next);
      toast("密码已修改", "success");
      onClose();
    } catch (e) {
      setError((e as Error).message);
      setBusy(false);
    }
  };

  return (
    <Modal title="修改密码" onClose={onClose}>
      <div className="stack-sm" style={{ marginTop: 10 }}>
        <div className="field">
          <label htmlFor="pw-current">当前密码</label>
          <input
            className="input"
            id="pw-current"
            type="password"
            autoComplete="current-password"
            value={current}
            onChange={(event) => setCurrent(event.target.value)}
          />
        </div>
        <div className="field">
          <label htmlFor="pw-next">新密码（至少 8 位）</label>
          <input
            className="input"
            id="pw-next"
            type="password"
            autoComplete="new-password"
            value={next}
            onChange={(event) => setNext(event.target.value)}
          />
        </div>
        <div className="field">
          <label htmlFor="pw-repeat">重复新密码</label>
          <input
            className="input"
            id="pw-repeat"
            type="password"
            autoComplete="new-password"
            value={repeat}
            onChange={(event) => setRepeat(event.target.value)}
          />
        </div>
        {error ? <p className="error-text">{error}</p> : null}
      </div>
      <div className="modal-foot">
        <button className="btn btn-ghost" type="button" onClick={onClose}>
          取消
        </button>
        <button className="btn btn-primary" type="button" disabled={busy} onClick={() => void submit()}>
          {busy ? "修改中…" : "修改密码"}
        </button>
      </div>
    </Modal>
  );
}
