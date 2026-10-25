/** 钱包管理：一个加密文件里保存多个钱包。
 *
 * 所有钱包共用本机密码与地址簿；交易记录各自独立。切换钱包不需要重新
 * 输入密码，因为会话里已经解开了整个文件。 */

import { api } from "../api.js?v=24";
import { escapeHtml, formatDateTime } from "../format.js?v=24";
import { askPassword, confirmDialog, debounce, icon, openModal, setBusy, toast } from "../ui.js?v=24";

export function render(ctx) {
  return {
    html: `<section class="view">
      <div class="page-head">
        <div>
          <h1>钱包管理</h1>
          <div class="sub">同一个加密文件里保存多个钱包，共用本机密码、地址簿与设置；交易记录各自独立。</div>
        </div>
        <button class="btn btn-primary" type="button" id="wallet-add">${icon("plus")}新建钱包</button>
      </div>
      <div class="card card-pad" style="margin-top:18px" id="wallet-list"></div>
    </section>`,

    mount(root) {
      const container = root.querySelector("#wallet-list");
      const paint = () => {
        container.innerHTML = listMarkup(ctx);
        bindList(ctx, container, paint);
      };
      paint();
      root.querySelector("#wallet-add").addEventListener("click", () => void addFlow(ctx, paint));
    },
  };
}

/* ------------------------------------------------------------------ markup */

function modeLabel(wallet) {
  return wallet.mode === "passphrase"
    ? `口令 · ${wallet.derivation === "strict" ? "严格" : "隔离"}`
    : "助记词 · BIP-39";
}

function listMarkup(ctx) {
  const view = ctx.state.wallet;
  const wallets = view?.wallets ?? [];
  const rows = wallets
    .map((wallet) => {
      const isActive = wallet.active;
      const accountCount = isActive ? (view?.batchAccounts ?? []).length : 0;
      return `<div class="wallet-row ${isActive ? "is-active" : ""}" data-id="${wallet.id}">
      <span class="wallet-mark">${icon("wallet")}</span>
      <div class="wallet-main">
        <div class="wallet-name">${escapeHtml(wallet.label)}
          ${isActive ? '<span class="chip chip-ok">当前使用</span>' : ""}</div>
        <div class="wallet-sub">
          <span class="chip">${escapeHtml(modeLabel(wallet))}</span>
          <span class="fingerprint muted">${escapeHtml(wallet.fingerprint)}</span>
        </div>
        <div class="wallet-sub muted small">创建于 ${escapeHtml(
          formatDateTime(wallet.createdAt),
        )} · ${wallet.activityCount} 条交易记录${
          isActive && accountCount > 0 ? ` · ${accountCount} 个派生账户` : ""
        }</div>
      </div>
      <div class="wallet-actions">
        ${
          isActive
            ? ""
            : `<button class="btn btn-ghost btn-sm" type="button" data-action="switch">${icon(
                "check",
              )}切换</button>`
        }
        <button class="btn btn-ghost btn-sm" type="button" data-action="rename">${icon(
          "edit",
        )}重命名</button>
        <button class="btn btn-ghost btn-sm" type="button" data-action="delete" ${
          wallets.length <= 1 ? 'disabled title="至少保留一个钱包"' : ""
        }>${icon("trash")}删除</button>
      </div>
    </div>`;
    })
    .join("");

  return `<div class="section-title">
      <h2>全部钱包</h2>
      <span class="hint">共 ${wallets.length} 个 · 派生账户在「总览 → 资产」按需添加</span>
    </div>
    <div class="wallet-list">${rows}</div>
    <p class="hint" style="margin-top:12px">所有钱包共用同一个本地密码。派生账户只存「派生方式 + 序号」，不占额外密钥；只要记得助记词/口令，随时可以全部恢复。派生账户的切换与管理在「总览 → 资产」页进行。</p>`;
}

/* ----------------------------------------------------------------- binding */

function bindList(ctx, container, paint) {
  container.querySelectorAll(".wallet-row").forEach((row) => {
    const wallet = (ctx.state.wallet?.wallets ?? []).find((item) => item.id === row.dataset.id);
    if (!wallet) return;
    row
      .querySelector('[data-action="switch"]')
      ?.addEventListener("click", () => void switchTo(ctx, wallet, paint));
    row
      .querySelector('[data-action="rename"]')
      ?.addEventListener("click", () => void renameFlow(ctx, wallet, paint));
    row
      .querySelector('[data-action="delete"]')
      ?.addEventListener("click", () => void deleteFlow(ctx, wallet, paint));
  });
}

