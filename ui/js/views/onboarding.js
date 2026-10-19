/** Onboarding: choose a wallet mode, then either a BIP-39 mnemonic or a
 *  hashed passphrase, and finally encrypt everything behind a local password. */

import { api } from "../api.js?v=10";
import { escapeHtml, shortAddress, copyText } from "../format.js?v=10";
import { bindCopy, debounce, icon, setBusy, toast } from "../ui.js?v=10";

const STEPS = ["选择模式", "生成密钥", "设置密码"];

export function render(ctx) {
  const local = {
    step: 0,
    mode: null,
    mnemonic: "",
    words: 12,
    revealed: false,
    verify: null,
    verifyAnswers: {},
    importText: "",
    phrase: "",
    derivation: "isolated",
    preview: null,
    previewError: null,
    previewing: false,
    password: "",
    confirm: "",
    hint: "",
    error: null,
    busy: false,
  };

  const state = () => local;

  return {
    html: `<section class="onboarding"><div class="onboarding-inner" id="ob-body"></div></section>`,
    mount(root) {
      const body = root.querySelector("#ob-body");
      paint(body, local, ctx, state);
    },
  };
}

function stepBar(step) {
  return `<div class="steps">${STEPS.map((label, index) => {
    const stateValue = index < step ? "done" : index === step ? "active" : "todo";
    return `<div class="step" data-state="${stateValue}"><span class="idx">${
      index < step ? "✓" : index + 1
    }</span><span>${escapeHtml(label)}</span></div>${
      index < STEPS.length - 1 ? '<span class="sep"></span>' : ""
    }`;
  }).join("")}</div>`;
}

function brandHead() {
  return `<div class="brand-head">
      <svg class="mark" viewBox="0 0 32 32" aria-hidden="true">
        <g fill="var(--vermilion)">
          <ellipse cx="16" cy="9.2" rx="2.6" ry="5.2" />
          <ellipse cx="16" cy="9.2" rx="2.6" ry="5.2" transform="rotate(72 16 16)" />
          <ellipse cx="16" cy="9.2" rx="2.6" ry="5.2" transform="rotate(144 16 16)" />
          <ellipse cx="16" cy="9.2" rx="2.6" ry="5.2" transform="rotate(216 16 16)" />
          <ellipse cx="16" cy="9.2" rx="2.6" ry="5.2" transform="rotate(288 16 16)" />
        </g>
        <circle cx="16" cy="16" r="2.4" fill="var(--ink)" />
      </svg>
      <h1>Lycoris Wallet</h1>
      <p>本地优先的多链钱包 · 密钥与签名永远留在你的设备上</p>
    </div>`;
}

function paint(body, local, ctx, state) {
  if (local.step === 0) paintMode(body, local, ctx, state);
  else if (local.step === 1) paintSecret(body, local, ctx, state);
  else paintPassword(body, local, ctx, state);
}

/* ------------------------------------------------------------------ step 1 */

function paintMode(body, local, ctx, state) {
  body.innerHTML = `${brandHead()}${stepBar(0)}
    <div class="mode-cards">
      <button class="mode-card" data-mode="mnemonic" type="button">
        <span class="tag">BIP-39 · BIP-32 · SLIP-0010</span>
        <h3>助记词钱包</h3>
        <p>用 12 或 24 个单词的标准助记词管理全部链。地址按各链通用的派生路径生成，可以与主流钱包互相导入导出。</p>
        <div class="foot">适合长期使用 · 助记词可以抄写在纸上离线保存</div>
      </button>
      <button class="mode-card" data-mode="passphrase" type="button">
        <span class="tag">H_chain(phrase) → private key</span>
        <h3>口令钱包</h3>
        <p>输入任意一句话，直接用它所在链的原生哈希算法派生私钥：比特币用 SHA-256、莱特币用 scrypt、EVM 用 Keccak-256、Solana 用 SHA-256。</p>
        <div class="foot">一句话管所有链 · 口令一旦忘记无法找回</div>
      </button>
    </div>
    <div class="note note-warn" style="margin-top:18px">${icon("warn")}
      <div>口令钱包把安全性完全交给这句话：太短或太常见的口令会被暴力猜测。请使用足够长、只有你记得的句子。</div>
    </div>`;

  body.querySelectorAll("[data-mode]").forEach((button) => {
    button.addEventListener("click", () => {
      local.mode = button.dataset.mode;
      local.step = 1;
      if (local.mode === "mnemonic") void regenerate(local, ctx, state);
      else paint(body, local, ctx, state);
    });
  });
}

