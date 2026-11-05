# 更新日志

## 未发布

### 新增：交易详情面板与全链区块浏览器链接

- **详情面板补全**：活动列表点开的交易详情现在显示确认数（来自同步报告
  回写，如「· 12 次确认」）、手续费、金额、时间、对手方地址与交易哈希，
  并新增「在区块浏览器查看该地址」入口。
- **浏览器链接覆盖全部 30 条链**：此前硬编码只有 6 条链的交易页前缀，且
  无地址页；现在前端建立与 Rust `ChainInfo.explorer_tx` / `explorer_address`
  一致的模板表（交易页 + 地址页），条目自带的 `explorerUrl` 仍优先。
  Cryptoid 查询串（`?h=`/`?u=`）、Monero 统一搜索页、Solana `/account/`、
  Zephyr 站点根路径等特殊格式均已按链配置。
- **确认数数据通道**：`refresh_activity` 同步报告新增 `statusUpdates`
  （chain/txid/status/confirmations），把 merge 时已算出的 pending →
  confirmed 转变与区块确认数带给 UI，活动页原地更新，详情面板即时可查；
  无需再等整页重载。
- **类型修正**：前端 `ActivityItem` 补上运行时本就存在的 `fee` 与
  `explorerUrl` 可选字段，移除从未存在的 `note`。
- **测试**：新增 `ui-react/src/explorers.test.ts`（Node 内建 test runner +
  原生 TS，8 例）：30 条链模板完整性、主流链/特殊格式链接拼接、空值防护、
  详情行组装（发送/接收/确认数回写/链接回退）。Rust 侧 merge 测试补
  `status_updates` 断言。

### 变更：口令哈希改用各链地址管线真实哈希

- **原则**：口令哈希 = 该链「私钥 → 地址」过程中真实使用的哈希。
- **修正映射**：Litecoin/Dogecoin/Feathercoin/Viacoin/Potcoin/e-Gulden 的口令哈希
  从旧实现改为 **SHA-256**；Zephyr 改为 **Keccak-256**——
  它是 CryptoNote 地址；Groestlcoin 从 SHA-256 改为 **Groestl-512**（取前 32 字节）——
  其地址管线把所有 SHA-256 替换为 Groestl-512（官方「Groestl-hash160」）。
- **枚举收敛**：`HashAlgorithm` 从 9 个变体缩到 3 个（`Sha256`/`Keccak256`/
  `Groestl512`），各链只保留地址管线真实哈希。
- **破坏性**：旧版生成的口令钱包地址在升级后不再有效（哈希变了），助记词钱包
  不受影响。受影响链请用助记词模式或重新记一个口令。

### 新增：U盘冷钱包

- **保险柜上U盘**：设置 → U盘冷钱包 → 启用。验密码后把本机加密保险柜
  `vault.json` 复制到可移动磁盘（`<U盘>\LycorisWallet\vault.json`）并改从
  U盘加载；密钥材料从此只随U盘走。本机默认保留副本，可手动删除。
- **拔盘即锁**：后台每 2 秒监测冷盘在位状态，拔出U盘立即锁定会话并清空
  内存密钥，前端 toast 提示原因；所有写入/解锁路径都会先校验冷盘在位，
  拔盘后任何操作返回 `USB_DRIVE_MISSING` 而不是静默回落到本机副本。
- **换机恢复**：本机没有保险柜时，解锁页出现「从U盘冷钱包恢复」入口；
  验密码后把U盘上的保险柜拉回本机。恢复动作同样要求密码，防止捡到U盘
  的人顺手拖走钱包。
- **演示模式**：`usb_drives`/`cold_*` 命令在演示模式下返回假数据，卡片
  可完整走通启用/停用/删除副本交互。

### 新增：网络层稳定性——多源回退与离线测试

- **EVM/Solana RPC 多源回退**（b4f8910/c957e31）：余额与报价请求按
  `rpc_fallbacks` 列表逐源尝试，任一可用即成功；JSON-RPC 应答里显式
  `"error": null` 按成功处理（部分节点会把 error 字段带 null 发回来）。
  慢源不再拖死整个刷新：客户端 35s 总超时 + 15s 连接超时。
- **Blockcypher 适配器**（e115001/c68ebf0）：DOGE/DASH 增加 keyless 三级
  回退源（Blockbook → Blockcypher → Blockchair）；兼容其时间戳带纳秒
  小数后缀的格式。
