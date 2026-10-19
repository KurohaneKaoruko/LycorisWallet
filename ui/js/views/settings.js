/** Settings: wallet identity, backups, appearance, security, danger zone. */

import { api, walletFull } from "../api.js";
import { copyText, escapeHtml, formatDateTime } from "../format.js";
import { askPassword, bindCopy, confirmDialog, icon, openModal, setBusy, toast } from "../ui.js";

const AUTO_LOCK_OPTIONS = [1, 5, 15, 30, 60];

/** 链开关计算用的全量链 id（顺序无关，仅用于集合运算）。 */
const ALL_CHAINS = [];

export function render(ctx) {
  return {
    html: `<section class="view">
      <div class="page-head">
        <div>
          <h1>设置</h1>
          <div class="sub">设置与地址簿都保存在加密的钱包文件里，跟随密码保护。</div>
        </div>
      </div>
      <div class="settings-grid" id="settings-root"></div>
    </section>`,

    mount(root) {
      const container = root.querySelector("#settings-root");
      const paint = (allAccounts) => {
        const accounts = allAccounts ?? ctx.state.wallet?.accounts ?? [];
        ALL_CHAINS.length = 0;
        ALL_CHAINS.push(...accounts.map((a) => a.chain));
        container.innerHTML = markup(ctx, accounts);
        bind(ctx, container, paint);
      };
      // 链开关需要全量账户列表（state.wallet 已被过滤）
      walletFull()
        .then((full) => paint(full?.accounts ?? []))
        .catch(() => paint());
    },
  };
}

/* ------------------------------------------------------------------ markup */

function markup(ctx, allAccounts = ctx.state.wallet?.accounts ?? []) {
  const wallet = ctx.state.wallet;
  return `${walletCard(wallet)}${chainsCard(wallet, allAccounts)}${appearanceCard(wallet.settings)}${securityCard(wallet)}${dangerCard()}`;
}

function walletCard(wallet) {
  const modeLabel =
    wallet.mode === "passphrase"
      ? `口令 · ${wallet.derivation === "strict" ? "严格" : "隔离"}`
      : "助记词 · BIP-39";
  const count = wallet.wallets?.length ?? 1;
  return `<div class="card card-pad stack-sm">
    <div class="section-title"><h2>钱包</h2><span class="chip">${escapeHtml(modeLabel)}</span></div>
    <div class="kv"><span class="kv-key">名称</span><span class="kv-value">${escapeHtml(wallet.label)}</span></div>
    <div class="kv"><span class="kv-key">指纹</span><span class="kv-value fingerprint">${escapeHtml(
      wallet.fingerprint,
    )}</span></div>
    <div class="kv"><span class="kv-key">创建时间</span><span class="kv-value">${escapeHtml(
      formatDateTime(wallet.createdAt),
    )}</span></div>
    <div class="kv"><span class="kv-key">派生公式</span><span class="kv-value">${escapeHtml(
      wallet.formula,
    )}</span></div>
    <div class="row" style="flex-wrap:wrap;margin-top:4px">
      <button class="btn btn-ghost btn-sm" type="button" data-action="rename">${icon("edit")}重命名</button>
      ${
        wallet.mode === "mnemonic"
          ? `<button class="btn btn-ghost btn-sm" type="button" data-action="backup">${icon(
              "shield",
            )}备份助记词</button>`
          : ""
      }
      <button class="btn btn-ghost btn-sm" type="button" data-action="export">${icon("key")}导出私钥</button>
      <button class="btn btn-ghost btn-sm" type="button" data-action="wallets">${icon(
        "wallet",
      )}钱包管理（共 ${count} 个）</button>
    </div>
    <p class="hint">${
      wallet.mode === "mnemonic"
        ? "助记词是恢复资产的唯一凭证，请离线抄写并妥善保存。"
        : "口令钱包没有助记词：口令一旦忘记，任何人都无法帮你找回。"
    }</p>
  </div>`;
}

