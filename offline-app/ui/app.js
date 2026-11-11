// Lycoris Signer 前端逻辑。唯一入口 window.__TAURI__.invoke，三个命令：
// signer_prepare / signer_sign / signer_validate。除此之外零请求、零存储。

"use strict";

const $ = (id) => document.getElementById(id);

const el = {
  steps: document.querySelectorAll("#steps li"),
  payload: $("payload"),
  secret: $("secret"),
  toggleSecret: $("toggle-secret"),
  btnPrepare: $("btn-prepare"),
  prepareError: $("prepare-error"),
  cardInput: $("card-input"),
  cardReview: $("card-review"),
  cardResult: $("card-result"),
  summary: $("summary"),
  fromCheck: $("from-check"),
  confirmCheck: $("confirm-check"),
  btnSign: $("btn-sign"),
  btnBack: $("btn-back"),
  signedOut: $("signed-out"),
  btnCopy: $("btn-copy"),
  btnReset: $("btn-reset"),
};

let prepared = null; // 步骤 2 的解析结果缓存

function invoke(cmd, args) {
  const tauri = window.__TAURI__;
  if (!tauri || !tauri.invoke) {
    throw new Error("Tauri IPC 不可用（请在 App 内运行）");
  }
  return tauri.invoke(cmd, args);
}

function setStep(n) {
  el.steps.forEach((li) => {
    const k = Number(li.dataset.step);
    li.classList.toggle("active", k === n);
    li.classList.toggle("done", k < n);
  });
}

function show(card) {
  for (const c of [el.cardInput, el.cardReview, el.cardResult]) {
    c.classList.toggle("hidden", c !== card);
  }
}

function errorNote(target, message) {
  target.textContent = message;
  target.classList.remove("hidden");
}

// ---------- 秘密可见性 ----------
el.toggleSecret.addEventListener("click", () => {
  el.secret.type = el.secret.type === "password" ? "text" : "password";
});

// ---------- 步骤 1 → 2：解析 ----------
el.btnPrepare.addEventListener("click", () => {
  el.prepareError.classList.add("hidden");
  const payload = el.payload.value.trim();
  const secretRaw = el.secret.value.trim();
  if (!payload || !secretRaw) return;

  let secret;
  try {
    secret = parseSecret(secretRaw);
  } catch (e) {
    errorNote(el.prepareError, String(e.message || e));
    return;
  }

  el.btnPrepare.disabled = true;
  invoke("signer_prepare", { payload, secret })
    .then((res) => {
      prepared = res;
      renderSummary(res);
      setStep(2);
      show(el.cardReview);
    })
    .catch((e) => errorNote(el.prepareError, String(e)))
    .finally(() => {
      el.btnPrepare.disabled = false;
    });
});

// 密钥输入解析：助记词 / 口令 / 原始私钥 → WalletSecret 结构（与主钱包 serde 一致）。
function parseSecret(raw) {
  const text = raw.trim();
  // EVM 风格 hex 私钥（0x 前缀或 64 位 hex）→ 私钥环 secp256k1 槽
  if (/^(0x)?[0-9a-fA-F]{64}$/.test(text)) {
    const hex = text.toLowerCase().replace(/^0x/, "");
    return { mode: "private_key", keys: { secp256k1: hex } };
  }
  // WIF（Bitcoin 系，L/K 开头压缩格式；btcz/ltc 等前缀不同，这里只做最常见判断）
  if (/^[LK53][1-9A-HJ-NP-Za-km-z]{50,51}$/.test(text)) {
    return { mode: "private_key", keys: { secp256k1: text } };
  }
  // 助记词：12/15/18/21/24 个英文词，或中文口令模式
  const words = text.split(/\s+/);
  if (words.length >= 12 && words.length <= 25 && /^[\x20-\x7e]+$/.test(text)) {
    return { mode: "mnemonic", mnemonic: text };
  }
  // 其他：当口令（Passphrase 模式，与主钱包「口令钱包」一致，isolated = 每条链独立钥匙）
  return { mode: "passphrase", phrase: text, derivation: "isolated" };
}

// ---------- 摘要渲染 ----------
function renderSummary(res) {
  el.summary.textContent = "";
  for (const line of res.summary) {
    const row = document.createElement("div");
    row.className = "row-line";
    const dt = document.createElement("dt");
    dt.textContent = line.label;
    const dd = document.createElement("dd");
    dd.textContent = line.value;
    if (line.emphasis) dd.classList.add("emph");
    row.append(dt, dd);
    el.summary.append(row);
  }

  el.fromCheck.classList.remove("note-ok", "note-warn", "note-bad");
  if (res.fromMatches) {
    el.fromCheck.classList.add("note-ok");
    el.fromCheck.textContent = res.fromAddress
      ? `✓ 本机密钥地址一致：${res.fromAddress}`
      : "✓ 载荷与本机密钥匹配";
  } else {
    el.fromCheck.classList.add("note-bad");
    el.fromCheck.textContent = res.fromAddress
      ? `⚠ 载荷来自其他账户（载荷链上地址与本机派生不一致）。\
用这台设备签名会花掉别人的 UTXO 或被链拒绝。除非你明确知道自己在做什么，否则请停止。本机地址：${res.fromAddress}`
      : "⚠ 本机密钥不包含该链账户，无法核对来源地址。请停止。";
  }
  el.confirmCheck.checked = false;
  el.btnSign.disabled = true;
}

el.confirmCheck.addEventListener("change", () => {
  el.btnSign.disabled = !el.confirmCheck.checked;
});

// ---------- 步骤 2 → 3：签名 ----------
el.btnSign.addEventListener("click", () => {
  const payload = el.payload.value.trim();
  const secret = parseSecret(el.secret.value.trim());
  el.btnSign.disabled = true;
  invoke("signer_sign", { payload, secret })
    .then((res) => {
      el.signedOut.value = res.signed;
      setStep(3);
      show(el.cardResult);
      // 签名完成立即清掉敏感输入
      el.secret.value = "";
    })
    .catch((e) => {
      errorNote(el.prepareError, String(e));
      setStep(1);
      show(el.cardInput);
    })
    .finally(() => {
      el.btnSign.disabled = false;
    });
});

el.btnBack.addEventListener("click", () => {
  setStep(1);
  show(el.cardInput);
});

// ---------- 结果 ----------
el.btnCopy.addEventListener("click", () => {
  const text = el.signedOut.value;
  const done = () => {
    el.btnCopy.textContent = "已复制 ✓";
    setTimeout(() => {
      el.btnCopy.textContent = "复制签名载荷";
    }, 1500);
  };
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(text).then(done).catch(() => fallbackCopy(text, done));
  } else {
    fallbackCopy(text, done);
  }
});

function fallbackCopy(text, done) {
  el.signedOut.removeAttribute("readonly");
  el.signedOut.select();
  document.execCommand("copy");
  el.signedOut.setAttribute("readonly", "");
  window.getSelection()?.removeAllRanges();
  done();
}

el.btnReset.addEventListener("click", () => {
  el.payload.value = "";
  el.secret.value = "";
  el.signedOut.value = "";
  el.prepareError.classList.add("hidden");
  el.confirmCheck.checked = false;
  el.btnSign.disabled = true;
  prepared = null;
  setStep(1);
  show(el.cardInput);
});
