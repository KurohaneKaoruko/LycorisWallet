/** Shared UI primitives: icons, toasts, modals, password prompts. */

import { copyText, escapeHtml } from "./format.js?v=7";

const ICONS = {
  dashboard:
    '<path d="M4 13.2 12 5l8 8.2"/><path d="M6.2 11.5V19h11.6v-7.5"/>',
  receive:
    '<path d="M12 4v12"/><path d="m7.5 12 4.5 4.5L16.5 12"/><path d="M4.5 19.5h15"/>',
  send: '<path d="M12 19.5V6.5"/><path d="m6.5 12 5.5-5.5L17.5 12"/>',
  activity:
    '<path d="M3.6 12a8.4 8.4 0 1 0 2.6-6.1"/><path d="M3.5 4.6V9h4.4"/><path d="M12 8.4V12l2.8 1.8"/>',
  contacts:
    '<circle cx="9" cy="8.4" r="3.2"/><path d="M3.4 19.2c.7-3 2.9-4.6 5.6-4.6s4.9 1.6 5.6 4.6"/><path d="M16.4 7.2a2.6 2.6 0 0 1 0 5.1"/><path d="M18 19.2c-.2-1.5-.7-2.8-1.5-3.8"/>',
  settings:
    '<circle cx="12" cy="12" r="3"/><path d="M12 3.6v2.2M12 18.2v2.2M4.9 7.8l1.9 1.1M17.2 15.1l1.9 1.1M4.9 16.2l1.9-1.1M17.2 8.9l1.9-1.1"/>',
  copy: '<rect x="9" y="9" width="10.5" height="10.5" rx="2"/><path d="M15 6.5V5.6c0-.9-.7-1.6-1.6-1.6H5.6C4.7 4 4 4.7 4 5.6v7.8c0 .9.7 1.6 1.6 1.6h.9"/>',
  external:
    '<path d="M14 4h6v6"/><path d="M20 4l-9.2 9.2"/><path d="M18 14.4V18a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h3.6"/>',
  check: '<path d="m5 12.6 4.6 4.4L19 7"/>',
  warn: '<path d="M12 4.8 3.4 19.2h17.2L12 4.8Z"/><path d="M12 10v4"/><path d="M12 16.6h.01"/>',
  info: '<circle cx="12" cy="12" r="8.4"/><path d="M12 11v5.4"/><path d="M12 7.8h.01"/>',
  trash:
    '<path d="M4.8 7.4h14.4"/><path d="M9.4 7.4V5.8c0-.8.6-1.4 1.4-1.4h2.4c.8 0 1.4.6 1.4 1.4v1.6"/><path d="M6.6 7.4 7.4 19c0 .8.7 1.4 1.5 1.4h6.2c.8 0 1.5-.6 1.5-1.4l.8-11.6"/>',
  edit: '<path d="M4.5 19.5h4l9.4-9.4-4-4-9.4 9.4v4Z"/><path d="m14.6 5.4 4 4"/>',
  lock:
    '<path d="M7.4 10.6V8.8a4.6 4.6 0 1 1 9.2 0v1.8"/><rect x="5.2" y="10.6" width="13.6" height="9.4" rx="2.4"/>',
  key: '<circle cx="8" cy="15.4" r="3.4"/><path d="m10.6 13 8-8"/><path d="m16 7.6 2 2"/><path d="m14 9.6 2 2"/>',
  eye: '<path d="M2.8 12S6.4 6.4 12 6.4 21.2 12 21.2 12 17.6 17.6 12 17.6 2.8 12 2.8 12Z"/><circle cx="12" cy="12" r="2.8"/>',
  eyeOff:
    '<path d="M4 4.4 20 20.4"/><path d="M9.4 9.6A2.8 2.8 0 0 0 12 14.8"/><path d="M6.3 6.9C4.2 8.3 2.8 12 2.8 12s3.6 5.6 9.2 5.6c1.6 0 3-.4 4.2-1"/><path d="M18.6 15.1c1.6-1.4 2.6-3.1 2.6-3.1S17.6 6.4 12 6.4c-.8 0-1.6.1-2.3.3"/>',
  refresh:
    '<path d="M19.6 12a7.6 7.6 0 1 1-2.3-5.4"/><path d="M19.8 4.6v4.2h-4.2"/>',
  qr: '<rect x="4" y="4" width="6" height="6" rx="1"/><rect x="14" y="4" width="6" height="6" rx="1"/><rect x="4" y="14" width="6" height="6" rx="1"/><path d="M14 14h2.4v2.4H14zM18.4 18.4H20V20h-4v-2.4"/><path d="M14 20h1.4M20 14h-2.4"/>',
  wallet:
    '<rect x="3.4" y="6.4" width="17.2" height="12.6" rx="2.6"/><path d="M3.4 10.4h17.2"/><path d="M16.4 14.6h1.6"/>',
  arrowUpRight: '<path d="M7 17 17 7"/><path d="M9.4 7H17v7.6"/>',
  shield:
    '<path d="M12 3.6 5.4 6v5.6c0 4 2.8 7.4 6.6 8.8 3.8-1.4 6.6-4.8 6.6-8.8V6L12 3.6Z"/><path d="m9.2 12 2 2 3.6-3.8"/>',
  plus: '<path d="M12 5.4v13.2M5.4 12h13.2"/>',
  search: '<circle cx="11" cy="11" r="6.2"/><path d="m15.6 15.6 3.8 3.8"/>',
  chevronLeft: '<path d="M14.5 6.5 9 12l5.5 5.5"/>',
  chevronRight: '<path d="M9.5 6.5 15 12l-5.5 5.5"/>',
  menu: '<path d="M4.5 7h15M4.5 12h15M4.5 17h15"/>',
  clock: '<circle cx="12" cy="12" r="8.4"/><path d="M12 7.8V12l3 1.8"/>',
  sparkle:
    '<path d="M12 4.4l1.7 4.6 4.6 1.7-4.6 1.7L12 17l-1.7-4.6L5.7 10.7l4.6-1.7L12 4.4Z"/><path d="M18.6 16.4l.7 1.8 1.8.7-1.8.7-.7 1.8-.7-1.8-1.8-.7 1.8-.7.7-1.8Z"/>',
};

