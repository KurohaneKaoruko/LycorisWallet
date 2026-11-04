# 更新日志

## 未发布

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