/* ------------------------------------------------------------------ step 2 */

async function regenerate(local, ctx, state) {
  local.mnemonic = await api.generateMnemonic(local.words);
  local.verify = null;
  local.verifyAnswers = {};
  local.revealed = false;
  paint(document.getElementById("ob-body"), local, ctx, state);
}

function previewBlock(local) {
  if (!local.preview && !local.previewing && !local.previewError) return "";
  if (local.previewing) {
    return `<div class="stack-sm"><div class="skeleton" style="height:64px"></div>
      <div class="skeleton" style="height:64px"></div></div>`;
  }
  if (local.previewError) {
    return `<div class="note note-warn">${icon("warn")}<div>${escapeHtml(local.previewError)}</div></div>`;
  }
  const preview = local.preview;
  return `<div class="stack-sm">
      <div class="row-between">
        <span class="field-label">实时派生预览</span>
        <span class="fingerprint muted">${escapeHtml(preview.fingerprint)}</span>
      </div>
      <div class="preview-grid">
        ${preview.accounts
          .map(
            (account) => `<div class="preview-account">
              <div class="head"><span class="dot" style="--accent:${account.accent}"></span>${escapeHtml(account.name)}
                <span class="muted">· ${escapeHtml(account.symbol)}</span></div>
              <div class="addr">${escapeHtml(account.address)}</div>
              <div class="algo">${escapeHtml(account.hashAlgorithm)} · ${escapeHtml(account.addressFormat)}</div>
            </div>`,
          )
          .join("")}
      </div>
      <p class="hint">${escapeHtml(preview.formula)}</p>
    </div>`;
}

function paintSecret(body, local, ctx, state) {
  if (local.mode === "mnemonic-import") {
    paintImport(body, local, ctx, state);
    return;
  }
  if (local.mode === "mnemonic") paintMnemonic(body, local, ctx, state);
  else paintPassphrase(body, local, ctx, state);
}

function paintMnemonic(body, local, ctx, state) {
  const words = local.mnemonic ? local.mnemonic.split(" ") : [];
  const verifyReady = words.length > 0;
  body.innerHTML = `${brandHead()}${stepBar(1)}
    <div class="card card-pad stack">
      <div class="row-between">
        <div>
          <h2>助记词</h2>
          <p class="hint">按顺序抄写到纸上。任何拿到这些单词的人都可以拿走全部资产。</p>
        </div>
        <div class="seg" role="group" aria-label="助记词长度">
          ${[12, 24]
            .map(
              (count) =>
                `<button type="button" data-words="${count}" aria-pressed="${local.words === count}">${count} 词</button>`,
            )
            .join("")}
        </div>
      </div>

      <div class="mnemonic-grid ${local.revealed ? "" : "mnemonic-blur"}" id="mnemonic-grid">
        ${words
          .map(
            (word, index) =>
              `<div class="mnemonic-word"><span class="idx">${index + 1}</span><span>${escapeHtml(word)}</span></div>`,
          )
          .join("")}
      </div>

      <div class="row" style="gap:8px">
        <button class="btn btn-ghost btn-sm" id="toggle-reveal" type="button">
          ${icon(local.revealed ? "eyeOff" : "eye")}${local.revealed ? "隐藏" : "显示"}
        </button>
        <button class="btn btn-ghost btn-sm" id="copy-mnemonic" type="button">${icon("copy")}复制</button>
        <button class="btn btn-ghost btn-sm" id="regenerate" type="button">${icon("refresh")}重新生成</button>
      </div>

      <div id="verify-block"></div>

      <div class="row" style="justify-content:flex-end">
        <button class="btn btn-ghost" id="to-import" type="button">改用已有助记词</button>
        <button class="btn btn-primary" id="to-password" type="button" ${verifyReady ? "" : "disabled"}>下一步</button>
      </div>
    </div>
    <div class="row" style="margin-top:14px"><button class="btn btn-ghost btn-sm" id="back" type="button">${icon("chevronLeft")}返回</button></div>`;

  body.querySelectorAll("[data-words]").forEach((button) => {
    button.addEventListener("click", () => {
      local.words = Number(button.dataset.words);
      void regenerate(local, ctx, state);
    });
  });
  body.querySelector("#toggle-reveal").addEventListener("click", () => {
    local.revealed = !local.revealed;
    paint(body, local, ctx, state);
  });
  body.querySelector("#copy-mnemonic").addEventListener("click", async () => {
    const ok = await copyText(local.mnemonic);
    toast(ok ? "助记词已复制（记得清理剪贴板）" : "复制失败", ok ? "success" : "error");
  });
  body.querySelector("#regenerate").addEventListener("click", () => void regenerate(local, ctx, state));
  body.querySelector("#back").addEventListener("click", () => {
    local.step = 0;
    paint(body, local, ctx, state);
  });
  body.querySelector("#to-import").addEventListener("click", () => {
    local.importText = local.importText || "";
    local.mode = "mnemonic-import";
    paint(body, local, ctx, state);
  });
  body.querySelector("#to-password").addEventListener("click", () => {
    if (!local.verify) {
      local.verify = makeVerify(words);
      paint(body, local, ctx, state);
      return;
    }
    local.step = 2;
    paint(body, local, ctx, state);
  });

  if (local.verify) paintVerify(body, local, ctx, state, words);
}

