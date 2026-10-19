/** Display helpers. All numbers arrive pre-formatted from Rust and are only
 *  grouped/trimmed here for presentation. */

const NBSP = "\u2009";

export function escapeHtml(value) {
  return String(value ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

export function shortAddress(address, head = 10, tail = 6) {
  const value = String(address ?? "");
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

export function groupDigits(value) {
  const text = String(value ?? "");
  const match = /^(-?)(\d+)(\.\d+)?$/.exec(text.trim());
  if (!match) return text;
  const [, sign, integer, fraction] = match;
  const grouped = integer.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return `${sign}${grouped}${fraction ?? ""}`;
}

export function formatFiat(value, currency = "usd") {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  const symbol = currency === "cny" ? "¥" : "$";
  const abs = Math.abs(value);
  const digits = abs >= 1000 ? 2 : abs >= 1 ? 2 : abs >= 0.01 ? 4 : 6;
  return `${symbol}${value.toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: digits,
  })}`;
}

export function formatTotal(value, currency = "usd") {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  const symbol = currency === "cny" ? "¥" : "$";
  return `${symbol}${value.toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

export function formatDateTime(timestamp) {
  if (!timestamp) return "—";
  const date = new Date(timestamp * 1000);
  const now = new Date();
  const sameDay = date.toDateString() === now.toDateString();
  const time = `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
  if (sameDay) return `今天 ${time}`;
  const yesterday = new Date(now.getTime() - 86_400_000);
  if (date.toDateString() === yesterday.toDateString()) return `昨天 ${time}`;
  const sameYear = date.getFullYear() === now.getFullYear();
  const month = date.getMonth() + 1;
  const day = date.getDate();
  return sameYear ? `${month}月${day}日 ${time}` : `${date.getFullYear()}年${month}月${day}日`;
}

export function relativeTime(timestamp) {
  if (!timestamp) return "—";
  const seconds = Math.max(0, Math.floor(Date.now() / 1000) - timestamp);
  if (seconds < 45) return "刚刚";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes} 分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours} 小时前`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days} 天前`;
  return formatDateTime(timestamp);
}

export async function copyText(value) {
  const text = String(value ?? "");
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    /* fall through to the legacy path */
  }
  try {
    const area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    const ok = document.execCommand("copy");
    document.body.removeChild(area);
    return ok;
  } catch {
    return false;
  }
}

/** "0.00000001" style trimming for text typed by the user. */
export function normalizeAmountInput(value) {
  let text = String(value ?? "").replace(/[^\d.]/g, "");
  const firstDot = text.indexOf(".");
  if (firstDot !== -1) {
    text = text.slice(0, firstDot + 1) + text.slice(firstDot + 1).replace(/\./g, "");
  }
  return text;
}

export function megabytesText(bytes) {
  if (!Number.isFinite(bytes)) return "—";
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export function thinSpace() {
  return NBSP;
}

export function chainAccent(chain) {
  const table = {
    bitcoin: "#F7931A",
    litecoin: "#345D9D",
    dogecoin: "#C2A633",
    bitcoincash: "#8DC351",
    ethereum: "#627EEA",
    "ethereum-classic": "#3AB83A",
    bsc: "#F3BA2F",
    polygon: "#8247E5",
    solana: "#14F195",
    monero: "#FF6600",
  };
  return table[chain] ?? "var(--ink-3)";
}
