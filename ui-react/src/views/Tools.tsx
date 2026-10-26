/** 工具箱:地址体检 / 单位换算 / 助记词校验 / 哈希与编码 / 消息签名 / 支付 URI。
 *  纯前端编排,所有计算走 Rust 命令(演示模式由 mock 内核承担)。 */

import { useEffect, useMemo, useState } from "react";

import { api, isMock } from "../api";
import { useSession } from "../shell/session";
import { useDialogs, useTabScroll, useToast } from "../ui/overlay";
import { Icon } from "../ui/Icon";
import { copyText } from "../clipboard";
import { shortAddress } from "../format";
import type {
  AddressReport,
  ChainId,
  DigestOutput,
  EncodingOutput,
  MessageSignature,
  MnemonicCheck,
  ParsedUri,
  UnitConversion,
  VerifyResult,
} from "../types";

type ToolId = "inspect" | "units" | "mnemonic" | "hash" | "sign" | "uri";

const TOOLS: Array<{ id: ToolId; label: string; icon: string; blurb: string }> = [
  { id: "inspect", label: "地址体检", icon: "search", blurb: "粘贴任意地址,识别链与格式,检查校验和" },
  { id: "units", label: "单位换算", icon: "units", blurb: "20 条链的显示单位与最小单位互转" },
  { id: "mnemonic", label: "助记词校验", icon: "key", blurb: "BIP-39 词表与校验和检查,不派生不保存" },
  { id: "hash", label: "哈希 / 编码", icon: "hash", blurb: "SHA / Keccak / RIPEMD 与 hex / base64 / base58" },
  { id: "sign", label: "消息签名", icon: "signature", blurb: "EIP-191 / BIP-137 / ed25519 签名与验签" },
  { id: "uri", label: "支付 URI", icon: "receive", blurb: "BIP-21 / EIP-681 / Solana Pay 解析与构造" },
];

const CHAINS: Array<{ id: ChainId; name: string; symbol: string; decimals: number }> = [
  { id: "bitcoin", name: "Bitcoin", symbol: "BTC", decimals: 8 },
  { id: "litecoin", name: "Litecoin", symbol: "LTC", decimals: 8 },
  { id: "dogecoin", name: "Dogecoin", symbol: "DOGE", decimals: 8 },
  { id: "bitcoincash", name: "Bitcoin Cash", symbol: "BCH", decimals: 8 },
  { id: "ethereum", name: "Ethereum", symbol: "ETH", decimals: 18 },
  { id: "ethereum-classic", name: "Ethereum Classic", symbol: "ETC", decimals: 18 },
  { id: "bsc", name: "BNB Chain", symbol: "BNB", decimals: 18 },
  { id: "polygon", name: "Polygon", symbol: "POL", decimals: 18 },
  { id: "solana", name: "Solana", symbol: "SOL", decimals: 9 },
  { id: "monero", name: "Monero", symbol: "XMR", decimals: 12 },
  { id: "bitcoinsv", name: "Bitcoin SV", symbol: "BSV", decimals: 8 },
  { id: "namecoin", name: "Namecoin", symbol: "NMC", decimals: 8 },
  { id: "litecoincash", name: "Litecoin Cash", symbol: "LCC", decimals: 8 },
  { id: "feathercoin", name: "Feathercoin", symbol: "FTC", decimals: 8 },
  { id: "viacoin", name: "Viacoin", symbol: "VIA", decimals: 8 },
  { id: "potcoin", name: "Potcoin", symbol: "POT", decimals: 8 },
  { id: "egulden", name: "e-Gulden", symbol: "EFL", decimals: 8 },
  { id: "dash", name: "Dash", symbol: "DASH", decimals: 8 },
  { id: "groestlcoin", name: "Groestlcoin", symbol: "GRS", decimals: 8 },
  { id: "vertcoin", name: "Vertcoin", symbol: "VTC", decimals: 8 },
];

function Field({
  label,
  children,
  hint,
}: {
  label: string;
  children: React.ReactNode;
  hint?: string;
}) {
  return (
    <label className="field">
      <span className="kv-key">{label}</span>
      {children}
      {hint ? <span className="field-hint">{hint}</span> : null}
    </label>
  );
}

function CopyRow({ label, value }: { label: string; value: string }) {
  const toast = useToast();
  return (
    <div className="kv-row">
      <span className="kv-key">{label}</span>
      <span className="kv-value mono">{value}</span>
      <button
        type="button"
        className="btn btn-sm"
        onClick={() => {
          copyText(value);
          toast("已复制", "success");
        }}
      >
        <Icon name="copy" /> 复制
      </button>
    </div>
  );
}

