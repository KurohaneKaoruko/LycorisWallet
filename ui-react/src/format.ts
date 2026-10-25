/** Display helpers. All numbers arrive pre-formatted from Rust and are only
 *  grouped/trimmed here for presentation. Ported verbatim from ui/js/format.js
 *  minus escapeHtml (React escapes by default). */

const NBSP = "\u2009";

export function shortAddress(address: string | null | undefined, head = 10, tail = 6): string {
  const value = String(address ?? "");
  if (value.length <= head + tail + 1) return value;
  return `${value.slice(0, head)}…${value.slice(-tail)}`;
}

export function groupDigits(value: string | null | undefined): string {
  const text = String(value ?? "");
  const match = /^(-?)(\d+)(\.\d+)?$/.exec(text.trim());
  if (!match) return text;
  const [, sign, integer, fraction] = match;
  const grouped = integer.replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  return `${sign}${grouped}${fraction ?? ""}`;
}

export function formatFiat(value: number | null | undefined, currency: string = "usd"): string {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  const symbol = currency === "cny" ? "¥" : "$";
  const abs = Math.abs(value);
  const digits = abs >= 1000 ? 2 : abs >= 1 ? 2 : abs >= 0.01 ? 4 : 6;
  return `${symbol}${value.toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: digits,
  })}`;
}

export function formatTotal(value: number | null | undefined, currency: string = "usd"): string {
  if (value === null || value === undefined || Number.isNaN(value)) return "—";
  const symbol = currency === "cny" ? "¥" : "$";
  return `${symbol}${value.toLocaleString("zh-CN", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  })}`;
}

export function formatDateTime(timestamp: number | null | undefined): string {
  if (!timestamp) return "—";
  const date = new Date(timestamp * 1000);
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(
    date.getHours(),
  )}:${pad(date.getMinutes())}`;
}

export function formatRelative(timestamp: number | null | undefined): string {
  if (!timestamp) return "—";
  const diff = Date.now() / 1000 - timestamp;
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)} 天前`;
  return formatDateTime(timestamp);
}

export { NBSP };
