/** Chain explorer URL templates and the pure helpers that build links from
 *  them — the TS mirror of `ChainInfo.explorer_tx` / `explorer_address` in
 *  `crates/core/src/chains.rs`. Data mapping only; no network, no side
 *  effects, so every function here is offline-testable.
 *
 *  `ActivityItem.explorerUrl` (populated by the Rust side) always wins when
 *  present: the Rust template is the single source of truth, and this table
 *  is the fallback for items stored before the field existed, plus the
 *  address-page links the item itself never carries. */

import { formatDateTime } from "./format.ts";
import type { ChainId } from "./types";

/** Per-chain transaction-page URL prefix; `${prefix}${txid}` is the link.
 *  Hosts verified live 2026-11: the trezor/zelcore Blockbook fleets and
 *  cryptoid.info shut down, so DOGE/BCH/DASH/ZEC/FIRO now point at
 *  Blockchair and the chainz five at their surviving (or generic) pages. */
export const EXPLORER_TX: Record<ChainId, string> = {
  bitcoin: "https://mempool.space/tx/",
  litecoin: "https://litecoinspace.org/tx/",
  dogecoin: "https://blockchair.com/dogecoin/transaction/",
  bitcoincash: "https://blockchair.com/bitcoin-cash/transaction/",
  ethereum: "https://etherscan.io/tx/",
  "ethereum-classic": "https://blockscout.com/etc/mainnet/tx/",
  ethw: "https://www.oklink.com/ethw/tx/",
  bsc: "https://bscscan.com/tx/",
  polygon: "https://polygonscan.com/tx/",
  solana: "https://solscan.io/tx/",
  monero: "https://xmrchain.net/tx/",
  bitcoinsv: "https://whatsonchain.com/tx/",
  namecoin: "https://namecha.in/tx/",
  litecoincash: "https://bitinfocharts.com/litecoincash/explorer/",
  feathercoin: "https://explorer.feathercoin.com/tx/",
  viacoin: "https://blockbook.viacoin.org/tx/",
  potcoin: "https://bitinfocharts.com/potcoin/explorer/",
  egulden: "https://bitinfocharts.com/e-gulden/explorer/",
  dash: "https://insight.dash.org/tx/",
  groestlcoin: "https://blockbook.groestlcoin.org/tx/",
  vertcoin: "https://bitinfocharts.com/vertcoin/explorer/",
  ravencoin: "https://blockbook.ravencoin.org/tx/",
  monacoin: "https://blockbook.electrum-mona.org/tx/",
  fujicoin: "https://explorer.fujicoin.org/tx/",
  zcash: "https://blockchair.com/zcash/transaction/",
  firo: "https://explorer.firo.org/tx/",
  neurai: "https://blockbook.neurai.org/tx/",
  clore: "https://blockchair.com/clore/transaction/",
  zephyr: "https://explorer.zephyrprotocol.com/tx/",
  octaspace: "https://explorer.octa.space/tx/",
};

/** Per-chain address-page URL prefix; `${prefix}${address}` is the link.
 *  Special cases, mirrored from Rust: Solana uses `/account/`, Monero's
 *  explorer has a single search endpoint, and e-Gulden / Vertcoin / Zephyr
 *  only have one page style (same prefix as the tx page). */