/* ---------------------------------------------------------------- 地址体检 */

function InspectTool() {
  const [input, setInput] = useState("");
  const [report, setReport] = useState<AddressReport | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);

  const run = async () => {
    if (!input.trim()) return;
    setBusy(true);
    setError("");
    try {
      setReport(await api.toolInspectAddress(input.trim()));
    } catch (e) {
      setReport(null);
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="stack">
      <Field label="地址" hint="BTC / LTC / DOGE / BCH / EVM / SOL / XMR 与 2026 新增的 10 条链均可识别">
        <textarea
          className="textarea mono"
          rows={2}
          value={input}
          placeholder="bc1q… / 0x… / D… / 4…"
          onChange={(e) => setInput(e.target.value)}
        />
      </Field>
      <div className="row">
        <button type="button" className="btn" disabled={busy || !input.trim()} onClick={run}>
          <Icon name="search" /> {busy ? "检查中…" : "体检"}
        </button>
      </div>
      {error ? <p className="form-error">{error}</p> : null}
      {report ? (
        <div className="stack">
          {report.matches.length === 0 ? (
            <p className="form-error">没有一条受支持的链接受这个地址。</p>
          ) : (
            report.matches.map((match) => (
              <div key={match.chain} className="detail-card">
                <div className="row spread">
                  <strong>{match.chainName}</strong>
                  <span className="chip">{match.addressKind}</span>
                </div>
                <CopyRow label="规范地址" value={match.normalised} />
                {match.checksumOk === false ? (
                  <p className="form-error">EIP-55 大小写不匹配,请逐字核对。</p>
                ) : match.checksumOk ? (
                  <p className="form-ok">EIP-55 校验和通过。</p>
                ) : null}
              </div>
            ))
          )}
          {report.notes.map((note) => (
            <p key={note} className="field-hint">
              <Icon name="info" /> {note}
            </p>
          ))}
        </div>
      ) : null}
    </div>
  );
}

/* ---------------------------------------------------------------- 单位换算 */