/* ----------------------------------------------------------------- actions */

async function switchTo(ctx, wallet, paint) {
  try {
    const view = await api.switchWallet(wallet.id);
    ctx.setWallet(view);
    paint();
    toast(`已切换到「${wallet.label}」`, "success");
  } catch (error) {
    toast(error.message, "error");
  }
}

function renameFlow(ctx, wallet, paint) {
  openModal(
    `<h2>重命名钱包</h2>
     <div class="field" style="margin-top:12px">
       <label for="wallet-label">钱包名称</label>
       <input class="input" id="wallet-label" maxlength="40" value="${escapeHtml(wallet.label)}" />
     </div>
     <p class="error-text" id="rename-error" hidden></p>
     <div class="modal-foot">
       <button class="btn btn-ghost" data-close type="button">取消</button>
       <button class="btn btn-primary" id="rename-save" type="button">保存</button>
     </div>`,
    {
      onMount(element, close) {
        const input = element.querySelector("#wallet-label");
        const error = element.querySelector("#rename-error");
        const button = element.querySelector("#rename-save");
        button.addEventListener("click", async () => {
          const value = input.value.trim();
          if (!value) {
            error.hidden = false;
            error.textContent = "名称不能为空";
            return;
          }
          setBusy(button, true, "保存中…");
          try {
            const view = await api.renameWallet(wallet.id, value);
            ctx.setWallet(view);
            close();
            paint();
            toast("名称已更新", "success");
          } catch (failure) {
            setBusy(button, false);
            error.hidden = false;
            error.textContent = failure.message;
          }
        });
        input.addEventListener("keydown", (event) => {
          if (event.key === "Enter") button.click();
        });
      },
    },
  );
}

async function deleteFlow(ctx, wallet, paint) {
  const confirmed = await confirmDialog({
    title: "删除钱包",
    danger: true,
    confirmText: "继续",
    body: `<div class="note note-danger">${icon("warn")}<div>「${escapeHtml(
      wallet.label,
    )}」将从本机钱包文件中移除。</div></div>
      <p class="hint">文件里保存的只是派生信息，<strong>只要还记得助记词或口令，随时可以重新导入</strong>。如果已经丢失备份，删除后就再也无法找回。</p>`,
  });
  if (!confirmed) return;
  const password = await askPassword({
    title: "输入密码确认删除",
    description: `删除「${escapeHtml(wallet.label)}」需要验证本机密码。`,
    confirmText: "删除",
  });
  if (!password) return;
  try {
    const view = await api.deleteWallet(wallet.id, password);
    ctx.setWallet(view);
    paint();
    toast("钱包已删除", "success");
  } catch (error) {
    toast(error.message, "error");
  }
}

/* ---------------------------------------------------------------- add flow */

function addFlow(ctx, onDone) {
  const local = {
    mode: "passphrase",
    phrase: "",
    derivation: "isolated",
    mnemonic: "",
    words: 12,
    name: "",
    preview: null,
    previewing: false,
    previewError: null,
    busy: false,
  };

  openModal(`<div id="add-wallet-body"></div>`, {
    onMount(element, close) {
      const body = element.querySelector("#add-wallet-body");
      const paint = () => {
        body.innerHTML = formMarkup(local);
        bindForm(body, local, { ctx, close, onDone, paint });
      };
      paint();
      void runPreview(body, local);
    },
  });
}