function chainsCard(wallet, allAccounts) {
  const visible = wallet.settings?.visibleChains;
  const isAll = !Array.isArray(visible) || visible.length === 0;
  return `<div class="card card-pad">
    <div class="section-title"><h2>链开关</h2></div>
    <div class="setting-row" style="border-bottom:1px dashed var(--hair)">
      <div class="text"><div class="name">显示的链</div><div class="desc">关闭只是隐藏入口，地址与密钥仍会为全部 ${allAccounts.length} 条链派生。</div></div>
      <button class="btn btn-ghost btn-sm" type="button" data-chains-all>全部开启</button>
    </div>
    <div class="chain-toggle-list" id="chain-toggle-list">
      ${allAccounts
        .map((a) => {
          const on = isAll || visible.includes(a.chain);
          return `<div class="chain-toggle-row">
            <span class="asset-mark" style="--accent:${a.accent}">${escapeHtml(a.symbol.slice(0, 2))}</span>
            <div class="text" style="flex:1;min-width:0">
              <div class="name">${escapeHtml(a.name)} <span class="muted small">${escapeHtml(a.symbol)}</span></div>
            </div>
            <button class="switch" type="button" role="switch" aria-checked="${on}" data-chain-toggle="${a.chain}" aria-label="显示 ${escapeHtml(a.name)}"></button>
          </div>`;
        })
        .join("")}
    </div>
  </div>`;
}