export function icon(name, className = "") {
  const body = ICONS[name] ?? ICONS.info;
  return `<svg class="${className}" viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round">${body}</svg>`;
}

/* -------------------------------------------------------------------- toasts */

export function toast(message, kind = "info", timeout = 3600) {
  const root = document.getElementById("toasts");
  if (!root) return;
  const name = kind === "error" ? "warn" : kind === "success" ? "check" : "info";
  const flavor = kind === "success" ? "ok" : kind;
  const element = document.createElement("div");
  element.className = `toast toast-${flavor}`;
  element.innerHTML = `${icon(name)}<div>${escapeHtml(message)}</div>`;
  root.appendChild(element);
  const remove = () => {
    element.classList.add("out");
    setTimeout(() => element.remove(), 200);
  };
  const timer = setTimeout(remove, timeout);
  element.addEventListener("click", () => {
    clearTimeout(timer);
    remove();
  });
}

/* -------------------------------------------------------------------- modals */

export function openModal(html, { onMount, dismissable = true } = {}) {
  const root = document.getElementById("modal-root");
  const backdrop = document.createElement("div");
  backdrop.className = "modal-backdrop";
  backdrop.innerHTML = `<div class="modal" role="dialog" aria-modal="true">${html}</div>`;
  root.appendChild(backdrop);

  const close = (value) => {
    backdrop.remove();
    document.removeEventListener("keydown", onKey);
    resolveClose?.(value);
  };
  let resolveClose = null;
  const onKey = (event) => {
    if (event.key === "Escape" && dismissable) close(null);
  };
  document.addEventListener("keydown", onKey);
  if (dismissable) {
    backdrop.addEventListener("mousedown", (event) => {
      if (event.target === backdrop) close(null);
    });
  }
  backdrop.querySelectorAll("[data-close]").forEach((button) => {
    button.addEventListener("click", () => close(null));
  });
  onMount?.(backdrop.querySelector(".modal"), close);
  const firstInput = backdrop.querySelector("input, textarea, button.btn-primary");
  firstInput?.focus();
  return { element: backdrop.querySelector(".modal"), close };
}

