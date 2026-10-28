# 更新日志

## 0.7.0（2026-10-28）

新增 2miners 在架币种批次一——6 条新链，共 30 条链。

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

### 暂不支持（详见 README「暂不支持的 2miners 在架币种」）

KAS/KLS/PYI/NEXA/QTC/PRL/QUAI/KDA/CKB/ERG/ALPH（自定义签名体系）、
GRIN/MWC/BEAM（MimbleWimble 无地址模型）、BTG/FLUX（数据源不可实测）、
NEOX/CTXC（2miners 已下架）。

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