function UnitsTool() {
  const [chain, setChain] = useState<ChainId>("bitcoin");
  const [value, setValue] = useState("1");
  const [fromSmallest, setFromSmallest] = useState(false);
  const [result, setResult] = useState<UnitConversion | null>(null);
  const [error, setError] = useState("");

  const meta = CHAINS.find((c) => c.id === chain)!;

  useEffect(() => {
    if (!value.trim()) return;
    let cancelled = false;
    setError("");
    api
      .toolConvertUnits(chain, value.trim(), fromSmallest)
      .then((out) => !cancelled && setResult(out))
      .catch((e) => {
        if (!cancelled) {
          setResult(null);
          setError((e as Error).message);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [chain, value, fromSmallest]);

  return (
    <div className="stack">
      <div className="row">
        <label className="field grow">
          <span className="kv-key">链</span>
          <select className="select" value={chain} onChange={(e) => setChain(e.target.value as ChainId)}>
            {CHAINS.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name} ({c.symbol})
              </option>
            ))}
          </select>
        </label>
        <label className="field grow">
          <span className="kv-key">{fromSmallest ? `最小单位 (${meta.decimals} 位)` : `${meta.symbol} 数量`}</span>
          <input
            className="input mono"
            value={value}
            onChange={(e) => setValue(e.target.value)}
            placeholder={fromSmallest ? "100000000" : "0.01"}
          />
        </label>
      </div>
      <label className="row check">
        <input
          type="checkbox"
          checked={fromSmallest}
          onChange={(e) => setFromSmallest(e.target.checked)}
        />
        <span>输入是最小单位(聪 / wei / lamport …)</span>
      </label>
      {error ? <p className="form-error">{error}</p> : null}
      {result ? (
        <div className="detail-card">
          <CopyRow label={`${result.symbol}`} value={result.decimal} />
          <CopyRow label="最小单位" value={result.smallest} />
          <p className="field-hint">
            1 {result.symbol} = 10^{result.decimals} 最小单位({result.decimals} 位小数)
          </p>
        </div>
      ) : null}
    </div>
  );
}

/* -------------------------------------------------------------- 助记词校验 */

function MnemonicTool() {
  const [input, setInput] = useState("");
  const [check, setCheck] = useState<MnemonicCheck | null>(null);
  const [error, setError] = useState("");

  const run = async () => {
    setError("");
    try {
      setCheck(await api.toolCheckMnemonic(input.trim()));
    } catch (e) {
      setCheck(null);
      setError((e as Error).message);
    }
  };

  return (
    <div className="stack">
      <Field label="助记词" hint="校验只在本机内存中进行,不会保存,也不会派生任何地址">
        <textarea
          className="textarea mono"
          rows={3}
          value={input}
          placeholder="abandon abandon … about"
          onChange={(e) => setInput(e.target.value)}
        />
      </Field>
      <div className="row">
        <button type="button" className="btn" disabled={!input.trim()} onClick={run}>
          <Icon name="check" /> 校验
        </button>
      </div>
      {error ? <p className="form-error">{error}</p> : null}
      {check ? (
        <div className="stack">
          <p className={check.valid ? "form-ok" : "form-error"}>{check.verdict}</p>
          <div className="word-grid">
            {check.words.map((w) => (
              <span key={`${w.index}-${w.word}`} className={w.valid ? "word-chip" : "word-chip is-bad"}>
                {w.index + 1}. {w.word}
              </span>
            ))}
          </div>
        </div>
      ) : null}
    </div>
  );
}

/* --------------------------------------------------------------- 哈希 / 编码 */

const HASH_ALGOS = [
  { id: "sha256", label: "SHA-256" },
  { id: "sha512", label: "SHA-512" },
  { id: "double-sha256", label: "SHA-256d" },
  { id: "keccak256", label: "Keccak-256" },
  { id: "hash160", label: "HASH160" },
  { id: "ripemd160", label: "RIPEMD-160" },
];

function HashTool() {
  const [input, setInput] = useState("");
  const [algorithm, setAlgorithm] = useState("sha256");
  const [digest, setDigest] = useState<DigestOutput | null>(null);
  const [encoding, setEncoding] = useState<EncodingOutput | null>(null);
  const [error, setError] = useState("");

  const run = async () => {
    setError("");
    try {
      const [d, e] = await Promise.all([api.toolHash(algorithm, input), api.toolEncode(input)]);
      setDigest(d);
      setEncoding(e);
    } catch (e2) {
      setDigest(null);
      setEncoding(null);
      setError((e2 as Error).message);
    }
  };

  return (
    <div className="stack">
      <div className="row">
        <label className="field grow">
          <span className="kv-key">算法</span>
          <select className="select" value={algorithm} onChange={(e) => setAlgorithm(e.target.value)}>
            {HASH_ALGOS.map((a) => (
              <option key={a.id} value={a.id}>
                {a.label}
              </option>
            ))}
          </select>
        </label>
      </div>
      <Field label="输入" hint="按 UTF-8 字节计算,不附加换行;0x 前缀按 hex 解码">
        <textarea
          className="textarea mono"
          rows={2}
          value={input}
          onChange={(e) => setInput(e.target.value)}
        />
      </Field>
      <div className="row">
        <button type="button" className="btn" disabled={!input.trim()} onClick={run}>
          <Icon name="hash" /> 计算
        </button>
      </div>
      {error ? <p className="form-error">{error}</p> : null}
      {digest ? (
        <div className="detail-card stack">
          <strong>{digest.algorithm}</strong>
          <CopyRow label="hex" value={digest.hex} />
          <CopyRow label="base64" value={digest.base64} />
        </div>
      ) : null}
      {encoding ? (
        <div className="detail-card stack">
          <strong>编码视图</strong>
          <CopyRow label="hex" value={encoding.hex} />
          <CopyRow label="base64" value={encoding.base64} />
          <CopyRow label="base58" value={encoding.base58} />
        </div>
      ) : null}
    </div>
  );
}

/* ---------------------------------------------------------------- 消息签名 */

const SIGN_CHAINS: Array<{ id: ChainId; label: string; scheme: string }> = [
  { id: "ethereum", label: "Ethereum / EVM", scheme: "EIP-191 personal_sign" },
  { id: "bitcoin", label: "Bitcoin", scheme: "BIP-137" },
  { id: "solana", label: "Solana", scheme: "ed25519 sign_message" },
];

function SignTool() {
  const { wallet } = useSession();
  const toast = useToast();
  const { askPassword } = useDialogs();
  const [chain, setChain] = useState<ChainId>("ethereum");
  const [message, setMessage] = useState("");
  const [signature, setSignature] = useState<MessageSignature | null>(null);
  const [verifyInput, setVerifyInput] = useState("");
  const [verifyChain, setVerifyChain] = useState<ChainId>("ethereum");
  const [verifyMessage, setVerifyMessage] = useState("");
  const [verified, setVerified] = useState<VerifyResult | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const scheme = SIGN_CHAINS.find((c) => c.id === chain)?.scheme ?? "";
  const account = wallet?.accounts.find((a) => a.chain === chain);

  const sign = async () => {
    if (!message.trim()) return;
    setBusy(true);
    setError("");
    try {
      const password = await askPassword({ title: "签名需要验证密码" });
      if (!password) return;
      const result = await api.toolSignMessage(chain, message, password);
      setSignature(result);
      toast("签名完成", "success");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };

  const verify = async () => {
    setError("");
    setVerified(null);
    try {
      setVerified(await api.toolVerifyMessage(verifyChain, verifyMessage, verifyInput));
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <div className="stack">
      <div className="detail-card stack">
        <strong>签名(用当前钱包的密钥)</strong>
        <div className="row">
          <label className="field grow">
            <span className="kv-key">链</span>
            <select className="select" value={chain} onChange={(e) => setChain(e.target.value as ChainId)}>
              {SIGN_CHAINS.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.label} · {c.scheme}
                </option>
              ))}
            </select>
          </label>
        </div>
        {account ? (
          <p className="field-hint">
            签名者:{shortAddress(account.address)}({scheme})
          </p>
        ) : null}
        <Field label="消息">
          <textarea
            className="textarea mono"
            rows={2}
            value={message}
            onChange={(e) => setMessage(e.target.value)}
          />
        </Field>
        <div className="row">
          <button type="button" className="btn" disabled={busy || !message.trim()} onClick={sign}>
            <Icon name="signature" /> {busy ? "签名中…" : "签名"}
          </button>
        </div>
        {signature ? (
          <div className="stack">
            <CopyRow label="签名 (hex)" value={signature.signatureHex} />
            {signature.address ? <CopyRow label="签名者" value={signature.address} /> : null}
          </div>
        ) : null}
      </div>

      <div className="detail-card stack">
        <strong>验签(恢复签名者)</strong>
        <div className="row">
          <label className="field grow">
            <span className="kv-key">链</span>
            <select
              className="select"
              value={verifyChain}
              onChange={(e) => setVerifyChain(e.target.value as ChainId)}
            >
              {SIGN_CHAINS.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.label}
                </option>
              ))}
            </select>
          </label>
        </div>
        <Field
          label="签名"
          hint={
            verifyChain === "solana"
              ? "格式:地址:签名hex"
              : verifyChain === "ethereum"
                ? "65 字节 hex(r‖s‖v)"
                : "base64 或 130 位 hex"
          }
        >
          <textarea
            className="textarea mono"
            rows={2}
            value={verifyInput}
            onChange={(e) => setVerifyInput(e.target.value)}
          />
        </Field>
        <Field label="原始消息">
          <textarea
            className="textarea mono"
            rows={2}
            value={verifyMessage}
            onChange={(e) => setVerifyMessage(e.target.value)}
          />
        </Field>
        <div className="row">
          <button
            type="button"
            className="btn"
            disabled={!verifyInput.trim() || !verifyMessage.trim()}
            onClick={verify}
          >
            <Icon name="check" /> 验证
          </button>
        </div>
        {verified ? (
          verified.verified ? (
            <div className="form-ok">
              <Icon name="check" /> 验证通过 · 签名者 {verified.signer}
            </div>
          ) : (
            <p className="form-error">
              <Icon name="warn" /> 签名与消息不匹配
            </p>
          )
        ) : null}
      </div>
      {error ? <p className="form-error">{error}</p> : null}
    </div>
  );
}

