/** Boot-time side effects ported from the old inline scripts in index.html:
 *  theme application and demo-mode console banner. */

import { api, hasTauri, isMock } from "./api";

export type ThemeName = "system" | "light" | "dark";

/** Apply the theme to <html data-theme>; system follows prefers-color-scheme. */
export function applyTheme(theme: ThemeName): void {
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const resolved = theme === "system" ? (media.matches ? "dark" : "light") : theme;
  const changed = document.documentElement.dataset.theme !== resolved;
  document.documentElement.dataset.theme = resolved;
  (document.documentElement as HTMLElement & { currentTheme?: ThemeName }).currentTheme = theme;
  // 二维码颜色是烤进 SVG 的，主题变了要通知页面重画（旧版 lycoris:theme）。
  if (changed) document.dispatchEvent(new CustomEvent("lycoris:theme"));
}

// Follow system theme changes while in "system" mode.
window
  .matchMedia("(prefers-color-scheme: dark)")
  .addEventListener?.("change", () => {
    const root = document.documentElement as HTMLElement & { currentTheme?: ThemeName };
    if (!root.currentTheme || root.currentTheme === "system") applyTheme("system");
  });

// 启动即应用一次（解锁前的界面也要有主题）；解锁后由 App 的 effect 按钱包设置覆盖。
applyTheme("system");

// Demo mode: make it obvious in the console as well as the chip in the UI.
if (isMock) {
  console.info(
    hasTauri
      ? "[lycoris] 演示模式（?mock=1）：不会触碰真实钱包文件"
      : "[lycoris] 浏览器直开：使用演示内核，地址与私钥均为假数据",
  );
}

// Warm the API bridge so the first paint has data sooner.
void api.vaultStatus().catch(() => {});