export function confirmDialog({
  title,
  body,
  confirmText = "确认",
  cancelText = "取消",
  danger = false,
}) {
  return new Promise((resolve) => {
    const modal = openModal(
      `<h2>${escapeHtml(title)}</h2>
       <div class="stack-sm" style="margin-top:10px">${body}</div>
       <div class="modal-foot">
         <button class="btn btn-ghost" data-cancel type="button">${escapeHtml(cancelText)}</button>
         <button class="btn ${danger ? "btn-danger" : "btn-primary"}" data-confirm type="button">${escapeHtml(confirmText)}</button>
       </div>`,
      {
        onMount(element, close) {
          element.querySelector("[data-cancel]").addEventListener("click", () => {
            close();
            resolve(false);
          });
          element.querySelector("[data-confirm]").addEventListener("click", () => {
            close();
            resolve(true);
          });
        },
      },
    );
    modal.element.addEventListener("click", (event) => {
      if (event.target.closest("[data-close]")) resolve(false);
    });
    // Resolve `false` when the dialog is dismissed with Escape or the backdrop.
    const observer = new MutationObserver(() => {
      if (!document.body.contains(modal.element)) {
        observer.disconnect();
        resolve(false);
      }
    });
    observer.observe(document.getElementById("modal-root"), { childList: true });
  });
}

/** Ask for the vault password with a small modal. Resolves `null` on cancel. */
export function askPassword({ title, description, confirmText = "确认" }) {
  return new Promise((resolve) => {
    let settled = false;
    const modal = openModal(
      `<h2>${escapeHtml(title)}</h2>
       <p class="hint" style="margin:8px 0 14px">${description ?? ""}</p>
       <div class="field">
         <label for="ask-password">钱包密码</label>
         <input class="input" id="ask-password" type="password" autocomplete="current-password" placeholder="输入密码以继续" />
       </div>
       <p class="error-text" id="ask-error" hidden></p>
       <div class="modal-foot">
         <button class="btn btn-ghost" data-close type="button">取消</button>
         <button class="btn btn-primary" id="ask-confirm" type="button">${escapeHtml(confirmText)}</button>
       </div>`,
      {
        onMount(element, close) {
          const input = element.querySelector("#ask-password");
          const error = element.querySelector("#ask-error");
          const confirm = element.querySelector("#ask-confirm");
          const submit = () => {
            const value = input.value;
            if (!value) {
              error.hidden = false;
              error.textContent = "请输入密码";
              return;
            }
            settled = true;
            close();
            resolve(value);
          };
          confirm.addEventListener("click", submit);
          input.addEventListener("keydown", (event) => {
            if (event.key === "Enter") submit();
          });
          const observer = new MutationObserver(() => {
            if (!document.body.contains(element)) {
              observer.disconnect();
              if (!settled) resolve(null);
            }
          });
          observer.observe(document.getElementById("modal-root"), { childList: true });
        },
      },
    );
    return modal;
  });
}

/* -------------------------------------------------------------- copy widgets */

export function copyField(value, { label = "", html = false } = {}) {
  const text = String(value ?? "");
  return `<div class="copy-field">
      <span class="value" data-copy-value>${html ? text : escapeHtml(text)}</span>
      <button class="btn btn-ghost btn-sm" type="button" data-copy title="复制" aria-label="复制">${icon("copy")}</button>
    </div>
    ${label ? `<p class="hint">${escapeHtml(label)}</p>` : ""}`;
}

export function bindCopy(root, onCopied) {
  root.querySelectorAll("[data-copy]").forEach((button) => {
    button.addEventListener("click", async () => {
      const field = button.closest(".copy-field");
      const value = field?.querySelector("[data-copy-value]")?.textContent ?? "";
      const ok = await copyText(value);
      onCopied?.(ok, value);
    });
  });
}

export function setBusy(button, busy, busyText = "处理中…") {
  if (!button) return;
  if (busy) {
    button.dataset.originalText = button.innerHTML;
    button.disabled = true;
    button.innerHTML = `<span class="spinner"></span>${escapeHtml(busyText)}`;
  } else {
    button.disabled = false;
    if (button.dataset.originalText) {
      button.innerHTML = button.dataset.originalText;
      delete button.dataset.originalText;
    }
  }
}

export function debounce(fn, delay = 320) {
  let timer = null;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), delay);
  };
}

/**
 * Polish a horizontally scrollable chip row: the native scrollbar is hidden
 * (it is ugly on Windows) and the overflow is hinted with a soft fade on
 * whichever side still has content.
 */
export function enhanceTabScroll(container) {
  if (!container) return () => {};
  const update = () => {
    const overflow = container.scrollWidth - container.clientWidth;
    const left = container.scrollLeft;
    container.classList.toggle("has-left", left > 4);
    container.classList.toggle("has-right", overflow > 4 && left < overflow - 4);
  };
  container.addEventListener("scroll", update, { passive: true });
  const observer = new ResizeObserver(update);
  observer.observe(container);
  update();
  return () => {
    container.removeEventListener("scroll", update);
    observer.disconnect();
  };
}