function makeVerify(words) {
  const total = words.length;
  const positions = new Set();
  while (positions.size < Math.min(3, total)) {
    positions.add(1 + Math.floor(Math.random() * total));
  }
  return [...positions].sort((a, b) => a - b);
}

function paintVerify(body, local, ctx, state, words) {
  const block = body.querySelector("#verify-block");
  if (!block) return;
  block.innerHTML = `<div class="divider"></div>
    <div class="stack-sm">
      <span class="field-label">校验一下：请补全下面几个单词</span>
      ${local.verify
        .map(
          (position) => `<div class="verify-row">
            <span class="ask">第 ${position} 个</span>
            <input class="input" data-verify="${position}" placeholder="输入单词" autocomplete="off" spellcheck="false" value="${escapeHtml(
              local.verifyAnswers[position] ?? "",
            )}" />
          </div>`,
        )
        .join("")}
      <p class="error-text" id="verify-error" hidden></p>
    </div>`;

  block.querySelectorAll("[data-verify]").forEach((input) => {
    input.addEventListener("input", () => {
      local.verifyAnswers[input.dataset.verify] = input.value.trim().toLowerCase();
    });
    input.addEventListener("keydown", (event) => {
      if (event.key !== "Enter") return;
      checkVerify(body, local, ctx, state, words);
    });
  });

  const next = body.querySelector("#to-password");
  next.textContent = "核对并继续";
  next.onclick = () => checkVerify(body, local, ctx, state, words);
}

function checkVerify(body, local, ctx, state, words) {
  const error = body.querySelector("#verify-error");
  const wrong = local.verify.filter(
    (position) => (local.verifyAnswers[position] ?? "") !== words[position - 1],
  );
  if (wrong.length > 0) {
    error.hidden = false;
    error.textContent = `第 ${wrong.join("、")} 个单词不正确，请对照抄写内容`;
    wrong.forEach((position) => {
      body.querySelector(`[data-verify="${position}"]`)?.setAttribute("aria-invalid", "true");
    });
    return;
  }
  local.step = 2;
  paint(body, local, ctx, state);
}

function paintImport(body, local, ctx, state) {
  body.innerHTML = `${brandHead()}${stepBar(1)}
    <div class="card card-pad stack">
      <div>
        <h2>导入助记词</h2>
        <p class="hint">支持 12 / 24 个英文单词（BIP-39）。请输入完整助记词，单词之间用空格分隔。</p>
      </div>
      <div class="field">
        <label for="import-words">助记词</label>
        <textarea class="textarea" id="import-words" placeholder="例如 abandon abandon … about" spellcheck="false" autocomplete="off">${escapeHtml(
          local.importText,
        )}</textarea>
      </div>
      <div id="import-preview">${previewBlock(local)}</div>
      <div class="row" style="justify-content:flex-end">
        <button class="btn btn-ghost" id="back" type="button">${icon("chevronLeft")}返回</button>
        <button class="btn btn-primary" id="to-password" type="button">下一步</button>
      </div>
    </div>`;

  const textarea = body.querySelector("#import-words");
  const runPreview = debounce(async (value) => {
    const words = value.trim().split(/\s+/).filter(Boolean);
    if (words.length === 0) {
      local.preview = null;
      local.previewError = null;
      local.previewing = false;
      body.querySelector("#import-preview").innerHTML = previewBlock(local);
      return;
    }
    local.previewing = true;
    body.querySelector("#import-preview").innerHTML = previewBlock(local);
    try {
      local.preview = await api.previewDerivation({ mode: "mnemonic", mnemonic: value.trim() });
      local.previewError = null;
    } catch (error) {
      local.preview = null;
      local.previewError = error.message;
    } finally {
      local.previewing = false;
      body.querySelector("#import-preview").innerHTML = previewBlock(local);
    }
  }, 380);

  textarea.addEventListener("input", () => {
    local.importText = textarea.value;
    runPreview(textarea.value);
  });
  body.querySelector("#back").addEventListener("click", () => {
    local.mode = "mnemonic";
    paint(body, local, ctx, state);
  });
  body.querySelector("#to-password").addEventListener("click", () => {
    if (!local.preview) {
      toast("请先输入有效的助记词", "error");
      return;
    }
    local.step = 2;
    paint(body, local, ctx, state);
  });
}

