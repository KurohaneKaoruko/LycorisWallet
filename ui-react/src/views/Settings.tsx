/** Settings: display (currency/theme/hide), security (password, auto-lock,
 *  chain toggles), Monero RPC, price service and dangerous actions.
 *  Ported from ui/js/views/settings.js (first pass — full parity). */

import { useState } from "react";

import { api, walletFull } from "../api";
import { useSession } from "../shell/session";
import { Modal, useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";
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

export function SettingsView() {
  const { wallet, setWallet, refreshStatus } = useSession();
  const toast = useToast();
  const { confirm, askPassword } = useDialogs();
  const [revealed, setRevealed] = useState<string[] | null>(null);
  const [pwOpen, setPwOpen] = useState(false);

  if (!wallet) return null;
  const s = wallet.settings;
  const allChains = Object.keys(CHAIN_LABELS) as ChainId[];

  const save = async (patch: Partial<SettingsModel>) => {
    try {
      // 链开关需要全量视图;其余设置在过滤后的视图上保存即可。
      const full = patch.visibleChains !== undefined ? await walletFull() : wallet;
      const view = await api.saveSettings({ ...full.settings, ...patch });
      setWallet(view);
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

  return (
    <section className="view">
      <div className="page-head">
        <div>
          <h1>设置</h1>
          <div className="sub">显示、安全与本机服务。所有设置保存在加密的钱包文件里。</div>
        </div>
      </div>

      <div className="card card-pad stack">
        <div className="section-title">
          <h3>显示</h3>
        </div>
        <div className="row" style={{ gap: 12, flexWrap: "wrap" }}>
          <div className="field" style={{ flex: 1, minWidth: 160 }}>
            <label htmlFor="settings-fiat">法币</label>
            <select
              className="select"
              id="settings-fiat"
              value={s.fiat}
              onChange={(event) => void save({ fiat: event.target.value as SettingsModel["fiat"] })}
            >
              <option value="usd">USD ($)</option>
              <option value="cny">CNY (¥)</option>
            </select>
          </div>
          <div className="field" style={{ flex: 1, minWidth: 160 }}>
            <label htmlFor="settings-theme">主题</label>
            <select
              className="select"
              id="settings-theme"
              value={s.theme}
              onChange={(event) => void save({ theme: event.target.value as SettingsModel["theme"] })}
            >
              <option value="system">跟随系统</option>
              <option value="light">浅色</option>
              <option value="dark">深色</option>
            </select>
          </div>
          <div className="field" style={{ flex: 1, minWidth: 160 }}>
            <label htmlFor="settings-lock">自动锁定（分钟）</label>
            <input
              className="input"
              id="settings-lock"
              type="number"
              min={1}
              max={120}
              value={s.autoLockMinutes}
              onChange={(event) => {
                const value = Number(event.target.value);
                if (Number.isFinite(value)) void save({ autoLockMinutes: Math.max(1, Math.min(120, Math.floor(value))) });
              }}
            />
          </div>
        </div>
        <label className="check">
          <input
            type="checkbox"
            checked={s.hideBalances}
            onChange={(event) => void save({ hideBalances: event.target.checked })}
          />
          隐藏余额（截图友好）
        </label>
        <label className="check">
          <input
            type="checkbox"
            checked={s.priceService}
            onChange={(event) => void save({ priceService: event.target.checked })}
          />
          启用行情服务（CoinGecko）
        </label>

        <div className="section-title" style={{ marginTop: 10 }}>
          <h3>显示的链</h3>
          <span className="hint">只影响 UI 展示；派生与密钥始终覆盖全部链</span>
        </div>
        <div className="chain-toggles">
          {allChains.map((chain) => {
            const visible = s.visibleChains.length === 0 || s.visibleChains.includes(chain);
            return (
              <label className="check" key={chain}>
                <input
                  type="checkbox"
                  checked={visible}
                  onChange={(event) => {
                    const current = s.visibleChains.length === 0 ? [...allChains] : [...s.visibleChains];
                    const next = event.target.checked
                      ? allChains.filter((c) => current.includes(c) || c === chain)
                      : current.filter((c) => c !== chain);
                    void save({
                      visibleChains:
                        next.length === allChains.length ? [] : (next as ChainId[]),
                    });
                  }}
                />
                {CHAIN_LABELS[chain]}
              </label>
            );
          })}
        </div>

        <div className="section-title" style={{ marginTop: 10 }}>
          <h3>安全</h3>
        </div>
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          <button className="btn" type="button" onClick={() => setPwOpen(true)}>
            <Icon name="key" />
            修改密码
          </button>
          {wallet.mode === "mnemonic" ? (
            <button className="btn" type="button" onClick={() => void revealWords()}>
              <Icon name="eye" />
              导出助记词
            </button>
          ) : null}
          <button
            className="btn"
            type="button"
            onClick={async () => {
              const password = await askPassword({
                title: "导出基础私钥",
                description: "请选择币种对应的私钥格式。验证密码后显示。",
                confirmText: "下一步",
              });
              if (!password) return;
              toast("请到资产页的具体地址查看与导出私钥", "info");
            }}
          >
            <Icon name="key" />
            导出基础私钥
          </button>
        </div>

        <div className="section-title" style={{ marginTop: 10 }}>
          <h3>Monero</h3>
          <span className="hint">转账需要本机 monero-wallet-rpc；余额查询不依赖它</span>
        </div>
        <div className="field">
          <label htmlFor="settings-xmr-rpc">RPC 地址</label>
          <input
            className="input mono"
            id="settings-xmr-rpc"
            value={s.moneroRpcUrl}
            onChange={(event) => void save({ moneroRpcUrl: event.target.value })}
          />
        </div>
        <div className="row" style={{ gap: 12 }}>
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="settings-xmr-user">RPC 用户名（可选）</label>
            <input
              className="input"
              id="settings-xmr-user"
              value={s.moneroRpcUser}
              onChange={(event) => void save({ moneroRpcUser: event.target.value })}
            />
          </div>
          <div className="field" style={{ flex: 1 }}>
            <label htmlFor="settings-xmr-pass">RPC 密码（可选）</label>
            <input
              className="input"
              id="settings-xmr-pass"
              type="password"
              value={s.moneroRpcPassword}
              onChange={(event) => void save({ moneroRpcPassword: event.target.value })}
            />
          </div>
        </div>
        <div className="field">
          <label htmlFor="settings-xmr-height">扫描起始高度（0 = 全链扫描）</label>
          <input
            className="input"
            id="settings-xmr-height"
            type="number"
            min={0}
            value={s.moneroRestoreHeight}
            onChange={(event) => {
              const value = Number(event.target.value);
              if (Number.isFinite(value)) void save({ moneroRestoreHeight: Math.max(0, Math.floor(value)) });
            }}
          />
        </div>
        {moneroAccount ? (
          <button
            className="btn btn-ghost btn-sm"
            type="button"
            onClick={async () => {
              const ok = await copyText(moneroAccount.address);
              toast(ok ? "XMR 地址已复制" : "复制失败", ok ? "success" : "error");
            }}
          >
            <Icon name="copy" />
            复制 XMR 地址
          </button>
        ) : null}

        <div className="section-title" style={{ marginTop: 10 }}>
          <h3>行情数据</h3>
        </div>
        <div className="field">
          <label htmlFor="settings-blockchair">Blockchair API Key（DOGE/BCH 需要）</label>
          <input
            className="input mono"
            id="settings-blockchair"
            value={s.blockchairApiKey}
            onChange={(event) => void save({ blockchairApiKey: event.target.value })}
          />
        </div>

        <div className="section-title" style={{ marginTop: 10 }}>
          <h3>危险区</h3>
        </div>
        <div className="row" style={{ gap: 8, flexWrap: "wrap" }}>
          <button
            className="btn btn-danger"
            type="button"
            onClick={async () => {
              const password = await askPassword({
                title: "删除钱包文件",
                description: "整个加密文件将被删除，包括所有钱包、记录与设置。助记词备份是唯一恢复手段。",
                confirmText: "删除",
              });
              if (!password) return;
              const ok = await confirm({
                title: "最后确认",
                body: "<p>真的要删除整个钱包文件吗？此操作无法撤销。</p>",
                confirmText: "永久删除",
                danger: true,
              });
              if (!ok) return;
              try {
                await api.deleteVault(password);
                await refreshStatus();
                toast("钱包文件已删除", "success");
              } catch (e) {
                toast((e as Error).message, "error");
              }
            }}
          >
            <Icon name="trash" />
            删除钱包文件
          </button>
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