export const EXPLORER_ADDRESS: Record<ChainId, string> = {
  bitcoin: "https://mempool.space/address/",
  litecoin: "https://litecoinspace.org/address/",
  dogecoin: "https://blockchair.com/dogecoin/address/",
  bitcoincash: "https://blockchair.com/bitcoin-cash/address/",
  ethereum: "https://etherscan.io/address/",
  "ethereum-classic": "https://blockscout.com/etc/mainnet/address/",
  ethw: "https://www.oklink.com/ethw/address/",
  bsc: "https://bscscan.com/address/",
  polygon: "https://polygonscan.com/address/",
  solana: "https://solscan.io/account/",
  monero: "https://xmrchain.net/search?value=",
  bitcoinsv: "https://whatsonchain.com/address/",
  namecoin: "https://namecha.in/address/",
  litecoincash: "https://bitinfocharts.com/litecoincash/explorer/",
  feathercoin: "https://explorer.feathercoin.com/address/",
  viacoin: "https://blockbook.viacoin.org/address/",
  potcoin: "https://bitinfocharts.com/potcoin/explorer/",
  egulden: "https://bitinfocharts.com/e-gulden/explorer/",
  dash: "https://insight.dash.org/address/",
  groestlcoin: "https://blockbook.groestlcoin.org/address/",
  vertcoin: "https://bitinfocharts.com/vertcoin/explorer/",
  ravencoin: "https://blockbook.ravencoin.org/address/",
  monacoin: "https://blockbook.electrum-mona.org/address/",
  fujicoin: "https://explorer.fujicoin.org/address/",
  zcash: "https://blockchair.com/zcash/address/",
  firo: "https://explorer.firo.org/address/",
  neurai: "https://blockbook.neurai.org/address/",
  clore: "https://blockchair.com/clore/address/",
  zephyr: "https://explorer.zephyrprotocol.com/",
  octaspace: "https://explorer.octa.space/address/",
};

/** Transaction-page link for a chain. Empty txid → empty string (the UI
 *  hides the button instead of opening a half-built URL). */
export function explorerTxUrl(chain: ChainId, txid: string): string {
  const prefix = EXPLORER_TX[chain];
  if (!prefix || !txid) return "";
  return `${prefix}${txid}`;
}

/** Address-page link for a chain. Empty address → empty string. */
export function explorerAddressUrl(chain: ChainId, address: string): string {
  const prefix = EXPLORER_ADDRESS[chain];
  if (!prefix || !address) return "";
  return `${prefix}${address}`;
}

/** Minimal shape of an activity entry the detail assembler needs — a subset
 *  of `ActivityItem` plus the runtime-only `explorerUrl`/`fee` fields the
 *  Rust payload may carry. */
export interface ActivityDetailInput {
  chain: ChainId;
  direction: "send" | "receive";
  amount: string;
  symbol: string;
  counterparty: string;
  txid: string;
  status: string;
  fee?: string;
  timestamp: number;
  explorerUrl?: string;
}

/** One key/value row of the detail panel. */
export interface DetailLine {
  key: string;
  value: string;
  /** Show a copy affordance for this row (addresses, hashes). */
  copy?: boolean;
  /** Render the value as an external link with this label. */
  link?: string;
}

/** Assemble the detail panel's data rows from an activity entry — pure, so
 *  the UI component just renders whatever comes out. Confirmation count is
 *  taken from the sync report when the caller has fresher data (the stored
 *  row itself only carries a coarse status). Direction decides whether the
 *  counterparty is labelled 收款地址 or 来源地址. */
export function activityDetailLines(
  item: ActivityDetailInput,
  confirmations?: number,
): DetailLine[] {
  const send = item.direction === "send";
  const lines: DetailLine[] = [];
  if (item.status === "pending") {
    lines.push({ key: "确认数", value: "0（待确认）" });
  } else if (typeof confirmations === "number" && confirmations > 0) {
    lines.push({ key: "确认数", value: `${confirmations}` });
  }
  lines.push(
    { key: "金额", value: `${send ? "−" : "+"}${item.amount} ${item.symbol}` },
  );
  if (item.fee) {
    lines.push({ key: "手续费", value: `${item.fee} ${item.symbol}` });
  }
  lines.push({ key: "时间", value: formatDateTime(item.timestamp) });
  lines.push({
    key: send ? "收款地址" : "来源地址",
    value: item.counterparty,
    copy: true,
    link: explorerAddressUrl(item.chain, item.counterparty) || undefined,
  });
  lines.push({
    key: "交易哈希",
    value: item.txid,
    copy: true,
    link: explorerTxUrl(item.chain, item.txid) || undefined,
  });
  return lines;
}

/** Explorer link the detail panel opens — the stored per-item URL wins
 *  (it comes from the Rust template), the local table is the fallback. */
export function activityExplorerUrl(item: ActivityDetailInput): string {
  if (item.explorerUrl?.startsWith("http")) return item.explorerUrl;
  return explorerTxUrl(item.chain, item.txid);
}
