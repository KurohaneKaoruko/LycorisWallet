/** Offline unit tests for the explorer URL builders and the detail-panel
 *  data assembly — run with `npm test` (Node's built-in test runner with
 *  native TS stripping, no extra dependencies). */

import assert from "node:assert/strict";
import { test } from "node:test";

import {
  EXPLORER_ADDRESS,
  EXPLORER_TX,
  activityDetailLines,
  activityExplorerUrl,
  explorerAddressUrl,
  explorerTxUrl,
} from "./explorers.ts";
import type { ChainId } from "./types";

const ALL_CHAINS = Object.keys(EXPLORER_TX) as ChainId[];

test("30 条链全部配置了交易页与地址页模板", () => {
  assert.equal(ALL_CHAINS.length, 30, "exactly the 30 supported chains");
  const addressChains = Object.keys(EXPLORER_ADDRESS) as ChainId[];
  assert.deepEqual(addressChains.sort(), [...ALL_CHAINS].sort());
  for (const chain of ALL_CHAINS) {
    assert.match(EXPLORER_TX[chain], /^https:\/\//, `${chain} tx prefix is an https URL`);
    assert.match(EXPLORER_ADDRESS[chain], /^https:\/\//, `${chain} address prefix is an https URL`);
  }
});

test("主流链的交易页/地址页 URL 拼接格式", () => {
  assert.equal(
    explorerTxUrl("bitcoin", "abc123"),
    "https://mempool.space/tx/abc123",
  );
  assert.equal(
    explorerAddressUrl("bitcoin", "bc1qxyz"),
    "https://mempool.space/address/bc1qxyz",
  );
  assert.equal(explorerTxUrl("ethereum", "0xdead"), "https://etherscan.io/tx/0xdead");
  assert.equal(explorerTxUrl("bsc", "0xbeef"), "https://bscscan.com/tx/0xbeef");
  assert.equal(explorerTxUrl("polygon", "0x01"), "https://polygonscan.com/tx/0x01");
  assert.equal(explorerTxUrl("solana", "SIG123"), "https://solscan.io/tx/SIG123");
  assert.equal(
    explorerAddressUrl("solana", "ACC456"),
    "https://solscan.io/account/ACC456",
    "Solana 地址页用 /account/",
  );
  assert.equal(explorerTxUrl("litecoin", "ltc1"), "https://litecoinspace.org/tx/ltc1");
});

test("特殊格式链:Cryptoid 查询串、Monero 搜索页、单页式浏览器", () => {
  assert.equal(
    explorerTxUrl("litecoincash", "h1"),
    "https://chainz.cryptoid.info/lcc/tx.dws?h=h1",
    "Cryptoid 交易页是 ?h= 查询串",
  );
  assert.equal(
    explorerAddressUrl("potcoin", "p1"),
    "https://chainz.cryptoid.info/pot/address.dws?u=p1",
    "Cryptoid 地址页是 ?u= 查询串",
  );
  assert.equal(
    explorerAddressUrl("monero", "4Ax1"),
    "https://xmrchain.net/search?value=4Ax1",
    "Monero 浏览器只有统一搜索端点",
  );
  assert.equal(
    explorerTxUrl("egulden", "e1"),
    "https://bitinfocharts.com/e-gulden/explorer/e1",
    "e-Gulden 单页式,地址前缀与交易页相同",
  );
  assert.equal(
    explorerTxUrl("zephyr", "z1"),
    "https://explorer.zephyrprotocol.com/tx/z1",
  );
  assert.equal(
    explorerAddressUrl("zephyr", "z1"),
    "https://explorer.zephyrprotocol.com/z1",
    "Zephyr 地址页前缀是站点根路径",
  );
});

test("空 txid/地址返回空串,UI 据此隐藏按钮而不是拼半截链接", () => {
  assert.equal(explorerTxUrl("bitcoin", ""), "");
  assert.equal(explorerAddressUrl("ethereum", ""), "");
});

test("activityDetailLines:确认的发送交易,金额/费率/地址/哈希齐备", () => {
  const lines = activityDetailLines({
    chain: "bitcoin",
    direction: "send",
    amount: "0.01",
    symbol: "BTC",
    counterparty: "bc1qexample",
    txid: "deadbeef",
    status: "confirmed",
    fee: "0.0001",
    timestamp: 1_700_000_000,
  });
  const byKey = new Map(lines.map((line) => [line.key, line.value]));
  assert.equal(byKey.get("金额"), "−0.01 BTC");
  assert.equal(byKey.get("手续费"), "0.0001 BTC");
  assert.equal(byKey.get("收款地址"), "bc1qexample");
  assert.equal(byKey.get("交易哈希"), "deadbeef");
  const addrLine = lines.find((line) => line.key === "收款地址");
  assert.equal(addrLine?.copy, true, "地址行可复制");
  assert.equal(
    addrLine?.link,
    "https://mempool.space/address/bc1qexample",
    "地址行带浏览器链接",
  );
  const txLine = lines.find((line) => line.key === "交易哈希");
  assert.equal(txLine?.link, "https://mempool.space/tx/deadbeef");
});

test("activityDetailLines:接收交易无手续费行,对手方标注为来源地址", () => {
  const lines = activityDetailLines({
    chain: "ethereum",
    direction: "receive",
    amount: "1.5",
    symbol: "ETH",
    counterparty: "0xabc",
    txid: "0x01",
    status: "confirmed",
    timestamp: 1_700_000_000,
  });
  assert.equal(lines.some((line) => line.key === "手续费"), false);
  assert.equal(lines.some((line) => line.key === "收款地址"), false);
  assert.equal(lines.some((line) => line.key === "来源地址"), true);
  assert.equal(lines.find((line) => line.key === "金额")?.value, "+1.5 ETH");
});

test("activityDetailLines:确认数随回写数据出现,待确认显示 0", () => {
  const base = {
    chain: "bitcoin" as ChainId,
    direction: "receive" as const,
    amount: "1",
    symbol: "BTC",
    counterparty: "bc1qx",
    txid: "t1",
    status: "confirmed",
    timestamp: 1_700_000_000,
  };
  assert.equal(
    activityDetailLines(base).some((line) => line.key === "确认数"),
    false,
    "没有回写数据时不显示确认数行",
  );
  const confirmedLines = activityDetailLines(base, 12);
  assert.equal(confirmedLines.find((line) => line.key === "确认数")?.value, "12");
  const pendingLines = activityDetailLines({ ...base, status: "pending" }, 0);
  assert.equal(
    pendingLines.find((line) => line.key === "确认数")?.value,
    "0（待确认）",
  );
});

test("activityExplorerUrl:条目自带链接优先,否则按模板回退", () => {
  assert.equal(
    activityExplorerUrl({
      chain: "dogecoin",
      direction: "receive",
      amount: "1",
      symbol: "DOGE",
      counterparty: "D8x",
      txid: "dt",
      status: "confirmed",
      timestamp: 1,
      explorerUrl: "https://doge.trezor.io/tx/dt",
    }),
    "https://doge.trezor.io/tx/dt",
  );
  assert.equal(
    activityExplorerUrl({
      chain: "dogecoin",
      direction: "receive",
      amount: "1",
      symbol: "DOGE",
      counterparty: "D8x",
      txid: "dt",
      status: "confirmed",
      timestamp: 1,
    }),
    "https://doge.trezor.io/tx/dt",
    "无自带链接时按本地模板生成,结果一致",
  );
});