- **IPv4 强制**（b4f8910）：本机 IPv6 出口不可达或被干扰导致间歇性连接
  超时，HTTP 客户端强制走 IPv4。
- **UA 版本跟随**（cdefbb9）：`User-Agent` 里的版本号改读
  `CARGO_PKG_VERSION`，升版不再漏改。
- **离线 mock 测试**（a2b8df6/c957e31）：UtxoAdapter 构造支持注入 base
  URL，EVM/SOL/UTXO 回退编排全部用本机 TcpListener 零网络验证。

### 修复：交易历史与汇率展示

- **BTC/LTC 链上历史修复**：Esplora 解析结构误用 `camelCase` 字段名——
  mempool.space 的 wire 格式是 `snake_case`（`block_time`/
  `scriptpubkey_address`），字段全部落回默认值导致地址永远匹配不上，
  BTC/LTC 的活动列表此前拉不到任何链上记录。
- **活动列表去重修复**：同一笔交易本地记录与链上拉回的时间戳不同
  （本地 `now()` vs 链上 `block_time`），按时间排序后不相邻，原
  `dedup_by` 相邻去重漏删，列表出现双行；改为按键（链+txid+方向）去重。
- **历史页容量**：Blockbook 历史请求 `pageSize` 从 25 提到 100，大账户
  （数百笔交易）活动列表不再只看到最近一两天。
- **汇率缓存降级**：CoinGecko 限流或断网时，法币换算降级到 24h 内的
  最近一次缓存并在警告条注明「展示缓存价格」，不再闪变 "—"；价格服务
  彻底不可用时警告「仅显示链上余额」。
- **余额拉取失败显式提示**：Dashboard 顶部出现错误横幅（含后端消息与
  重试指引），不再静默吞错后永远挂着「正在同步…」。

### 变更：CI

- rust job 拆分 core/tauri 双 job（c34c11b），workspaces 前缀独立缓存；
  release notes 改从本文件提取本版段落（d8932db/07f2156/8ff6b1b）。

---

## 0.7.0（2026-10-28）

新增独立链批次一——6 条新链，共 30 条链。

### 新增链

| 链 | 代号 | 类型 | 数据源 |
| --- | --- | --- | --- |
| Zcash | ZEC | UTXO（透明 t1 地址，ZIP-243 签名） | zelcore Blockbook + Blockchair 回退 |
| Firo | FIRO | UTXO（P2PKH） | blockbook.firo.org |
| Neurai | XNA | UTXO（P2PKH） | blockbook.neurai.org |
| Clore.ai | CLORE | UTXO（P2PKH） | blockbook.clore.zelcore.io |
| Zephyr Protocol | ZEPH | CryptoNote（本地 zephyr-wallet-rpc 签名） | 官方 wallet-rpc（19797） |
| OctaSpace | OCTA | EVM（chain 800001，legacy gas） | 官方 rpc.octa.space |

### 亮点

- **Zcash ZIP-243 签名**：BLAKE2b-256 个性化摘要（NU6.2 branch id）、v4 交易格式、
  空 shielded bundle 全零哈希。摘要算法用两笔真实主网交易端到端验证
  （SIGHASH_ALL 与 SIGHASH_SINGLE 各一，ECDSA 对拍全部通过），并新增
  构建→解析→自验签的 wire-format 测试。
- **签名库修正**：发现 `blake2` crate 的 `Blake2bMac` 无 key 构造会把 128 字节
  零块计入消息（与 ZIP-243 的无 key BLAKE2b 语义不兼容），换用 zcash 生态
  同款 `blake2b_simd`。
- **Zephyr**：与 Monero 共用 CryptoNote 派生（SLIP-10 kholaw）与 wallet-rpc
  架构，设置页新增独立的 zephyr-wallet-rpc 地址（默认端口 19797）。
- **费用估算**：UTXO coin selection 现按链选择交易尺寸模型（Zcash v4 头部
  22 字节 vs 传统 10 字节），避免找零场景构建失败。

---

## 0.6.0（2026-09）

- Monero 原生 25 词助记词与 BIP-39 双格式支持。

---

## 0.5.0（2026-08）

- EthereumPoW (ETHW) 支持；多钱包管理；批次派生预览。

---

## 0.4.0（2026-07）

- Ravencoin / Monacoin / Fujicoin；社区 Blockbook 数据源；
  消息签名与验签（EIP-191 / BIP-137 / Solana）。

---

## 0.3.0（2026-06）

- 20 条链基础版本：收发、总览、工具箱、本地加密 vault。