function formMarkup(local) {
  const passphrase = local.mode === "passphrase";
  return `<h2>新建钱包</h2>
    <p class="hint" style="margin:6px 0 12px">在当前保险库里添加一个钱包，并立即切换过去；其他钱包不受影响。</p>

    <div class="seg" role="group" aria-label="钱包类型" style="margin-bottom:14px">
      <button type="button" data-mode="passphrase" aria-pressed="${passphrase}">口令（哈希派生）</button>
      <button type="button" data-mode="mnemonic" aria-pressed="${!passphrase}">助记词（BIP-39）</button>
    </div>

    ${
      passphrase
        ? `<div class="field">
             <label for="wallet-phrase">口令</label>
             <textarea class="textarea" id="wallet-phrase" placeholder="例如：a quiet garden at dusk" autocomplete="off" spellcheck="false">${escapeHtml(
               local.phrase,
             )}</textarea>
           </div>
           <div class="field">
             <span class="field-label">派生方式</span>
             <div class="seg" role="group">
               <button type="button" data-derivation="isolated" aria-pressed="${
                 local.derivation === "isolated"
               }">隔离（推荐）</button>
               <button type="button" data-derivation="strict" aria-pressed="${
                 local.derivation === "strict"
               }">严格 H(phrase)</button>
             </div>
           </div>`
        : `<div class="field">
             <label for="wallet-mnemonic">助记词</label>
             <textarea class="textarea" id="wallet-mnemonic" placeholder="粘贴已有的 12 / 24 个英文单词，或点下方按钮生成" autocomplete="off" spellcheck="false">${escapeHtml(
               local.mnemonic,
             )}</textarea>
             <div class="row" style="gap:8px;margin-top:8px">
               <button class="btn btn-ghost btn-sm" type="button" id="wallet-generate">${icon(
                 "refresh",
               )}生成新助记词</button>
               <div class="seg" role="group" aria-label="助记词长度">
                 ${[12, 24]
                   .map(
                     (count) =>
                       `<button type="button" data-words="${count}" aria-pressed="${
                         local.words === count
                       }">${count} 词</button>`,
                   )
                   .join("")}
               </div>
             </div>
           </div>`
    }

    <div class="field">
      <label for="wallet-name">钱包名称（可选）</label>
      <input class="input" id="wallet-name" maxlength="40" placeholder="例如：储蓄 / 日常开销" value="${escapeHtml(
        local.name,
      )}" />
    </div>

    <div id="wallet-preview">${previewMarkup(local)}</div>
    <div class="modal-foot">
      <button class="btn btn-ghost" data-close type="button">取消</button>
      <button class="btn btn-primary" id="wallet-save" type="button" ${
        local.preview && !local.busy ? "" : "disabled"
      }>添加并切换</button>
    </div>`;
}

function previewMarkup(local) {
  if (local.previewing) {
    return `<div class="stack-sm"><div class="skeleton" style="height:58px"></div><div class="skeleton" style="height:58px"></div></div>`;
  }
  if (local.previewError) {
    return `<div class="note note-warn">${icon("warn")}<div>${escapeHtml(local.previewError)}</div></div>`;
  }
  if (!local.preview) return "";
  const preview = local.preview;
  return `<div class="stack-sm">
      <div class="row-between">
        <span class="field-label">派生预览</span>
        <span class="fingerprint muted">${escapeHtml(preview.fingerprint)}</span>
      </div>
      <div class="preview-grid">
        ${preview.accounts
          .map(
            (account) => `<div class="preview-account">
              <div class="head"><span class="dot" style="--accent:${account.accent}"></span>${escapeHtml(
                account.name,
              )}<span class="muted"> · ${escapeHtml(account.symbol)}</span></div>
              <div class="addr">${escapeHtml(account.address)}</div>
            </div>`,
          )
          .join("")}
      </div>
      <p class="hint">${escapeHtml(preview.formula)}</p>
    </div>`;
}

function secretOf(local) {
  return local.mode === "passphrase"
    ? { mode: "passphrase", phrase: local.phrase, derivation: local.derivation }
    : { mode: "mnemonic", mnemonic: local.mnemonic.trim() };
}

let previewToken = 0;

async function runPreview(body, local) {
  const empty =
    local.mode === "passphrase" ? !local.phrase.trim() : !local.mnemonic.trim();
  if (empty) {
    local.preview = null;
    local.previewError = null;
    local.previewing = false;
    patchPreview(body, local);
    return;
  }
  local.previewing = true;
  patchPreview(body, local);
  const token = ++previewToken;
  try {
    const preview = await api.previewDerivation(secretOf(local));
    if (token !== previewToken) return;
    local.preview = preview;
    local.previewError = null;
  } catch (error) {
    if (token !== previewToken) return;
    local.preview = null;
    local.previewError = error.message;
  } finally {
    if (token === previewToken) {
      local.previewing = false;
      patchPreview(body, local);
    }
  }
}

function patchPreview(body, local) {
  const container = body.querySelector("#wallet-preview");
  if (container) container.innerHTML = previewMarkup(local);
  const save = body.querySelector("#wallet-save");
  if (save) save.disabled = !(local.preview && !local.busy);
}