function paintPassphrase(body, local, ctx, state) {
  body.innerHTML = `${brandHead()}${stepBar(1)}
    <div class="card card-pad stack">
      <div>
        <h2>口令</h2>
        <p class="hint">这句话会被每条链的原生哈希算法处理，直接得到该链的私钥。同一个口令在隔离模式下每条链得到不同私钥。</p>
      </div>
      <div class="field">
        <label for="phrase">你的口令</label>
        <textarea class="textarea" id="phrase" placeholder="例如：a quiet garden at dusk, 2019 年的那场雨" autocomplete="off" spellcheck="false">${escapeHtml(
          local.phrase,
        )}</textarea>
      </div>

      <div class="field">
        <span class="field-label">派生方式</span>
        <div class="seg" role="group">
          <button type="button" data-derivation="isolated" aria-pressed="${local.derivation === "isolated"}">隔离（推荐）</button>
          <button type="button" data-derivation="strict" aria-pressed="${local.derivation === "strict"}">严格 H(phrase)</button>
        </div>
        <p class="hint" id="derivation-hint">${
          local.derivation === "isolated"
            ? '消息 = "lycoris/v1/&lt;链名&gt;\\0" + 口令，每条链的私钥都不同。'
            : "直接哈希口令本身。Ethereum / Ethereum Classic / BNB Chain / Polygon 使用同一算法，会得到同一把私钥。"
        }</p>
      </div>

      <div id="passphrase-preview">${previewBlock(local)}</div>

      <div class="row" style="justify-content:flex-end">
        <button class="btn btn-ghost" id="back" type="button">${icon("chevronLeft")}返回</button>
        <button class="btn btn-primary" id="to-password" type="button">下一步</button>
      </div>
    </div>`;

  const textarea = body.querySelector("#phrase");
  const runPreview = debounce(async (value) => {
    if (!value.trim()) {
      local.preview = null;
      local.previewError = null;
      body.querySelector("#passphrase-preview").innerHTML = previewBlock(local);
      return;
    }
    local.previewing = true;
    body.querySelector("#passphrase-preview").innerHTML = previewBlock(local);
    try {
      local.preview = await api.previewDerivation({
        mode: "passphrase",
        phrase: value,
        derivation: local.derivation,
      });
      local.previewError = null;
    } catch (error) {
      local.preview = null;
      local.previewError = error.message;
    } finally {
      local.previewing = false;
      body.querySelector("#passphrase-preview").innerHTML = previewBlock(local);
    }
  }, 420);

  textarea.addEventListener("input", () => {
    local.phrase = textarea.value;
    runPreview(textarea.value);
  });
  body.querySelectorAll("[data-derivation]").forEach((button) => {
    button.addEventListener("click", () => {
      local.derivation = button.dataset.derivation;
      paint(body, local, ctx, state);
      if (local.phrase.trim()) runPreview(local.phrase);
    });
  });
  body.querySelector("#back").addEventListener("click", () => {
    local.step = 0;
    paint(body, local, ctx, state);
  });
  body.querySelector("#to-password").addEventListener("click", () => {
    if (!local.preview) {
      toast("请先输入有效的口令", "error");
      return;
    }
    local.step = 2;
    paint(body, local, ctx, state);
  });
  if (local.phrase.trim() && !local.preview) runPreview(local.phrase);
}

/* ------------------------------------------------------------------ step 3 */