/* ---------------------------------------------------------------- 支付 URI */

function UriTool() {
  const { go } = useSession();
  const toast = useToast();
  const [mode, setMode] = useState<"parse" | "compose">("parse");
  const [input, setInput] = useState("");
  const [parsed, setParsed] = useState<ParsedUri | null>(null);
  const [error, setError] = useState("");

  const [chain, setChain] = useState<ChainId>("bitcoin");
  const [address, setAddress] = useState("");
  const [amount, setAmount] = useState("");
  const [label, setLabel] = useState("");
  const [message, setMessage] = useState("");
  const [composed, setComposed] = useState("");

  const parse = async () => {
    setError("");
    setParsed(null);
    try {
      setParsed(await api.toolParseUri(input.trim()));
    } catch (e) {
      setError((e as Error).message);
    }
  };

  const compose = async () => {
    setError("");
    setComposed("");
    try {
      const uri = await api.toolComposeUri(
        chain,
        address.trim(),
        amount.trim() || null,
        label.trim() || null,
        message.trim() || null,
      );
      setComposed(uri);
    } catch (e) {
      setError((e as Error).message);
    }
  };

  return (
    <div className="stack">
      <div className="tabs">
        <button className="tab" aria-pressed={mode === "parse"} onClick={() => setMode("parse")}>
          解析
        </button>
        <button className="tab" aria-pressed={mode === "compose"} onClick={() => setMode("compose")}>
          构造
        </button>
      </div>

      {mode === "parse" ? (
        <div className="stack">
          <Field label="支付 URI" hint="BIP-21(btc/ltc/doge/bch…)、EIP-681(eth…)、Solana Pay(solana:…)">
            <textarea
              className="textarea mono"
              rows={2}
              value={input}
              placeholder="bitcoin:bc1q…?amount=0.01"
              onChange={(e) => setInput(e.target.value)}
            />
          </Field>
          <div className="row">
            <button type="button" className="btn" disabled={!input.trim()} onClick={parse}>
              <Icon name="search" /> 解析
            </button>
          </div>
          {error ? <p className="form-error">{error}</p> : null}
          {parsed ? (
            <div className="detail-card stack">
              <div className="row spread">
                <strong>{parsed.chainName}</strong>
                <span className="chip">{parsed.symbol}</span>
              </div>
              <CopyRow label="收款地址" value={parsed.address} />
              {parsed.amount ? <CopyRow label="金额" value={`${parsed.amount} ${parsed.symbol}`} /> : null}
              {parsed.label ? <CopyRow label="标签" value={parsed.label} /> : null}
              {parsed.message ? <CopyRow label="消息" value={parsed.message} /> : null}
              {parsed.warnings.map((w) => (
                <p key={w} className="form-error">
                  <Icon name="warn" /> {w}
                </p>
              ))}
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => go("send", { chain: parsed.chain, to: parsed.address })}
              >
                <Icon name="send" /> 带入转账
              </button>
            </div>
          ) : null}
        </div>
      ) : (
        <div className="stack">
          <div className="row">
            <label className="field grow">
              <span className="kv-key">链</span>
              <select className="select" value={chain} onChange={(e) => setChain(e.target.value as ChainId)}>
                {CHAINS.filter((c) => c.id !== "monero").map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name} ({c.symbol})
                  </option>
                ))}
              </select>
            </label>
          </div>
          <Field label="收款地址">
            <input
              className="input mono"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              placeholder="地址"
            />
          </Field>
          <div className="row">
            <label className="field grow">
              <span className="kv-key">金额(可选)</span>
              <input className="input mono" value={amount} onChange={(e) => setAmount(e.target.value)} />
            </label>
            <label className="field grow">
              <span className="kv-key">标签(可选)</span>
              <input className="input" value={label} onChange={(e) => setLabel(e.target.value)} />
            </label>
          </div>
          <Field label="消息(可选)">
            <input className="input" value={message} onChange={(e) => setMessage(e.target.value)} />
          </Field>
          <div className="row">
            <button type="button" className="btn" disabled={!address.trim()} onClick={compose}>
              <Icon name="receive" /> 生成 URI
            </button>
          </div>
          {error ? <p className="form-error">{error}</p> : null}
          {composed ? (
            <div className="detail-card stack">
              <CopyRow label="URI" value={composed} />
              <button
                type="button"
                className="btn btn-sm"
                onClick={() => {
                  copyText(composed);
                  toast("URI 已复制", "success");
                }}
              >
                <Icon name="copy" /> 复制
              </button>
            </div>
          ) : null}
        </div>
      )}
    </div>
  );
}