function appearanceCard(settings) {
  const themes = [
    ["system", "跟随系统"],
    ["light", "浅色"],
    ["dark", "深色"],
  ];
  return `<div class="card card-pad">
    <div class="section-title"><h2>外观与货币</h2></div>
    <div class="setting-row">
      <div class="text"><div class="name">计价货币</div><div class="desc">总资产与手续费折算使用的法币。</div></div>
      <div class="seg">
        <button type="button" data-fiat="usd" aria-pressed="${settings.fiat === "usd"}">USD</button>
        <button type="button" data-fiat="cny" aria-pressed="${settings.fiat === "cny"}">CNY</button>
      </div>
    </div>
    <div class="setting-row">
      <div class="text"><div class="name">主题</div><div class="desc">浅色像和纸，深色像夜里的砚台。</div></div>
      <div class="seg">
        ${themes
          .map(
            ([value, label]) =>
              `<button type="button" data-choice="${value}" aria-pressed="${
                settings.theme === value
              }">${label}</button>`,
          )
          .join("")}
      </div>
    </div>
    <div class="setting-row">
      <div class="text"><div class="name">隐藏余额</div><div class="desc">总览页金额模糊显示，适合在公共场合查看。</div></div>
      <button class="switch" type="button" role="switch" aria-checked="${
        settings.hideBalances
      }" data-switch="hideBalances" aria-label="隐藏余额"></button>
    </div>
    <div class="setting-row">
      <div class="text"><div class="name">价格服务</div><div class="desc">联网获取币价并把余额折算成法币；关闭后只显示数量。</div></div>
      <button class="switch" type="button" role="switch" aria-checked="${
        settings.priceService
      }" data-switch="priceService" aria-label="价格服务"></button>
    </div>
  </div>`;
}

function securityCard(wallet) {
  const options = [...new Set([...AUTO_LOCK_OPTIONS, wallet.settings.autoLockMinutes])].sort(
    (a, b) => a - b,
  );
  return `<div class="card card-pad">
    <div class="section-title"><h2>安全</h2></div>
    <div class="setting-row">
      <div class="text"><div class="name">自动锁定</div><div class="desc">闲置超过设定时长后自动锁定，密钥会从内存中清除。</div></div>
      <select class="select" data-select="autoLockMinutes" style="width:auto">
        ${options
          .map(
            (minutes) =>
              `<option value="${minutes}" ${
                wallet.settings.autoLockMinutes === minutes ? "selected" : ""
              }>${minutes} 分钟${minutes === 5 ? "（默认）" : ""}</option>`,
          )
          .join("")}
      </select>
    </div>
    <div class="setting-row">
      <div class="text"><div class="name">修改密码</div><div class="desc">用新密码重新加密钱包文件，助记词与口令本身不会改变。</div></div>
      <button class="btn btn-ghost btn-sm" type="button" data-action="change-password">${icon(
        "lock",
      )}修改密码</button>
    </div>
    <div class="setting-row">
      <div class="text"><div class="name">立即锁定</div><div class="desc">结束当前会话，回到解锁界面。</div></div>
      <button class="btn btn-ghost btn-sm" type="button" data-action="lock">${icon("lock")}立即锁定</button>
    </div>
  </div>`;
}

function dangerCard() {
  return `<div class="card card-pad">
    <div class="section-title"><h2>危险区域</h2></div>
    <div class="note note-danger">${icon("warn")}<div>删除会移除本机的加密钱包文件（其中可能保存着多个钱包）。<strong>如果你没有备份助记词（或忘记了口令），资产将永久无法恢复。</strong></div></div>
    <div class="row" style="margin-top:14px">
      <button class="btn btn-danger" type="button" data-action="delete">${icon(
        "trash",
      )}删除本地钱包</button>
    </div>
  </div>`;
}

/* ----------------------------------------------------------------- binding */

function bind(ctx, container, paint) {
  const patch = async (changes, { repaint = true } = {}) => {
    try {
      const next = await api.saveSettings({ ...ctx.state.wallet.settings, ...changes });
      ctx.state.wallet = { ...ctx.state.wallet, settings: next };
      ctx.applyTheme();
      if (repaint) paint();
    } catch (error) {
      toast(error.message, "error");
      paint();
    }
  };

  container.querySelectorAll("[data-fiat]").forEach((button) => {
    button.addEventListener("click", () => void patch({ fiat: button.dataset.fiat }));
  });
  container.querySelectorAll("[data-choice]").forEach((button) => {
    button.addEventListener("click", () => void patch({ theme: button.dataset.choice }));
  });
  container.querySelectorAll("[data-switch]").forEach((button) => {
    button.addEventListener("click", () => {
      const key = button.dataset.switch;
      const next = button.getAttribute("aria-checked") !== "true";
      button.setAttribute("aria-checked", String(next));
      void patch({ [key]: next });
    });
  });
  container.querySelector('[data-select="autoLockMinutes"]')?.addEventListener("change", (event) => {
    void patch({ autoLockMinutes: Number(event.target.value) });
  });

  // 链开关：切换单链显示 / 一键全开。
  // 不整页重绘（连续快速点击会互相覆盖），只翻转开关本体；保存串行排队，
  // 每次保存读取闭包里最新的 visible 集合，后点击的永远最后落盘。
  const chainToggles = container.querySelectorAll("[data-chain-toggle]");
  if (chainToggles.length) {
    let visible;
    const raw = ctx.state.wallet.settings.visibleChains;
    if (Array.isArray(raw) && raw.length > 0) visible = new Set(raw);
    else visible = new Set(ALL_CHAINS); // null/空 = 全部显示

    let saveQueue = Promise.resolve();
    const persist = () => {
      saveQueue = saveQueue.then(() =>
        patch({ visibleChains: [...visible] }, { repaint: false }),
      );
      return saveQueue;
    };
    chainToggles.forEach((button) => {
      button.addEventListener("click", () => {
        const chain = button.dataset.chainToggle;
        if (visible.has(chain)) {
          if (visible.size <= 1) return; // 至少保留一条链
          visible.delete(chain);
        } else {
          visible.add(chain);
        }
        button.setAttribute("aria-checked", String(visible.has(chain)));
        persist();
      });
    });
    container.querySelector("[data-chains-all]")?.addEventListener("click", () => {
      if (visible.size === ALL_CHAINS.length) return;
      visible = new Set(ALL_CHAINS);
      chainToggles.forEach((b) => b.setAttribute("aria-checked", "true"));
      persist();
    });
  }
  container.querySelector('[data-action="rename"]')?.addEventListener("click", () => void rename(ctx, paint));
  container
    .querySelector('[data-action="wallets"]')
    ?.addEventListener("click", () => ctx.go("wallets"));
  container.querySelector('[data-action="backup"]')?.addEventListener("click", () => void backup());
  container.querySelector('[data-action="export"]')?.addEventListener("click", () => void exportKeys(ctx));
  container.querySelector('[data-action="change-password"]')?.addEventListener("click", () => changePassword());
  container.querySelector('[data-action="lock"]')?.addEventListener("click", () => void ctx.lockNow());
  container.querySelector('[data-action="delete"]')?.addEventListener("click", () => void erase(ctx));
}

/* ----------------------------------------------------------------- actions */

async function rename(ctx, paint) {
  const wallet = ctx.state.wallet;
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

async function backup() {
  const password = await askPassword({
    title: "备份助记词",
    description: "助记词等同于全部资产的控制权。请确认周围没有人偷看或录制。",
    confirmText: "查看助记词",
  });
  if (!password) return;
  let mnemonic;
  try {
    mnemonic = await api.revealMnemonic(password);
  } catch (error) {
    toast(error.message, "error");
    return;
  }
  const words = mnemonic.trim().split(/\s+/);
  openModal(
    `<h2>助记词</h2>
     <p class="hint" style="margin:6px 0 12px">按顺序抄写到纸上或金属板上，不要截图，也不要保存在联网设备里。</p>
     <div class="mnemonic-grid mnemonic-blur" id="mnemonic-view">
       ${words
         .map(
           (word, index) =>
             `<div class="mnemonic-word"><span class="idx">${index + 1}</span><span>${escapeHtml(
               word,
             )}</span></div>`,
         )
         .join("")}
     </div>
     <div class="row" style="margin-top:12px;gap:8px">
       <button class="btn btn-ghost btn-sm" id="mnemonic-reveal" type="button">${icon("eye")}显示</button>
       <button class="btn btn-ghost btn-sm" id="mnemonic-copy" type="button">${icon("copy")}复制</button>
     </div>
     <div class="note note-warn" style="margin-top:12px">${icon("warn")}<div>任何人得到这些单词都能拿走全部资产。</div></div>
     <div class="modal-foot"><button class="btn btn-primary" data-close type="button">我已抄写完成</button></div>`,
    {
      onMount(element) {
        const grid = element.querySelector("#mnemonic-view");
        const reveal = element.querySelector("#mnemonic-reveal");
        reveal.addEventListener("click", () => {
          grid.classList.toggle("mnemonic-blur");
          const hidden = grid.classList.contains("mnemonic-blur");
          reveal.innerHTML = `${icon(hidden ? "eye" : "eyeOff")}${hidden ? "显示" : "隐藏"}`;
        });
        element.querySelector("#mnemonic-copy").addEventListener("click", async () => {
          const ok = await copyText(mnemonic);
          toast(ok ? "助记词已复制（记得清理剪贴板）" : "复制失败", ok ? "success" : "error");
        });
      },
    },
  );
}

async function exportKeys(ctx) {
  const password = await askPassword({
    title: "导出私钥",
    description: "验证一次密码后，可在本窗口切换网络。私钥可以单独控制对应链的全部资产。",
    confirmText: "验证",
  });
  if (!password) return;
  const accounts = ctx.state.wallet.accounts;
  let active = accounts[0]?.chain;

  openModal(
    `<h2>导出私钥</h2>
     <div class="tabs" id="export-tabs" style="margin:12px 0">
       ${accounts
         .map(
           (account) =>
             `<button class="tab" type="button" data-chain="${account.chain}" aria-selected="${
               account.chain === active
             }"><span class="dot" style="--accent:${account.accent}"></span>${escapeHtml(
               account.symbol,
             )}</button>`,
         )
         .join("")}
     </div>
     <div id="export-body"><div class="skeleton" style="height:72px"></div></div>
     <div class="note note-danger" style="margin-top:12px">${icon("warn")}<div>不要截图，不要粘贴到网页或聊天窗口；使用完毕后请清理剪贴板。</div></div>
     <div class="modal-foot"><button class="btn btn-ghost" data-close type="button">关闭</button></div>`,
    {
      onMount(element) {
        const tabs = element.querySelector("#export-tabs");
        const body = element.querySelector("#export-body");
        let token = 0;
        const load = async (chain) => {
          const current = ++token;
          body.innerHTML = `<div class="skeleton" style="height:72px"></div>`;
          try {
            const exported = await api.exportPrivateKey(chain, password);
            if (current !== token) return;
            body.innerHTML = `
              <div class="kv"><span class="kv-key">格式</span><span class="kv-value">${escapeHtml(
                exported.format,
              )}</span></div>
              <div class="copy-field">
                <span class="value" data-copy-value>${escapeHtml(exported.value)}</span>
                <button class="btn btn-ghost btn-sm" data-copy type="button">${icon("copy")}复制</button>
              </div>
              <p class="hint" style="margin-top:8px">${escapeHtml(exported.warning)}</p>`;
            bindCopy(body, (ok) => toast(ok ? "私钥已复制" : "复制失败", ok ? "success" : "error"));
          } catch (error) {
            if (current !== token) return;
            body.innerHTML = `<div class="note note-danger">${icon("warn")}<div>${escapeHtml(
              error.message,
            )}</div></div>`;
          }
        };
        tabs.addEventListener("click", (event) => {
          const button = event.target.closest("[data-chain]");
          if (!button) return;
          active = button.dataset.chain;
          tabs
            .querySelectorAll("[data-chain]")
            .forEach((item) => item.setAttribute("aria-selected", String(item === button)));
          void load(active);
        });
        void load(active);
      },
    },
  );
}

function changePassword() {
  openModal(
    `<h2>修改密码</h2>
     <p class="hint" style="margin:6px 0 14px">修改后会用新密码重新加密钱包文件，其他内容不变。</p>
     <div class="stack-sm">
       <div class="field">
         <label for="current-password">当前密码</label>
         <input class="input" id="current-password" type="password" autocomplete="current-password" />
       </div>
       <div class="field">
         <label for="new-password">新密码</label>
         <input class="input" id="new-password" type="password" autocomplete="new-password" placeholder="至少 8 个字符" />
       </div>
       <div class="field">
         <label for="confirm-password">确认新密码</label>
         <input class="input" id="confirm-password" type="password" autocomplete="new-password" />
       </div>
       <p class="error-text" id="password-error" hidden></p>
     </div>
     <div class="modal-foot">
       <button class="btn btn-ghost" data-close type="button">取消</button>
       <button class="btn btn-primary" id="password-save" type="button">修改密码</button>
     </div>`,
    {
      onMount(element, close) {
        const error = element.querySelector("#password-error");
        const button = element.querySelector("#password-save");
        const fail = (message) => {
          error.hidden = false;
          error.textContent = message;
        };
        button.addEventListener("click", async () => {
          const current = element.querySelector("#current-password").value;
          const next = element.querySelector("#new-password").value;
          const confirm = element.querySelector("#confirm-password").value;
          if (!current) return fail("请输入当前密码");
          if (next.length < 8) return fail("新密码至少需要 8 个字符");
          if (next !== confirm) return fail("两次输入的新密码不一致");
          setBusy(button, true, "正在重新加密…");
          try {
            await api.changePassword(current, next);
            close();
            toast("密码已更新", "success");
          } catch (failure) {
            setBusy(button, false);
            fail(failure.message);
          }
        });
      },
    },
  );
}

async function erase(ctx) {
  const confirmed = await confirmDialog({
    title: "删除本地钱包",
    danger: true,
    confirmText: "继续",
    body: `<div class="note note-danger">${icon("warn")}<div>这会移除本机的加密钱包文件。<strong>如果助记词或口令没有备份，资产将无法找回。</strong></div></div>
      <p class="hint">建议先备份助记词（口令钱包请确认记得口令）再继续。</p>`,
  });
  if (!confirmed) return;
  const password = await askPassword({
    title: "输入密码确认删除",
    description: "验证密码后立即删除本机钱包文件。",
    confirmText: "删除",
  });
  if (!password) return;
  try {
    await api.deleteVault(password);
    ctx.setWallet(null);
    ctx.state.status = { exists: false, locked: true, hasHint: false, hint: null, label: null };
    ctx.go("onboarding");
    toast("本地钱包已删除", "success");
  } catch (error) {
    toast(error.message, "error");
  }
}
