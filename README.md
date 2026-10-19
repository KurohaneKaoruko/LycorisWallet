# Lycoris Wallet（彼岸花钱包）

本地优先的多链钱包。除标准的 BIP-39 助记词钱包外，它提供一种「口令钱包」：输入任意一句话，
由每条链自己的原生哈希算法直接派生该链私钥——比特币用 SHA-256、莱特币用 scrypt、EVM 用
Keccak-256、Solana 用 SHA-256 → ed25519。一句话，管住全部钱包。

所有密钥、签名与网络请求都发生在 Rust 侧，前端只是视图层。

## 功能

- **双模式钱包**
  - `助记词钱包`：BIP-39 → BIP-32 / SLIP-0010，地址可与主流钱包互相导入导出。
  - `口令钱包`：任意字符串 → 该链原生哈希 → 私钥，支持 `隔离`（默认）与 `严格 H(phrase)` 两种派生方式。
- **六条链**：Bitcoin、Litecoin、Ethereum、BNB Chain、Polygon、Solana。
- **收发与浏览**：总览（法币折算）、收款二维码、转账（EVM EIP-1559 / UTXO P2WPKH / Solana 系统转账）、交易记录、地址簿。
- **本地加密**：Argon2id + XChaCha20-Poly1305 加密的钱包文件，自动锁定、密码尝试冷却。
- **演示模式**：`ui/index.html?mock=1` 在浏览器里用内存假数据预览完整界面，不需要 Rust 后端。

## 支持的网络

| 网络 | 代号 | 口令模式哈希 | 助记词路径 | 地址格式 |
| --- | --- | --- | --- | --- |
| Bitcoin | BTC | SHA-256 | `m/84'/0'/0'/0/0` | bech32（P2WPKH） |
| Litecoin | LTC | scrypt（N=16384, r=8, p=1） | `m/84'/2'/0'/0/0` | bech32（P2WPKH） |
| Ethereum | ETH | Keccak-256 | `m/44'/60'/0'/0/0` | EIP-55 |
| BNB Chain | BNB | Keccak-256 | `m/44'/60'/0'/0/0` | EIP-55 |
| Polygon | POL | Keccak-256 | `m/44'/60'/0'/0/0` | EIP-55 |
| Solana | SOL | SHA-256 → ed25519 | `m/44'/501'/0'/0'` | base58 |

### 口令派生规则

- **隔离模式（默认）**：`message = "lycoris/v1/<chain>" + 0x00 + phrase`，再用该链的哈希算法处理；
  同一条口令在每条链得到不同私钥，域分隔字符串避免跨链重放。
- **严格模式**：`private_key = H_chain(phrase)`，与链上习惯的「passphrase → key」完全一致；
  注意 Ethereum / BNB Chain / Polygon 使用同一算法，会得到同一把私钥。

参考向量（隔离模式，口令 `a quiet garden at dusk`）：

| 网络 | 地址 |
| --- | --- |
| BTC | `bc1qx0q0wdh4z5a64twecyg0kqyqkpc3h66nv8n2gt` |
| LTC | `ltc1qg06z6x74fe6f2qyqkfqk7kle8llgty3s78tfql` |
| ETH | `0x9231e3Fa53ABF00CeC08DB45a7dEDe19f939a0CB` |
| SOL | `5HeimWLA7nee4kKKL8mWbPjMHPXKYTY6gM43LgMk9Q4a` |

## 仓库结构

```
crates/core   纯 Rust 核心：派生、地址、vault、网络、交易签名、二维码
crates/cli    命令行入口（占位，尚未实现）
src-tauri     Tauri 2 桌面外壳：命令、会话状态机、报价与广播、自动锁定
ui            无打包器的原生 ES 模块前端（withGlobalTauri）
tools         图标生成、交易夹具独立校验脚本
```

## 构建与运行

依赖：Rust（MSVC 工具链）+ Windows 上的 WebView2 Runtime。前端不需要 Node。

```bash
cargo run -p lycoris-wallet          # 开发运行
cargo build --release -p lycoris-wallet
```

- 钱包文件默认保存在系统应用数据目录；设置环境变量 `LYCORIS_VAULT_DIR` 可覆盖（测试用）。
- 演示前端：任意静态服务器指向 `ui/`，打开 `index.html?mock=1`；默认密码提示为 `lycoris-demo`。

### 打包

需要 Tauri CLI（仓库里没有预装）：

```bash
cargo install tauri-cli --version "^2"
cargo tauri build        # 目标：nsis / deb / appimage
```

macOS 打包暂未配置（图标集缺少 `.icns`）。

### 移动端

Tauri 2 的 Android / iOS 目标已经可用，但需要本地安装对应 SDK：

```bash
cargo tauri android init && cargo tauri android build
cargo tauri ios init && cargo tauri ios build
```

网络层使用 `rustls`（webpki roots），不依赖 OpenSSL，便于交叉编译。

## 验证

```bash
cargo test -p lycoris-core            # 派生 / vault / 交易 / QR 共 30 项测试
cargo test -p lycoris-wallet          # 会话生命周期与冷却
cargo clippy --all-targets
```

交易签名字节级对照（需要 Python：`eth-account`、`coincurve`、`solders`）：

```bash
cargo run -q -p lycoris-core --example tx_fixtures > /tmp/fixtures.json
python tools/verify_tx.py             # 独立实现逐字节比对，输出 all fixtures match
```

二维码矩阵用独立解码器回读（需要 Python：`pillow`、`pyzbar`；另装 `segno` 会追加尺寸对照）：

```bash
python tools/verify_qr.py             # 自行调用 qr_fixtures 示例，输出 all QR fixtures decode
```

## 已知限制

- 交易记录目前只记录本机发起的转账，不索引收款；余额来自公开节点/接口。
- 状态不会自动刷新（记录停留在「待确认」），请以区块浏览器为准。
- 尚无硬件钱包、多账户、隔离见证找零以外的 UTXO 管理。
- 价格来自 CoinGecko 公共接口，可在设置里关闭。
- `crates/cli` 仍是占位。

## 安全

请阅读 [SECURITY.md](SECURITY.md)。一句话：口令钱包的安全性完全取决于那句话的强度，
不要使用歌词、名言或任何搜索引擎能猜到的句子。

## License

MIT