function strengthOf(password) {
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

function paintPassword(body, local, ctx, state) {
  const strength = strengthOf(local.password);
  const colors = ["var(--hair-strong)", "var(--danger)", "var(--warn)", "var(--ok)"];
  const mismatch = local.confirm.length > 0 && local.confirm !== local.password;
  const ready =
    local.password.length >= 8 && local.password === local.confirm && !local.busy;

  body.innerHTML = `${brandHead()}${stepBar(2)}
    <div class="card card-pad stack">
      <div>
        <h2>设置本地密码</h2>
        <p class="hint">密码只用于加密本机的钱包文件（Argon2id + XChaCha20-Poly1305）。它不会被上传，也无法找回。</p>
      </div>

      <div class="field">
        <label for="password">密码</label>
        <input class="input" id="password" type="password" autocomplete="new-password" placeholder="至少 8 个字符" value="${escapeHtml(
          local.password,
        )}" />
        <div class="row" style="gap:8px">
          <div style="flex:1;display:flex;gap:4px">
            ${[1, 2, 3]
              .map(
                (index) =>
                  `<span style="flex:1;height:4px;border-radius:2px;background:${
                    strength.level >= index ? colors[strength.level] : "var(--hair)"
                  }"></span>`,
              )
              .join("")}
          </div>
          <span class="hint" style="min-width:70px;text-align:right">${escapeHtml(strength.label)}</span>
        </div>
        <p class="hint">${escapeHtml(strength.hint)}</p>
      </div>

      <div class="field">
        <label for="confirm">确认密码</label>
        <input class="input" id="confirm" type="password" autocomplete="new-password" placeholder="再次输入" value="${escapeHtml(
          local.confirm,
        )}" ${mismatch ? 'aria-invalid="true"' : ""} />
        ${mismatch ? '<p class="error-text">两次输入不一致</p>' : ""}
      </div>

      <div class="field">
        <label for="hint">密码提示（可选）</label>
        <input class="input" id="hint" placeholder="例如：生日 + 老家门牌" maxlength="80" value="${escapeHtml(
          local.hint,
        )}" />
        <p class="hint">提示会以明文保存在钱包文件里，请勿写入真正的密码。</p>
      </div>

      ${local.error ? `<div class="note note-danger">${icon("warn")}<div>${escapeHtml(local.error)}</div></div>` : ""}

      <div class="row" style="justify-content:flex-end">
        <button class="btn btn-ghost" id="back" type="button">${icon("chevronLeft")}返回</button>
        <button class="btn btn-primary" id="create" type="button" ${ready ? "" : "disabled"}>创建钱包</button>
      </div>
    </div>`;

  body.querySelector("#password").addEventListener("input", (event) => {
    local.password = event.target.value;
    const cursor = event.target.selectionStart;
    paint(body, local, ctx, state);
    const next = body.querySelector("#password");
    next.focus();
    next.setSelectionRange(cursor, cursor);
  });
  body.querySelector("#confirm").addEventListener("input", (event) => {
    local.confirm = event.target.value;
    const cursor = event.target.selectionStart;
    paint(body, local, ctx, state);
    const next = body.querySelector("#confirm");
    next.focus();
    next.setSelectionRange(cursor, cursor);
  });
  body.querySelector("#hint").addEventListener("input", (event) => {
    local.hint = event.target.value;
  });
  body.querySelector("#back").addEventListener("click", () => {
    local.step = 1;
    paint(body, local, ctx, state);
  });
  body.querySelector("#create").addEventListener("click", async (event) => {
    if (local.busy) return;
    local.busy = true;
    local.error = null;
    setBusy(event.currentTarget, true, "正在创建…");
    try {
      const secret =
        local.mode === "passphrase"
          ? { mode: "passphrase", phrase: local.phrase, derivation: local.derivation }
          : local.mode === "mnemonic-import"
            ? { mode: "mnemonic", mnemonic: local.importText.trim() }
            : { mode: "mnemonic", mnemonic: local.mnemonic };
      const wallet = await api.createVault({
        password: local.password,
        hint: local.hint.trim() || null,
        label: null,
        secret,
      });
      ctx.setWallet(wallet);
      toast("钱包已创建并加密保存", "success");
      ctx.go("dashboard");
    } catch (error) {
      local.error = error.message;
      local.busy = false;
      paint(body, local, ctx, state);
    }
  });
}