function bindForm(body, local, { ctx, close, onDone, paint }) {
  const schedule = debounce(() => void runPreview(body, local), 420);

  body.querySelectorAll("[data-mode]").forEach((button) => {
    button.addEventListener("click", () => {
      local.mode = button.dataset.mode;
      local.preview = null;
      local.previewError = null;
      paint();
      void runPreview(body, local);
    });
  });

  body.querySelectorAll("[data-derivation]").forEach((button) => {
    button.addEventListener("click", () => {
      local.derivation = button.dataset.derivation;
      paint();
      void runPreview(body, local);
    });
  });

  body.querySelectorAll("[data-words]").forEach((button) => {
    button.addEventListener("click", () => {
      local.words = Number(button.dataset.words);
      paint();
    });
  });

  const phrase = body.querySelector("#wallet-phrase");
  phrase?.addEventListener("input", () => {
    local.phrase = phrase.value;
    schedule();
  });

  const mnemonic = body.querySelector("#wallet-mnemonic");
  mnemonic?.addEventListener("input", () => {
    local.mnemonic = mnemonic.value;
    schedule();
  });

  const name = body.querySelector("#wallet-name");
  name?.addEventListener("input", () => {
    local.name = name.value;
  });

  body.querySelector("#wallet-generate")?.addEventListener("click", async (event) => {
    const button = event.currentTarget;
    setBusy(button, true, "生成中…");
    try {
      local.mnemonic = await api.generateMnemonic(local.words);
      const field = body.querySelector("#wallet-mnemonic");
      if (field) field.value = local.mnemonic;
      await runPreview(body, local);
    } catch (error) {
      toast(error.message, "error");
    } finally {
      setBusy(button, false);
    }
  });

  body.querySelector("#wallet-save")?.addEventListener("click", async (event) => {
    if (!local.preview || local.busy) return;
    local.busy = true;
    const button = event.currentTarget;
    setBusy(button, true, "添加中…");
    try {
      const view = await api.addWallet({
        name: local.name.trim() || null,
        secret: secretOf(local),
      });
      ctx.setWallet(view);
      close();
      onDone?.();
      toast(`已创建并切换到「${view.label}」`, "success");
    } catch (error) {
      local.busy = false;
      setBusy(button, false);
      toast(error.message, "error");
    }
  });
}

/* ------------------------------------------------------------- batch flow */

const STRATEGY_LABELS = {
  path: { title: "路径派生", desc: "BIP-32 地址索引递增，与主流钱包兼容（助记词）" },
  salted: { title: "加盐派生", desc: "H(域名‖密语‖#序号)，每个序号独立密钥" },
  iterated: { title: "迭代哈希", desc: "链哈希对自身迭代 N 次，序号即次数" },
};

/** Pin more derived accounts onto the current wallet. */
function batchFlow(ctx, onDone) {
  const wallet = ctx.state.wallet;
  if (!wallet) return;
  const local = {
    strategy: "salted",
    count: 5,
    start: 1,
    labelPrefix: "",
    previews: null,
    previewing: false,
    previewError: null,
    busy: false,
  };

  openModal(`<div id="batch-wallet-body"></div>`, {
    onMount(element, close) {
      const body = element.querySelector("#batch-wallet-body");
      const paint = () => {
        body.innerHTML = batchFormMarkup(local, wallet);
        bindBatchForm(body, local, { ctx, close, onDone, paint });
      };
      paint();
      void runBatchPreview(body, local);
    },
  });
}

function batchFormMarkup(local, wallet) {
  return `<h2>批量派生账户</h2>
    <p class="hint" style="margin:6px 0 12px">
      给「${escapeHtml(wallet.label)}」追加派生账户。所有账户共用这一份${wallet.mode === "mnemonic" ? "助记词" : "口令"}，
      只记录「派生方式 + 序号」，不额外保存任何密钥；切换账户时全链地址、余额与收款码一起跟随。
    </p>

    <div class="field">
      <span class="field-label">派生方式</span>
      <div class="seg seg-col" role="group">
        ${Object.entries(STRATEGY_LABELS)
          .map(
            ([key, meta]) =>
              `<button type="button" data-strategy="${key}" aria-pressed="${local.strategy === key}">
                 <span class="seg-title">${meta.title}</span><span class="seg-desc">${meta.desc}</span>
               </button>`,
          )
          .join("")}
      </div>
    </div>

    <div class="row" style="gap:12px">
      <div class="field" style="flex:1">
        <label for="batch-count">账户数量（1–${maxCount(local)}）</label>
        <input class="input" id="batch-count" type="number" min="1" max="${maxCount(local)}" value="${local.count}" />
      </div>
      <div class="field" style="flex:1">
        <label for="batch-start">起始序号</label>
        <input class="input" id="batch-start" type="number" min="0" max="${maxCount(local) - 1}" value="${local.start}" />
      </div>
      <div class="field" style="flex:2">
        <label for="batch-prefix">名称前缀（可选）</label>
        <input class="input" id="batch-prefix" type="text" placeholder="留空则用「${escapeHtml(wallet.label)} #序号」" value="${escapeHtml(
          local.labelPrefix,
        )}" />
      </div>
    </div>

    <div id="batch-preview">${batchPreviewMarkup(local)}</div>
    <div class="modal-foot">
      <button class="btn btn-ghost" data-close type="button">取消</button>
      <button class="btn btn-primary" id="batch-save" type="button" ${
        local.previews && !local.busy ? "" : "disabled"
      }>添加 ${local.count} 个账户</button>
    </div>`;
}

