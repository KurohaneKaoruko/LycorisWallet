/** Boot + lock + onboarding: the pre-unlock flow, ported from
 *  ui/js/views/unlock.js and ui/js/views/onboarding.js. The full onboarding
 *  wizard (create/import/passphrase) is the largest single view; this first
 *  pass covers create-with-generated-mnemonic and unlock, keeping the DOM
 *  shape compatible with the existing stylesheet. */

import { useEffect, useState } from "react";

import { api, isMock } from "../api";
import { useSession } from "./session";
import { useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";

export function UnlockView() {
  const { status, setWallet, refreshStatus, go } = useSession();
  const toast = useToast();
  const { confirm } = useDialogs();
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const info = status;

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  const submit = async () => {
    if (!password || busy) return;
    setBusy(true);
    setError("");
    try {
      const wallet = await api.unlockVault(password);
      setWallet(wallet);
      // 解锁成功：进入总览（否则 Router 的 unlock 分支会继续拦住）。
      go("dashboard");
      toast("已解锁", "success");
    } catch (e) {
      const err = e as { code?: string; message: string };
      setError(err.message || "解锁失败");
      if (err.code === "LOCKED_COOLDOWN") void refreshStatus();
    } finally {
      setBusy(false);
    }
  };

  const forgot = async () => {
    const ok = await confirm({
      title: "删除本机钱包文件？",
      body:
        "钱包文件删除后，助记词仍然可以重新导入，但本机的交易记录、联系人和设置会全部丢失。" +
        (isMock ? "（演示模式：会清空演示数据）" : "此操作无法撤销。"),
      confirmText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await api.deleteVault(password || "__confirm__");
      toast("钱包文件已删除", "success");
      await refreshStatus();
    } catch (e) {
      // Wrong password path: ask again via the confirm dialog flow.
      toast((e as Error).message || "删除失败", "error");
    }
  };

  return (
    <section className="onboarding">
      <div className="onboarding-inner" style={{ maxWidth: 440 }}>
        <div className="brand-head">
          <img className="mark" src="/favicon-flower.svg" alt="" draggable={false} />
          <h1>钱包已锁定</h1>
          <p>{info?.label ? info.label : "输入本地密码解锁"}</p>
        </div>

        <div className="card card-pad stack">
          <div className="field">
            <label htmlFor="unlock-password">密码</label>
            <input
              className="input"
              id="unlock-password"
              type="password"
              autoComplete="current-password"
              placeholder="输入密码"
              value={password}
              onChange={(event) => {
                setPassword(event.target.value);
                setError("");
              }}
              onKeyDown={(event) => {
                if (event.key === "Enter") void submit();
              }}
              autoFocus
            />
          </div>
          {info?.hasHint ? (
            <div className="note">
              <Icon name="info" />
              <div>
                密码提示：<strong>{info.hint ?? ""}</strong>
              </div>
            </div>
          ) : null}
          {error ? <p className="error-text">{error}</p> : null}
          <button className="btn btn-primary btn-block" type="button" disabled={busy} onClick={() => void submit()}>
            {busy ? "解锁中…" : "解锁"}
          </button>
          <p className="hint">
            密码只用于解密本机钱包文件，忘记密码无法找回，只能删除钱包后重新导入助记词或重建口令钱包。
          </p>
          <button className="btn btn-ghost btn-block" type="button" onClick={() => void forgot()}>
            忘记密码？删除本机钱包文件
          </button>
        </div>
      </div>
    </section>
  );
}

/** Onboarding wizard. First pass: create vault with a generated mnemonic and
 *  optional passphrase wallets; import flow lands with the settings rewrite. */
export function OnboardingView() {
  const { setWallet } = useSession();
  const toast = useToast();
  const [step, setStep] = useState<"intro" | "words">("intro");
  const [words, setWords] = useState<string[]>([]);
  const [password, setPassword] = useState("");
  const [hint, setHint] = useState("");
  const [label, setLabel] = useState("");
  const [busy, setBusy] = useState(false);

  const start = async () => {
    setBusy(true);
    try {
      const mnemonic = await api.generateMnemonic(12);
      setWords(mnemonic.trim().split(/\s+/));
      setStep("words");
    } catch (e) {
      toast((e as Error).message, "error");
    } finally {
      setBusy(false);
    }
  };

  const create = async () => {
    if (password.length < 8) {
      toast("密码至少 8 位", "error");
      return;
    }
    setBusy(true);
    try {
      const wallet = await api.createVault({
        password,
        hint: hint || null,
        label: label || null,
        secret: { mode: "mnemonic", mnemonic: words.join(" ") },
      });
      setWallet(wallet);
      toast("钱包已创建", "success");
    } catch (e) {
      toast((e as Error).message, "error");
    } finally {
      setBusy(false);
    }
  };

  if (step === "intro") {
    return (
      <section className="onboarding">
        <div className="onboarding-inner" style={{ maxWidth: 520 }}>
          <div className="brand-head">
            <img className="mark" src="/favicon-flower.svg" alt="" draggable={false} />
            <h1>Lycoris Wallet</h1>
            <p>本地优先的多链钱包 · 密钥从不离开本机</p>
          </div>
          <div className="card card-pad stack">
            <div className="note">
              <Icon name="shield" />
              <div>钱包以加密文件保存在本机；密码丢失无法找回，助记词是唯一的恢复手段。</div>
            </div>
            <button className="btn btn-primary btn-block" type="button" disabled={busy} onClick={() => void start()}>
              {busy ? "生成中…" : "创建新钱包（生成助记词）"}
            </button>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="onboarding">
      <div className="onboarding-inner" style={{ maxWidth: 560 }}>
        <div className="brand-head">
          <img className="mark" src="/favicon-flower.svg" alt="" draggable={false} />
          <h1>备份助记词</h1>
          <p>按顺序抄写在纸上，任何人拿到这 12 个词就能拿走全部资产</p>
        </div>
        <div className="card card-pad stack">
          <div className="mnemonic-grid">
            {words.map((word, index) => (
              <div key={index} className="mnemonic-chip mono">
                <span className="idx">{index + 1}</span>
                {word}
              </div>
            ))}
          </div>
          <button
            className="btn btn-ghost"
            type="button"
            onClick={async () => {
              const ok = await copyText(words.join(" "));
              toast(ok ? "已复制助记词" : "复制失败", ok ? "success" : "error");
            }}
          >
            <Icon name="copy" /> 复制助记词
          </button>
          <div className="field">
            <label htmlFor="setup-password">设置密码（至少 8 位）</label>
            <input
              className="input"
              id="setup-password"
              type="password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              placeholder="解锁钱包用"
            />
          </div>
          <div className="row" style={{ gap: 12 }}>
            <div className="field" style={{ flex: 1 }}>
              <label htmlFor="setup-label">钱包名称（可选）</label>
              <input
                className="input"
                id="setup-label"
                type="text"
                value={label}
                onChange={(event) => setLabel(event.target.value)}
                placeholder="默认「钱包 1」"
              />
            </div>
            <div className="field" style={{ flex: 1 }}>
              <label htmlFor="setup-hint">密码提示（可选）</label>
              <input
                className="input"
                id="setup-hint"
                type="text"
                value={hint}
                onChange={(event) => setHint(event.target.value)}
                placeholder="显示在锁屏上"
              />
            </div>
          </div>
          <button className="btn btn-primary btn-block" type="button" disabled={busy} onClick={() => void create()}>
            {busy ? "创建中…" : "我已备份，创建钱包"}
          </button>
        </div>
      </div>
    </section>
  );
}