/* -------------------------------------------------------------------- 主视图 */

export function ToolsView() {
  const [tool, setTool] = useState<ToolId>("inspect");
  const tabsRef = useTabScroll<HTMLDivElement>();
  const active = TOOLS.find((t) => t.id === tool)!;

  const body = useMemo(() => {
    switch (tool) {
      case "inspect":
        return <InspectTool />;
      case "units":
        return <UnitsTool />;
      case "mnemonic":
        return <MnemonicTool />;
      case "hash":
        return <HashTool />;
      case "sign":
        return <SignTool />;
      case "uri":
        return <UriTool />;
    }
  }, [tool]);

  return (
    <section className="view" data-view="tools">
      <header className="page-head">
        <div>
          <h1>工具箱</h1>
          <p className="page-sub">常用小工具都在这里;计算全部在本机完成{isMock ? "(演示模式为演示数据)" : ""}。</p>
        </div>
      </header>
      <div className="tool-grid" ref={tabsRef}>
        {TOOLS.map((t) => (
          <button
            key={t.id}
            type="button"
            className={`tool-card${t.id === tool ? " is-active" : ""}`}
            onClick={() => setTool(t.id)}
          >
            <Icon name={t.icon} />
            <strong>{t.label}</strong>
            <span>{t.blurb}</span>
          </button>
        ))}
      </div>
      <div className="detail-card stack">
        <strong>{active.label}</strong>
        {body}
      </div>
    </section>
  );
}
