/** Boot + lock + onboarding: the pre-unlock flow, ported from
 *  ui/js/views/unlock.js and ui/js/views/onboarding.js. The full wizard covers
 *  mode selection (mnemonic / passphrase), verification, import and the
 *  password step, keeping the DOM shape compatible with the stylesheet. */

import { Fragment, useEffect, useMemo, useState } from "react";

import { api, isMock } from "../api";
import { copyText } from "../clipboard";
import { useSession } from "./session";
import { useDialogs, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import type { ChainId, DerivationPreview, WalletView } from "../types";

const STEPS = ["选择模式", "生成密钥", "设置密码"];

const BRAND_SUB = "本地优先的多链钱包 · 密钥与签名永远留在你的设备上";

function StepBar({ step }: { step: number }) {
  return (
    <div className="steps">
      {STEPS.map((label, index) => (
        <Fragment key={label}>
          <div className="step" data-state={index < step ? "done" : index === step ? "active" : "todo"}>
            <span className="idx">{index < step ? "✓" : index + 1}</span>
            <span>{label}</span>
          </div>
          {index < STEPS.length - 1 ? <span className="sep" /> : null}
        </Fragment>
      ))}
    </div>
  );
}

function BrandHead({ title, sub }: { title: string; sub: string }) {
  return (
    <div className="brand-head">
      <img className="mark" src="/favicon-flower.svg" alt="" draggable={false} />
      <h1>{title}</h1>
      <p>{sub}</p>
    </div>
  );
}

/** 实时派生预览块（口令/导入共用）：骨架 → 错误 → 指纹 + 每链地址。 */
function PreviewBlock({
  preview,
  previewing,
  error,
}: {
  preview: DerivationPreview | null;
  previewing: boolean;
  error: string;
}) {
  if (!preview && !previewing && !error) return null;
  if (previewing) {
    return (
      <div className="stack-sm">
        <div className="skeleton" style={{ height: 64 }} />
        <div className="skeleton" style={{ height: 64 }} />
      </div>
    );
  }
  if (error) {
    return (
      <div className="note note-warn">
        <Icon name="warn" />
        <div>{error}</div>
      </div>
    );
  }
  if (!preview) return null;
  return (
    <div className="stack-sm">
      <div className="row-between">
        <span className="field-label">实时派生预览</span>
        <span className="fingerprint muted">{preview.fingerprint}</span>
      </div>
      <div className="preview-grid">
        {preview.accounts.map((account) => (
          <div key={account.chain} className="preview-account">
            <div className="head">
              <span className="dot" style={{ ["--accent" as string]: account.accent }} />
              {account.name}
              <span className="muted">· {account.symbol}</span>
            </div>
            <div className="addr">{account.address}</div>
            <div className="algo">
              {account.hashAlgorithm} · {account.addressFormat}
            </div>
          </div>
        ))}
      </div>
      <p className="hint">{preview.formula}</p>
    </div>
  );
}

/** 密码强度计（与旧版 strengthOf 同阈值）。 */
function strengthOf(password: string): { level: number; label: string; hint: string } {
  if (!password) return { level: 0, label: "空", hint: "至少 8 个字符" };
  let score = 0;
  if (password.length >= 8) score += 1;
  if (password.length >= 12) score += 1;
  if (password.length >= 16) score += 1;
  if (/[a-z]/.test(password) && /[A-Z]/.test(password)) score += 1;
  if (/\d/.test(password)) score += 1;
  if (/[^\w\s]/.test(password)) score += 1;
  if (score <= 2) return { level: 1, label: "偏弱", hint: "建议加长到 12 位以上并混合大小写与符号" };
  if (score <= 4) return { level: 2, label: "中等", hint: "再长一些会更安全" };
  return { level: 3, label: "较强", hint: "很好，请妥善保管" };
}

type Mode = "mnemonic" | "mnemonic-import" | "passphrase" | "private-key" | "aggregate";

/** 引导向导：选择模式 → 生成/导入/口令 → 设置密码。移植自 ui/js/views/onboarding.js。 */
export function OnboardingView() {
  const { setWallet, go } = useSession();
  const toast = useToast();

  const [step, setStep] = useState(0);
  const [mode, setMode] = useState<Mode | null>(null);
  const [mnemonic, setMnemonic] = useState("");
  const [wordCount, setWordCount] = useState(12);
  const [revealed, setRevealed] = useState(false);
  const [verify, setVerify] = useState<number[] | null>(null);
  const [verifyAnswers, setVerifyAnswers] = useState<Record<number, string>>({});
  const [verifyError, setVerifyError] = useState("");
  const [importText, setImportText] = useState("");
  const [importPreview, setImportPreview] = useState<DerivationPreview | null>(null);
  const [importPreviewing, setImportPreviewing] = useState(false);
  const [importError, setImportError] = useState("");
  const [phrase, setPhrase] = useState("");
  const [derivation, setDerivation] = useState<"isolated" | "strict">("isolated");
  const [phrasePreview, setPhrasePreview] = useState<DerivationPreview | null>(null);
  const [phrasePreviewing, setPhrasePreviewing] = useState(false);
  const [phraseError, setPhraseError] = useState("");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [hint, setHint] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const words = useMemo(() => (mnemonic ? mnemonic.split(" ") : []), [mnemonic]);

  const regenerate = async (count = wordCount) => {
    try {
      const next = await api.generateMnemonic(count);
      setMnemonic(next);
      setVerify(null);
      setVerifyAnswers({});
      setVerifyError("");
      setRevealed(false);
    } catch (e) {
      toast((e as Error).message, "error");
    }
  };

  /* 口令预览：输入防抖 380ms 后自动询价，与旧版一致。 */
  useEffect(() => {
    if (step !== 1 || mode !== "passphrase") return;
    if (!phrase.trim()) {
      setPhrasePreview(null);
      setPhraseError("");
      setPhrasePreviewing(false);
      return;
    }
    setPhrasePreviewing(true);
    const timer = window.setTimeout(async () => {
      try {
        setPhrasePreview(await api.previewDerivation({ mode: "passphrase", phrase, derivation }));
        setPhraseError("");
      } catch (e) {
        setPhrasePreview(null);
        setPhraseError((e as Error).message);
      } finally {
        setPhrasePreviewing(false);
      }
    }, 380);
    return () => window.clearTimeout(timer);
  }, [phrase, derivation, step, mode]);

  /* 导入预览：同样防抖。 */
  useEffect(() => {
    if (step !== 1 || mode !== "mnemonic-import") return;
    const list = importText.trim().split(/\s+/).filter(Boolean);
    if (list.length === 0) {
      setImportPreview(null);
      setImportError("");
      setImportPreviewing(false);
      return;
    }
    setImportPreviewing(true);
    const timer = window.setTimeout(async () => {
      try {
        setImportPreview(await api.previewDerivation({ mode: "mnemonic", mnemonic: importText.trim() }));
        setImportError("");
      } catch (e) {
        setImportPreview(null);
        setImportError((e as Error).message);
      } finally {
        setImportPreviewing(false);
      }
    }, 380);
    return () => window.clearTimeout(timer);
  }, [importText, step, mode]);

  /* 私钥预览：粘贴后防抖验证（解析失败立刻提示）。 */
  const [keyPreview, setKeyPreview] = useState<DerivationPreview | null>(null);
  useEffect(() => {
    if (step !== 1 || mode !== "private-key") return;
    if (!importText.trim()) {
      setKeyPreview(null);
      setImportError("");
      setImportPreviewing(false);
      return;
    }
    setImportPreviewing(true);
    const timer = window.setTimeout(async () => {
      try {
        setKeyPreview(await api.previewDerivation({ mode: "private_key", key: importText.trim() }));
        setImportError("");
      } catch (e) {
        setKeyPreview(null);
        setImportError((e as Error).message);
      } finally {
        setImportPreviewing(false);
      }
    }, 380);
    return () => window.clearTimeout(timer);
  }, [importText, step, mode]);

  /* 聚合钱包第一把钥匙预览:链 + 原始钥匙防抖验证。 */
  const [aggChain, setAggChain] = useState<ChainId>("bitcoin");
  const [aggPreview, setAggPreview] = useState<DerivationPreview | null>(null);
  const [aggError, setAggError] = useState("");
  useEffect(() => {
    if (step !== 1 || mode !== "aggregate") return;
    if (!importText.trim()) {
      setAggPreview(null);
      setAggError("");
      setImportPreviewing(false);
      return;
    }
    setImportPreviewing(true);
    const timer = window.setTimeout(async () => {
      try {
        setAggPreview(
          await api.previewDerivation({ mode: "aggregate", chain: aggChain, key: importText.trim() }),
        );
        setAggError("");
      } catch (e) {
        setAggPreview(null);
        setAggError((e as Error).message);
      } finally {
        setImportPreviewing(false);
      }
    }, 380);
    return () => window.clearTimeout(timer);
  }, [importText, step, mode, aggChain]);

  const makeVerify = () => {
    const positions = new Set<number>();
    const total = words.length;
    while (positions.size < Math.min(3, total)) {
      positions.add(1 + Math.floor(Math.random() * total));
    }
    setVerify([...positions].sort((a, b) => a - b));
    setVerifyError("");
    setVerifyAnswers({});
  };

  const checkVerify = () => {
    if (!verify) return;
    const wrong = verify.filter((position) => (verifyAnswers[position] ?? "") !== words[position - 1]);
    if (wrong.length > 0) {
      setVerifyError(`第 ${wrong.join("、")} 个单词不正确，请对照抄写内容`);
      return;
    }
    setVerifyError("");
    setStep(2);
  };

  const create = async () => {
    if (busy) return;
    setBusy(true);
    setError("");
    try {
      const secret =
        mode === "passphrase"
          ? { mode: "passphrase" as const, phrase, derivation }
          : mode === "private-key"
            ? { mode: "private_key" as const, key: importText.trim() }
            : mode === "aggregate"
              ? {
                  mode: "aggregate" as const,
                  chain: aggChain,
                  key: importText.trim(),
                  label: null,
                }
              : mode === "mnemonic-import"
                ? { mode: "mnemonic" as const, mnemonic: importText.trim() }
                : { mode: "mnemonic" as const, mnemonic };
      const wallet = await api.createVault({
        password,
        hint: hint.trim() || null,
        label: null,
        secret,
      });
      setWallet(wallet);
      toast("钱包已创建并加密保存", "success");
      go("dashboard");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  /* ------------------------------------------------------------ step 0: 模式 */
  if (step === 0) {
    return (
      <section className="onboarding">
        <div className="onboarding-inner">
          <BrandHead title="Lycoris Wallet" sub={BRAND_SUB} />
          <StepBar step={0} />
          <div className="mode-cards">
            <button
              className="mode-card"
              type="button"
              onClick={() => {
                setMode("mnemonic");
                setStep(1);
                void regenerate();
              }}
            >
              <span className="tag">BIP-39 · BIP-32 · SLIP-0010</span>
              <h3>助记词钱包</h3>
              <p>
                用 12 或 24 个单词的标准助记词管理全部链。地址按各链通用的派生路径生成，可以与主流钱包互相导入导出。
              </p>
              <div className="foot">适合长期使用 · 助记词可以抄写在纸上离线保存</div>
            </button>
            <button
              className="mode-card"
              type="button"
              onClick={() => {
                setMode("passphrase");
                setStep(1);
              }}
            >
              <span className="tag">H_chain(phrase) → private key</span>
              <h3>口令钱包</h3>
              <p>
                输入任意一句话，直接用它所在链的原生哈希算法派生私钥：比特币用
                SHA-256、莱特币用 scrypt、EVM 用 Keccak-256、Solana 用 SHA-256。
              </p>
              <div className="foot">一句话管所有链 · 口令一旦忘记无法找回</div>
            </button>
            <button
              className="mode-card"
              type="button"
              onClick={() => {
                setMode("private-key");
                setStep(1);
              }}
            >
              <span className="tag">钥匙环 · secp256k1 + ed25519 + CryptoNote</span>
              <h3>私钥钱包</h3>
              <p>
                导入已有的原始私钥(WIF / hex),按曲线分槽管理:一类钥匙覆盖一条曲线上的所有链,
                随时补加 Solana、Monero 槽补齐全能覆盖;同链多账户可再建一个私钥钱包。
              </p>
              <div className="foot">从其他钱包迁移 · 私钥泄露即全部资产泄露</div>
            </button>
            <button
              className="mode-card"
              type="button"
              onClick={() => {
                setMode("aggregate");
                setStep(1);
              }}
            >
              <span className="tag">一钥一链一址 · 数量不限</span>
              <h3>聚合钱包</h3>
              <p>
                随意添加任意数量的独立私钥:每把钥匙只管自己链上的一个地址,互不相关、
                无任何派生。同一条链想放几把就放几把,像仓库一样集中收发。
              </p>
              <div className="foot">大量零散私钥的统一管家 · 逐把导入逐把管理</div>
            </button>
          </div>
          <div className="note note-warn" style={{ marginTop: 18 }}>
            <Icon name="warn" />
            <div>口令钱包把安全性完全交给这句话：太短或太常见的口令会被暴力猜测。请使用足够长、只有你记得的句子。</div>
          </div>
        </div>
      </section>
    );
  }

  /* ------------------------------------------------------------ step 1: 密钥 */
  if (step === 1 && (mode === "mnemonic" || mode === null)) {
    const verifyReady = words.length > 0;
    return (
      <section className="onboarding">
        <div className="onboarding-inner">
          <BrandHead title="Lycoris Wallet" sub={BRAND_SUB} />
          <StepBar step={1} />
          <div className="card card-pad stack">
            <div className="row-between">
              <div>
                <h2>助记词</h2>
                <p className="hint">按顺序抄写到纸上。任何拿到这些单词的人都可以拿走全部资产。</p>
              </div>
              <div className="seg" role="group" aria-label="助记词长度">
                {[12, 24].map((count) => (
                  <button
                    key={count}
                    type="button"
                    data-words={count}
                    aria-pressed={wordCount === count}
                    onClick={() => {
                      setWordCount(count);
                      void regenerate(count);
                    }}
                  >
                    {count} 词
                  </button>
                ))}
              </div>
            </div>

            <div className={`mnemonic-grid${revealed ? "" : " mnemonic-blur"}`}>
              {words.map((word, index) => (
                <div key={index} className="mnemonic-word">
                  <span className="idx">{index + 1}</span>
                  <span>{word}</span>
                </div>
              ))}
            </div>

            <div className="row" style={{ gap: 8 }}>
              <button className="btn btn-ghost btn-sm" type="button" onClick={() => setRevealed(!revealed)}>
                <Icon name={revealed ? "eyeOff" : "eye"} />
                {revealed ? "隐藏" : "显示"}
              </button>
              <button
                className="btn btn-ghost btn-sm"
                type="button"
                onClick={async () => {
                  const ok = await copyText(mnemonic);
                  toast(ok ? "助记词已复制（记得清理剪贴板）" : "复制失败", ok ? "success" : "error");
                }}
              >
                <Icon name="copy" />
                复制
              </button>
              <button className="btn btn-ghost btn-sm" type="button" onClick={() => void regenerate()}>
                <Icon name="refresh" />
                重新生成
              </button>
            </div>

            <div id="verify-block">
              {verify ? (
                <>
                  <div className="divider" />
                  <div className="stack-sm">
                    <span className="field-label">校验一下：请补全下面几个单词</span>
                    {verify.map((position) => (
                      <div key={position} className="verify-row">
                        <span className="ask">第 {position} 个</span>
                        <input
                          className="input"
                          data-verify={position}
                          placeholder="输入单词"
                          autoComplete="off"
                          spellCheck={false}
                          value={verifyAnswers[position] ?? ""}
                          onChange={(event) => {
                            setVerifyAnswers({
                              ...verifyAnswers,
                              [position]: event.target.value.trim().toLowerCase(),
                            });
                            setVerifyError("");
                          }}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") checkVerify();
                          }}
                        />
                      </div>
                    ))}
                    {verifyError ? <p className="error-text">{verifyError}</p> : null}
                  </div>
                </>
              ) : null}
            </div>

            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" type="button" onClick={() => setMode("mnemonic-import")}>
                改用已有助记词
              </button>
              <button
                className="btn btn-primary"
                type="button"
                disabled={!verifyReady}
                onClick={() => {
                  if (!verify) {
                    makeVerify();
                    return;
                  }
                  checkVerify();
                }}
              >
                {verify ? "下一步" : "核对并继续"}
              </button>
            </div>
          </div>
          <div className="row" style={{ marginTop: 14 }}>
            <button className="btn btn-ghost btn-sm" type="button" onClick={() => setStep(0)}>
              <Icon name="chevronLeft" />
              返回
            </button>
          </div>
        </div>
      </section>
    );
  }

  /* -- 导入助记词 -- */
  if (step === 1 && mode === "mnemonic-import") {
    return (
      <section className="onboarding">
        <div className="onboarding-inner">
          <BrandHead title="Lycoris Wallet" sub={BRAND_SUB} />
          <StepBar step={1} />
          <div className="card card-pad stack">
            <div>
              <h2>导入助记词</h2>
              <p className="hint">支持 BIP-39 12/24 词或 Monero 原生 25 词（自动识别）。请输入完整助记词，单词之间用空格分隔。</p>
            </div>
            <div className="field">
              <label htmlFor="import-words">助记词</label>
              <textarea
                className="textarea"
                id="import-words"
                placeholder="例如 abandon abandon … about"
                spellCheck={false}
                autoComplete="off"
                value={importText}
                onChange={(event) => setImportText(event.target.value)}
              />
            </div>
            <div id="import-preview">
              <PreviewBlock preview={importPreview} previewing={importPreviewing} error={importError} />
            </div>
            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" type="button" onClick={() => setMode("mnemonic")}>
                <Icon name="chevronLeft" />
                返回
              </button>
              <button
                className="btn btn-primary"
                type="button"
                onClick={() => {
                  if (!importPreview) {
                    toast("请先输入有效的助记词", "error");
                    return;
                  }
                  setStep(2);
                }}
              >
                下一步
              </button>
            </div>
          </div>
        </div>
      </section>
    );
  }

  /* -- 口令钱包 -- */
  if (step === 1 && mode === "passphrase") {
    return (
      <section className="onboarding">
        <div className="onboarding-inner">
          <BrandHead title="Lycoris Wallet" sub={BRAND_SUB} />
          <StepBar step={1} />
          <div className="card card-pad stack">
            <div>
              <h2>口令</h2>
              <p className="hint">
                这句话会被每条链的原生哈希算法处理，直接得到该链的私钥。同一个口令在隔离模式下每条链得到不同私钥。
              </p>
            </div>
            <div className="field">
              <label htmlFor="phrase">你的口令</label>
              <textarea
                className="textarea"
                id="phrase"
                placeholder="例如：a quiet garden at dusk, 2019 年的那场雨"
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
              <p className="hint" id="derivation-hint">
                {derivation === "isolated"
                  ? '消息 = "lycoris/v1/<链名>\\0" + 口令，每条链的私钥都不同。'
                  : "直接哈希口令本身。Ethereum / Ethereum Classic / BNB Chain / Polygon 使用同一算法，会得到同一把私钥。"}
              </p>
            </div>

            <div id="passphrase-preview">
              <PreviewBlock preview={phrasePreview} previewing={phrasePreviewing} error={phraseError} />
            </div>

            <div className="row" style={{ justifyContent: "flex-end" }}>
              <button className="btn btn-ghost" type="button" onClick={() => setStep(0)}>
                <Icon name="chevronLeft" />
                返回
              </button>
              <button
                className="btn btn-primary"
                type="button"
                disabled={!phrase.trim()}
                onClick={() => setStep(2)}
              >
                下一步
              </button>
            </div>
          </div>
        </div>
      </section>
    );
  }

  /* -- 私钥钱包 -- */
  if (step === 1 && mode === "private-key") {
    return (
      <section className="onboarding">
        <div className="onboarding-inner">
          <BrandHead title="Lycoris Wallet" sub={BRAND_SUB} />
          <StepBar step={1} />
          <div className="card card-pad stack">
            <div>
              <h2>导入第一把私钥</h2>
              <p className="hint">
                钥匙环按椭圆曲线分三类,互相独立:secp256k1 覆盖 Bitcoin、Ethereum
                等 8 条链,ed25519 覆盖 Solana,CryptoNote 覆盖 Monero。这里先导入任意一把,
                创建后可在 设置 → 钥匙环 随时补加其余的,补齐后即为覆盖全部链的全能钱包。
              </p>
            </div>
            <div className="field">
              <label htmlFor="private-key-input">原始私钥</label>
              <textarea
                className="textarea mono"
                id="private-key-input"
                placeholder="secp256k1:Kx…(WIF)或 64 位 hex"
                autoComplete="off"
                spellCheck={false}
                value={importText}
                onChange={(event) => setImportText(event.target.value)}
              />
              {importError ? (
                <p className="hint" style={{ color: "var(--vermilion)" }}>{importError}</p>
              ) : keyPreview ? (
                <p className="hint">
                  有效 · 指纹 {keyPreview.fingerprint} · 已覆盖 {keyPreview.accounts.length}{" "}
                  条链{keyPreview.accounts.length < 10 ? "(其余可在设置里补加)" : ""}
                </p>
              ) : (
                <p className="hint">输入将自动校验;密钥只保存在本机加密文件里。Solana / Monero 的私钥创建后可在设置里补加。</p>
              )}
            </div>
            <div className="note note-warn">
              <Icon name="warn" />
              <div>导入的私钥等同于明文钥匙:任何拿到它的人都能取走资产,且无法像助记词那样抄成单词备份。请确认来源可信。</div>
            </div>
            <div className="row" style={{ gap: 10 }}>
              <button className="btn btn-ghost" type="button" onClick={() => setStep(0)}>
                返回
              </button>
              <button
                className="btn"
                type="button"
                disabled={!importText.trim() || !keyPreview}
                onClick={() => setStep(2)}
              >
                下一步
              </button>
            </div>
          </div>
        </div>
      </section>
    );
  }

  /* -- 聚合钱包 -- */
  if (step === 1 && mode === "aggregate") {
    const CHAIN_OPTIONS: Array<[ChainId, string, string]> = [
      ["bitcoin", "Bitcoin", "WIF(5/K/L 开头)或 64 位 hex"],
      ["litecoin", "Litecoin", "WIF 或 64 位 hex"],
      ["dogecoin", "Dogecoin", "WIF(D 开头)或 64 位 hex"],
      ["bitcoincash", "Bitcoin Cash", "WIF 或 64 位 hex"],
      ["ethereum", "Ethereum", "64 位 hex"],
      ["ethereum-classic", "Ethereum Classic", "64 位 hex"],
      ["bsc", "BNB Chain", "64 位 hex"],
      ["polygon", "Polygon", "64 位 hex"],
      ["solana", "Solana", "base58 keypair(Phantom 导出)或 64 位 hex"],
      ["monero", "Monero", "64 位 hex spend key"],
    ];
    const active = CHAIN_OPTIONS.find(([id]) => id === aggChain)!;
    return (
      <section className="onboarding">
        <div className="onboarding-inner">
          <BrandHead title="Lycoris Wallet" sub={BRAND_SUB} />
          <StepBar step={1} />
          <div className="card card-pad stack">
            <div>
              <h2>导入第一把私钥</h2>
              <p className="hint">
                聚合钱包从一把钥匙开始:每把钥匙只管自己链上的一个地址,互不相关、无派生。
                创建后可在 设置 → 聚合钥匙 里随意追加,想加多少把都行。
              </p>
            </div>
            <div className="field">
              <label htmlFor="agg-chain">这条钥匙属于哪条链</label>
              <div className="seg seg-flow" role="group">
                {CHAIN_OPTIONS.map(([id, name]) => (
                  <button
                    key={id}
                    type="button"
                    aria-pressed={aggChain === id}
                    onClick={() => {
                      setAggChain(id);
                      setAggPreview(null);
                      setAggError("");
                    }}
                  >
                    {name}
                  </button>
                ))}
              </div>
            </div>
            <div className="field">
              <label htmlFor="private-key-input">原始私钥</label>
              <textarea
                className="textarea mono"
                id="private-key-input"
                placeholder={active[2]}
                autoComplete="off"
                spellCheck={false}
                value={importText}
                onChange={(event) => setImportText(event.target.value)}
              />
              {aggError || importError ? (
                <p className="hint" style={{ color: "var(--vermilion)" }}>{aggError || importError}</p>
              ) : aggPreview ? (
                <p className="hint">有效 · 地址 {aggPreview.accounts[0]?.address.slice(0, 14)}… · 创建后可继续追加更多钥匙</p>
              ) : (
                <p className="hint">输入将自动校验;密钥只保存在本机加密文件里。</p>
              )}
            </div>
            <div className="note note-warn">
              <Icon name="warn" />
              <div>聚合钱包里的每把私钥都是独立的明文钥匙,泄露一把就丢一把的资产。请逐把确认来源可信。</div>
            </div>
            <div className="row" style={{ gap: 10 }}>
              <button className="btn btn-ghost" type="button" onClick={() => setStep(0)}>
                返回
              </button>
              <button
                className="btn"
                type="button"
                disabled={!importText.trim() || !aggPreview}
                onClick={() => setStep(2)}
              >
                下一步
              </button>
            </div>
          </div>
        </div>
      </section>
    );
  }

  /* ------------------------------------------------------------ step 2: 密码 */
  const strength = strengthOf(password);
  const colors = ["var(--hair-strong)", "var(--danger)", "var(--warn)", "var(--ok)"];
  const mismatch = confirmPassword.length > 0 && confirmPassword !== password;
  const ready = password.length >= 8 && password === confirmPassword && !busy;
  return (
    <section className="onboarding">
      <div className="onboarding-inner">
        <BrandHead title="Lycoris Wallet" sub={BRAND_SUB} />
        <StepBar step={2} />
        <div className="card card-pad stack">
          <div>
            <h2>设置本地密码</h2>
            <p className="hint">
              密码只用于加密本机的钱包文件（Argon2id + XChaCha20-Poly1305）。它不会被上传，也无法找回。
            </p>
          </div>

          <div className="field">
            <label htmlFor="password">密码</label>
            <input
              className="input"
              id="password"
              type="password"
              autoComplete="new-password"
              placeholder="至少 8 个字符"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
            />
            <div className="row" style={{ gap: 10, alignItems: "center", marginTop: 8 }}>
              <div className="row" style={{ gap: 4, flex: 1 }}>
                {[1, 2, 3].map((index) => (
                  <span
                    key={index}
                    style={{
                      flex: 1,
                      height: 4,
                      borderRadius: 2,
                      background: strength.level >= index ? colors[strength.level] : "var(--hair)",
                    }}
                  />
                ))}
              </div>
              <span className="hint" style={{ minWidth: 70, textAlign: "right" }}>
                {strength.label}
              </span>
            </div>
            <p className="hint">{strength.hint}</p>
          </div>

          <div className="field">
            <label htmlFor="confirm">确认密码</label>
            <input
              className="input"
              id="confirm"
              type="password"
              autoComplete="new-password"
              placeholder="再次输入"
              value={confirmPassword}
              onChange={(event) => setConfirmPassword(event.target.value)}
              aria-invalid={mismatch || undefined}
            />
            {mismatch ? <p className="error-text">两次输入不一致</p> : null}
          </div>

          <div className="field">
            <label htmlFor="hint">密码提示（可选）</label>
            <input
              className="input"
              id="hint"
              placeholder="例如：生日 + 老家门牌"
              maxLength={80}
              value={hint}
              onChange={(event) => setHint(event.target.value)}
            />
            <p className="hint">提示会以明文保存在钱包文件里，请勿写入真正的密码。</p>
          </div>

          {error ? (
            <div className="note note-danger">
              <Icon name="warn" />
              <div>{error}</div>
            </div>
          ) : null}

          <div className="row" style={{ justifyContent: "flex-end" }}>
            <button className="btn btn-ghost" type="button" onClick={() => setStep(1)}>
              <Icon name="chevronLeft" />
              返回
            </button>
            <button className="btn btn-primary" type="button" disabled={!ready} onClick={() => void create()}>
              创建钱包
            </button>
          </div>
        </div>
      </div>
    </section>
  );
}

/** 锁屏：密码提示、错误次数冷却倒计时、忘记密码走 resetVault。
 *  移植自 ui/js/views/unlock.js。 */
export function UnlockView() {
  const { status, setWallet, refreshStatus, go } = useSession();
  const toast = useToast();
  const { confirm } = useDialogs();
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [countdown, setCountdown] = useState(0);
  const [busy, setBusy] = useState(false);
  const info = status;

  useEffect(() => {
    void refreshStatus();
  }, [refreshStatus]);

  useEffect(() => {
    if (countdown <= 0) return;
    const timer = window.setInterval(() => setCountdown((value) => Math.max(0, value - 1)), 1000);
    return () => window.clearInterval(timer);
  }, [countdown]);

  const submit = async () => {
    if (!password || busy || countdown > 0) return;
    setBusy(true);
    setError("");
    try {
      const wallet = await api.unlockVault(password);
      setWallet(wallet);
      go("dashboard");
      toast("已解锁", "success");
    } catch (e) {
      const err = e as { code?: string; message: string };
      setError(err.message || "解锁失败");
      if (err.code === "TOO_MANY_ATTEMPTS") {
        const match = /(\d+)\s*秒/.exec(err.message) ?? /(\d+)/.exec(err.message);
        setCountdown(match ? Number(match[1]) : 30);
      }
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
      await api.resetVault();
      toast("本机钱包文件已删除", "info");
      setWallet(null as unknown as WalletView);
      await refreshStatus();
      go("onboarding");
    } catch (e) {
      toast((e as Error).message || "删除失败", "error");
    }
  };

  const locked = countdown > 0;
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
              disabled={locked}
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
          {countdown > 0 ? (
            <p className="error-text">尝试次数过多，请等待 {countdown} 秒</p>
          ) : error ? (
            <p className="error-text">{error}</p>
          ) : null}
          <button className="btn btn-primary btn-block" type="button" disabled={busy || locked} onClick={() => void submit()}>
            解锁
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