function maxCount(local) {
  return local.strategy === "iterated" ? 50 : 100;
}

function batchPreviewMarkup(local) {
  if (local.previewing) {
    return `<div class="stack-sm"><div class="skeleton" style="height:58px"></div><div class="skeleton" style="height:58px"></div></div>`;
  }
  if (local.previewError) {
    return `<div class="note note-warn">${icon("warn")}<div>${escapeHtml(local.previewError)}</div></div>`;
  }
  if (!local.previews) return "";
  return `<div class="stack-sm">
      <div class="field-label">派生预览（前 ${local.previews.length} 个 · 每个含全部链地址）</div>
      ${local.previews
        .map(
          (preview, offset) => `<div class="preview-account" style="margin-bottom:8px">
            <div class="head">
              <span class="dot" style="--accent:var(--vermilion)"></span>
              #${local.start + offset}
              <span class="fingerprint muted">${escapeHtml(preview.fingerprint)}</span>
            </div>
            <div class="addr">${escapeHtml(preview.accounts[0].address)} <span class="muted">+${preview.accounts.length - 1} 链</span></div>
          </div>`,
        )
        .join("")}
    </div>`;
}

let batchPreviewToken = 0;

async function runBatchPreview(body, local) {
  local.previewing = true;
  local.previewError = null;
  patchBatchPreview(body, local);
  const token = ++batchPreviewToken;
  try {
    const previews = await api.previewBatch({
      batch: { strategy: local.strategy, count: Math.min(local.count, 3), start: local.start },
    });
    if (token !== batchPreviewToken) return;
    local.previews = previews;
    local.previewError = null;
  } catch (error) {
    if (token !== batchPreviewToken) return;
    local.previews = null;
    local.previewError = error.message;
  } finally {
    if (token === batchPreviewToken) {
      local.previewing = false;
      patchBatchPreview(body, local);
    }
  }
}

function patchBatchPreview(body, local) {
  const container = body.querySelector("#batch-preview");
  if (container) container.innerHTML = batchPreviewMarkup(local);
  const save = body.querySelector("#batch-save");
  if (save) {
    save.textContent = `添加 ${local.count} 个账户`;
    save.disabled = !(local.previews && !local.busy);
  }
}

function bindBatchForm(body, local, { ctx, close, onDone, paint }) {
  body.querySelectorAll("[data-strategy]").forEach((button) => {
    button.addEventListener("click", () => {
      local.strategy = button.dataset.strategy;
      const max = maxCount(local);
      local.count = Math.min(local.count, max);
      local.start = Math.min(local.start, max - 1);
      paint();
      void runBatchPreview(body, local);
    });
  });

  const countField = body.querySelector("#batch-count");
  countField.addEventListener("input", () => {
    const value = Number(countField.value);
    if (Number.isFinite(value)) {
      local.count = Math.max(1, Math.min(maxCount(local), Math.floor(value)));
      patchBatchPreview(body, local);
    }
  });
  const startField = body.querySelector("#batch-start");
  startField.addEventListener("input", () => {
    const value = Number(startField.value);
    if (Number.isFinite(value)) {
      local.start = Math.max(0, Math.min(maxCount(local) - 1, Math.floor(value)));
      patchBatchPreview(body, local);
      debounce(() => void runBatchPreview(body, local), 420)();
    }
  });
  const prefixField = body.querySelector("#batch-prefix");
  prefixField.addEventListener("input", () => {
    local.labelPrefix = prefixField.value;
  });

  body.querySelector("#batch-save")?.addEventListener("click", async (event) => {
    if (!local.previews || local.busy) return;
    local.busy = true;
    const button = event.currentTarget;
    setBusy(button, true, "派生中…");
    try {
      const view = await api.addWalletsBatch({
        batch: { strategy: local.strategy, count: local.count, start: local.start },
        name: local.labelPrefix.trim() || null,
      });
      ctx.setWallet(view);
      close();
      onDone?.();
      toast(`已为当前钱包添加 ${local.count} 个派生账户`, "success");
    } catch (error) {
      local.busy = false;
      setBusy(button, false);
      toast(error.message, "error");
    }
  });
}

/* ------------------------------------------------ batch account management */

