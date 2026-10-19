/** Lock screen. Shows the vault hint, handles the cool-down after repeated
 *  wrong passwords, and explains that the file cannot be recovered. */

import { api } from "../api.js?v=21";
import { escapeHtml } from "../format.js?v=21";
import { confirmDialog, icon, setBusy, toast } from "../ui.js?v=21";

export function render(ctx) {
  const status = ctx.state.status ?? {};
  let countdownTimer = null;

  return {
    html: `<section class="onboarding"><div class="onboarding-inner" style="max-width:440px">
        <div class="brand-head">
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
          <h1>钱包已锁定</h1>
          <p>${status.label ? escapeHtml(status.label) : "输入本地密码解锁"}</p>
        </div>

        <div class="card card-pad stack">
          <div class="field">
            <label for="unlock-password">密码</label>
            <input class="input" id="unlock-password" type="password" autocomplete="current-password" placeholder="输入密码" autofocus />
          </div>
          ${
            status.hasHint
              ? `<div class="note">${icon("info")}<div>密码提示：<strong>${escapeHtml(
                  status.hint ?? "",
                )}</strong></div></div>`
              : ""
          }
          <p class="error-text" id="unlock-error" hidden></p>
          <button class="btn btn-primary btn-block" id="unlock" type="button">解锁</button>
          <p class="hint">密码只用于解密本机钱包文件，忘记密码无法找回，只能删除钱包后重新导入助记词或重建口令钱包。</p>
          <button class="btn btn-ghost btn-block" id="forgot-password" type="button">忘记密码？删除本机钱包文件</button>
        </div>
      </div></section>`,

    mount(root) {
      const input = root.querySelector("#unlock-password");
      const error = root.querySelector("#unlock-error");
      const button = root.querySelector("#unlock");
      const forgot = root.querySelector("#forgot-password");

      const unlock = async () => {
        const password = input.value;
        if (!password) {
          error.hidden = false;
          error.textContent = "请输入密码";
          return;
        }
        setBusy(button, true, "解锁中…");
        try {
          const wallet = await api.unlockVault(password);
          ctx.setWallet(wallet);
          ctx.go("dashboard");
        } catch (failure) {
          setBusy(button, false);
          error.hidden = false;
          error.textContent = failure.message;
          input.setAttribute("aria-invalid", "true");
          if (failure.code === "TOO_MANY_ATTEMPTS") startCountdown(failure.message);
        }
      };

      const startCountdown = (message) => {
        const match = /(\d+)\s*秒/.exec(message) ?? /(\d+)/.exec(message);
        let seconds = match ? Number(match[1]) : 30;
        clearInterval(countdownTimer);
        input.disabled = true;
        button.disabled = true;
        const tick = () => {
          error.hidden = false;
          error.textContent = `尝试次数过多，请等待 ${seconds} 秒`;
          seconds -= 1;
          if (seconds < 0) {
            clearInterval(countdownTimer);
            input.disabled = false;
            button.disabled = false;
            error.hidden = true;
          }
        };
        tick();
        countdownTimer = setInterval(tick, 1000);
      };

      button.addEventListener("click", unlock);
      input.addEventListener("keydown", (event) => {
        if (event.key === "Enter") unlock();
      });
      input.addEventListener("input", () => input.removeAttribute("aria-invalid"));
      setTimeout(() => input.focus(), 40);

      /* 忘记密码：文件无法解密，只能删除后重建。删除前必须讲清后果。 */
      forgot.addEventListener("click", async () => {
        const confirmed = await confirmDialog({
          title: "删除本机钱包文件？",
          body: `<p>此操作会<strong>永久删除</strong>本机的钱包文件，无法撤销，也无法再找回其中的助记词与口令。</p>
                 <p>如果这个钱包已经在链上转入过资金，请先记住助记词或口令，用另一个钱包恢复资产，再来这里删除。</p>
                 <p>删除后会回到创建界面，你可以重新导入助记词或重建口令钱包。</p>`,
          confirmText: "永久删除",
          cancelText: "我再想想",
          danger: true,
        });
        if (!confirmed) return;
        setBusy(forgot, true, "删除中…");
        try {
          await api.resetVault();
          toast("本机钱包文件已删除", "info");
          ctx.setWallet(null);
          ctx.refreshStatus?.();
          ctx.go("onboarding");
        } catch (failure) {
          setBusy(forgot, false);
          toast(failure.message, "error");
        }
      });

      return () => clearInterval(countdownTimer);
    },
  };
}
