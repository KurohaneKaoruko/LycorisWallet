# Lycoris Wallet（彼岸花钱包）

本地优先的多链钱包。除标准的 BIP-39 助记词钱包外，它提供一种「口令钱包」：输入任意一句话，
由每条链自己的原生哈希算法直接派生该链私钥——比特币用 SHA-256、莱特币用 scrypt、EVM 用
Keccak-256、Solana 用 SHA-256 → ed25519。一句话，管住全部钱包。

所有密钥、签名与网络请求都发生在 Rust 侧。Monero 是唯一例外：CryptoNote 的
RingCT 环签名交给官方的本地 `monero-wallet-rpc` 完成，密钥仍由本应用派生后交给它。

## 功能

- **双模式钱包**
  - `助记词钱包`：BIP-39 → BIP-32 / SLIP-0010，地址可与主流钱包互相导入导出。
  - `口令钱包`：任意字符串 → 该链原生哈希 → 私钥，支持 `隔离`（默认）与 `严格 H(phrase)` 两种派生方式。
- **多钱包管理**：同一个加密文件里可以保存多个钱包（助记词 / 口令混搭），共用一个本地密码；
  支持新建、导入、重命名、切换与删除（删除需密码确认），交易记录按钱包隔离，地址簿与设置全局共享。
  旧版单钱包文件在解锁时自动迁移到新格式。
- **二十条链**：Bitcoin、Litecoin、Dogecoin、Bitcoin Cash、Ethereum、Ethereum Classic、BNB Chain、Polygon、Solana、Monero，
  以及 Bitcoin SV、Namecoin、Litecoin Cash、Feathercoin、Viacoin、Potcoin、e-Gulden、Dash、Groestlcoin、Vertcoin。
- **收发与浏览**：总览（法币折算）、收款二维码、转账（EVM EIP-1559 / UTXO P2WPKH 与 legacy P2PKH / Solana 系统转账 / Monero CryptoNote）、交易记录、地址簿。
- **工具箱**：地址体检（自动识别链 + 校验和）、单位换算（聪/wei/lamport ↔ 显示单位）、BIP-39 助记词校验、
  哈希与编码（SHA-256/512、Keccak-256、HASH160、RIPEMD-160 与 hex/base64/base58）、
  消息签名与验签（EVM EIP-191 / Bitcoin BIP-137 / Solana ed25519）、支付 URI（BIP-21 / EIP-681 / Solana Pay 解析与构造）。
- **本地加密**：Argon2id + XChaCha20-Poly1305 加密的钱包文件，自动锁定、密码尝试冷却。
- **演示模式**：`ui-react` 开发服务器（`npm run dev`）加 `?mock=1` 直接加载编译成 WebAssembly 的同一份 Rust 核心——地址、指纹、
  地址校验和二维码都是真数据，只有余额、价格与广播结果是演示值，不需要 Rust 后端。

## 支持的网络

| 网络 | 代号 | 口令模式哈希 | 助记词路径 | 地址格式 |
| --- | --- | --- | --- | --- |
| Bitcoin | BTC | SHA-256 | `m/84'/0'/0'/0/0` | bech32（P2WPKH） |
| Litecoin | LTC | scrypt（N=16384, r=8, p=1） | `m/84'/2'/0'/0/0` | bech32（P2WPKH） |
| Dogecoin | DOGE | scrypt（N=16384, r=8, p=1） | `m/44'/3'/0'/0/0` | base58（P2PKH） |
| Bitcoin Cash | BCH | SHA-256 | `m/44'/145'/0'/0/0` | cashaddr |
| Ethereum | ETH | Keccak-256 | `m/44'/60'/0'/0/0` | EIP-55 |
| Ethereum Classic | ETC | Keccak-256 | `m/44'/61'/0'/0/0` | EIP-55 |
| BNB Chain | BNB | Keccak-256 | `m/44'/60'/0'/0/0` | EIP-55 |
| Polygon | POL | Keccak-256 | `m/44'/60'/0'/0/0` | EIP-55 |
| Solana | SOL | SHA-256 → ed25519 | `m/44'/501'/0'/0'` | base58 |
| Monero | XMR | Keccak-256 → CryptoNote | `m/44'/128'/0'/0/0`（SLIP-10 kholaw） | Monero base58 |
| Bitcoin SV | BSV | SHA-256 | `m/44'/236'/0'/0/0` | base58（P2PKH） |
| Namecoin | NMC | SHA-256 | `m/44'/7'/0'/0/0` | base58（P2PKH） |
| Litecoin Cash | LCC | SHA-256 | `m/44'/192'/0'/0/0` | base58（P2PKH） |
| Feathercoin | FTC | scrypt（N=16384, r=8, p=1） | `m/44'/8'/0'/0/0` | base58（P2PKH） |
| Viacoin | VIA | scrypt（N=16384, r=8, p=1） | `m/44'/14'/0'/0/0` | bech32（P2WPKH） |
| Potcoin | POT | scrypt（N=16384, r=8, p=1） | `m/44'/81'/0'/0/0` | base58（P2PKH） |
| e-Gulden | EFL | scrypt（N=16384, r=8, p=1） | `m/44'/78'/0'/0/0` | base58（P2PKH） |
| Dash | DASH | SHA-256* | `m/44'/5'/0'/0/0` | base58（P2PKH） |
| Groestlcoin | GRS | SHA-256* | `m/44'/17'/0'/0/0` | bech32（P2WPKH） |
| Vertcoin | VTC | SHA-256* | `m/44'/28'/0'/0/0` | bech32（P2WPKH） |

> 带 * 的三条链（Dash 的 X11、Groestlcoin 的 Groestl、Vertcoin 的 Lyra2RE）在口令模式下使用
> 域分隔的 SHA-256 替代原生 PoW 哈希级联（`lycoris/pow-fallback/v1/`），保证派生稳定可审计；
> 助记词钱包不受影响（走 BIP-32）。
>
> Dogecoin 与 Bitcoin Cash 的收款、余额与转账均已可用：前者用 legacy P2PKH 传统 sighash，后者用
> BIP-143 + SIGHASH-FORKID，交易字节与 libsecp256k1 独立实现逐字节一致。
> 两条链的链上数据走 Blockchair，现在需要 API Key，可在「设置 → 网络」里填写（保存在加密钱包文件里）。
> 新增的十条链：地址派生、校验与收款即时可用；余额/转账依赖对应 blockbook 端点的可用性
> （部分小链的公共实例可能不稳定，失败时仅显示「余额未查询」并不影响资产管理）。
> Monero 的收款地址派生、余额与转账均已可用：RingCT 签名由本地的 `monero-wallet-rpc` 完成，
> 配置见下方「Monero 转账」一节。

### 口令派生规则

- **隔离模式（默认）**：`message = "lycoris/v1/<chain>" + 0x00 + phrase`，再用该链的哈希算法处理；
  同一条口令在每条链得到不同私钥，域分隔字符串避免跨链重放。
- **严格模式**：`private_key = H_chain(phrase)`，与链上习惯的「passphrase → key」完全一致；
  注意 Ethereum / Ethereum Classic / BNB Chain / Polygon 同为 Keccak-256，会得到同一把私钥。

参考向量（隔离模式，口令 `a quiet garden at dusk`）：

| 网络 | 地址 |
| --- | --- |
| BTC | `bc1qx0q0wdh4z5a64twecyg0kqyqkpc3h66nv8n2gt` |
| LTC | `ltc1qg06z6x74fe6f2qyqkfqk7kle8llgty3s78tfql` |
| ETH | `0x9231e3Fa53ABF00CeC08DB45a7dEDe19f939a0CB` |
| SOL | `5HeimWLA7nee4kKKL8mWbPjMHPXKYTY6gM43LgMk9Q4a` |
| XMR | `48zQvBdmMxj9ByEJWD3uPAVdSKJEnJEsMS8ppnVdJXuh5GjcU9wRcuBck7NTZidFd44bFJmpumZRRi5yufFhd3dP7mHe5Eb` |

Monero 口令模式的派生：`spend = sc_reduce32(Keccak-256(message))`、`view = sc_reduce32(Keccak-256(spend))`，
地址为 CryptoNote 双公钥 + Keccak-4 校验的 Monero base58（与 bip_utils 独立实现互相验证）；
助记词模式走 SLIP-10 kholaw（ed25519 非强化变体）`m/44'/128'/0'/0/0`，`spend = sc_reduce32(Keccak-256(kL ‖ kR))`。

## 工具箱

「工具箱」视图（侧栏 → 工具箱）收纳日常小工具，全部本地计算：

| 工具 | 说明 |
| --- | --- |
| 地址体检 | 粘贴任意地址，自动识别属于哪条链（含跨链同源格式提示）、地址类型（P2PKH/P2WPKH/cashaddr/evm/ed25519）与 EIP-55 校验和状态。 |
| 单位换算 | 二十条链的显示单位 ↔ 最小单位（聪/wei/lamport/piconero…）互转，输入即算。 |
| 助记词校验 | BIP-39 词表逐词检查（错词标红）+ 校验和判定；不派生地址、不落盘。 |
| 哈希 / 编码 | SHA-256、SHA-512、SHA-256d、Keccak-256、HASH160、RIPEMD-160；同一输入并排给出 hex / base64 / base58。 |
| 消息签名 | 用当前钱包密钥签名（EVM 走 EIP-191 `personal_sign`，Bitcoin 走 BIP-137，Solana 走 ed25519）；验签恢复签名者地址。签名需要密码确认，密钥不出 Rust 侧。 |
| 支付 URI | BIP-21 / EIP-681（含 `value=1e18` 科学计数法）/ Solana Pay 的解析与构造，解析结果可一键带入转账。 |

## Monero 转账

Monero 的每笔转账都需要 CLSAG 环签名和 Bulletproofs+ 范围证明。自己实现风险太高，因此
Lycoris 把签名交给 Monero 官方的 `monero-wallet-rpc`（与 Feather Wallet 相同的架构），
密钥仍由本应用从当前口令派生。

1. 启动本地 RPC（`--wallet-dir` 指向一个专用目录；`--disable-rpc-login` 只适合本机监听）：

```bash
monero-wallet-rpc --rpc-bind-port 18083 --disable-rpc-login \
  --wallet-dir ~/.lycoris/monero
```

2. 在「设置 → 网络 → Monero 钱包 RPC」里确认地址（默认 `http://127.0.0.1:18083/json_rpc`）、
   可选的 RPC 用户名/密码，以及**恢复高度**。填创建钱包时的大致区块高度可以避免从创世块
   扫描；留 0 会从头扫描，非常慢。
3. 首次查询余额或转账时，应用会用当前钱包的 spend/view key 在 `--wallet-dir` 下创建钱包文件：
   文件名 `lycoris-<地址哈希前 16 位 hex>`，钱包密码是 `keccak256(spend_key)` 的 hex。
   两者都不落盘保存，每次都从当前口令重新计算。

手续费按 Monero 动态费率估算：典型 1 入 2 出交易约 1.6 KB，费率下限 20,000 piconero/字节，
快速 / 标准 / 经济对应优先级 3 / 2 / 1（20× / 4× / 1×）；实际费用以 RPC 返回为准。
「全部」使用 `sweep_all`，手续费由 Monero 自己扣除。

> `monero-wallet-rpc` 能接触视图密钥与花费密钥，请只连接本机或完全信任的节点，
> 不要把 RPC 端口暴露到公网。
>
> 删除 Lycoris 的本机钱包文件不会删除 `--wallet-dir` 里的 Monero 钱包文件；需要彻底
> 清理时请手动删除该目录。

## 仓库结构

```
crates/core   纯 Rust 核心：派生、地址、vault、网络、交易签名、二维码
crates/wasm   把核心编译成 WebAssembly 的裸 C ABI 桥（演示模式用）
crates/cli    命令行入口（占位，尚未实现）
src-tauri     Tauri 2 桌面外壳：命令、会话状态机、报价与广播、自动锁定
ui-react      React 18 + TypeScript 前端（Vite 构建，桌面与演示模式共用）
tools         图标生成、交易夹具独立校验脚本
```

## 构建与运行

依赖：Rust（MSVC 工具链）+ Windows 上的 WebView2 Runtime；前端构建需要 Node 22。

```bash
cd ui-react && npm ci && npm run build   # 先产出 ui-react/dist（frontendDist 指向这里）
cargo run -p lycoris-wallet              # 开发运行
cargo build --release -p lycoris-wallet
```

- 钱包文件默认保存在系统应用数据目录；设置环境变量 `LYCORIS_VAULT_DIR` 可覆盖（测试用）。
- 演示前端：`cd ui-react && npm run dev`，打开 `http://localhost:5183/?mock=1`；演示内核与桌面同源。

演示模式加载 `ui-react/src/mock/vendor/lycoris-core.wasm`（已提交，约 1.2 MB）。修改核心后重新生成：

```bash
rustup target add wasm32-unknown-unknown
cargo build -p lycoris-wasm --target wasm32-unknown-unknown --release
copy target\wasm32-unknown-unknown\release\lycoris_wasm.wasm ui-react\src\mock\vendor\lycoris-core.wasm
```

`crates/wasm` 使用裸 C ABI（`lycoris_alloc` / `lycoris_call` / `lycoris_result_free`），
不需要 wasm-bindgen；`vault` 与 `network` 特性在 wasm 构建里关闭。少数由 `rand_core`
间接引入的 wasm-bindgen 内部符号在 JS 侧用空函数占位，演示代码不会调用它们。

### 打包

需要 Tauri CLI（仓库里没有预装）：

```bash
cargo install tauri-cli --version "^2"
cargo tauri icon src-tauri/icons/icon.png   # 补齐 macOS 用的 .icns
cargo tauri build                            # 目标：nsis / msi / app / dmg / deb / rpm / appimage
```

macOS 需要 `.icns`，仓库里没有提交，用上面第一条命令从 1024×1024 的源图生成。

### 自动构建与发布

仓库带两条 GitHub Actions 流水线：

- **CI**（`.github/workflows/ci.yml`）：推送到 `main` 或提 PR 时跑 `cargo fmt --check`、`cargo clippy -D warnings`、`cargo test --workspace`、前端模块语法检查，以及演示内核的 wasm 向量校验。
- **Release**（`.github/workflows/release.yml`）：推 `v*` tag 时，在 Windows x64、macOS x64、macOS arm64、Linux x64 上并行打包，产出的安装包统一重命名成带版本号的文件，然后创建 GitHub Release。

发布步骤：

1. 同步版本号——`Cargo.toml` 的 `[workspace.package] version` 与 `src-tauri/tauri.conf.json` 的 `version` 必须一致（流水线会校验，不一致直接失败并提示）；
2. 打 tag 并推送：

```bash
git tag -a v0.2.0 -m "v0.2.0"
git push origin v0.2.0
```

3. 等待流水线跑完，Release 页面会拿到这些文件：

| 平台 | 文件名 |
| --- | --- |
| Windows x64 | `LycorisWallet_0.2.0_x64-setup.exe` / `LycorisWallet_0.2.0_x64.msi` |
| macOS Apple Silicon | `LycorisWallet_0.2.0_arm64.dmg` |
| macOS Intel | `LycorisWallet_0.2.0_x64.dmg` |
| Linux x64 | `LycorisWallet_0.2.0_amd64.AppImage` / `.deb` / `.rpm` |

也可以在 Actions 页面手动触发 Release 流水线并填写版本号（此时会自行补打 tag）。
未做代码签名的 macOS 包首次打开需要在「系统设置 → 隐私与安全性」里放行。

### 移动端

Tauri 2 的 Android / iOS 目标已经可用，但需要本地安装对应 SDK：

```bash
cargo tauri android init && cargo tauri android build
cargo tauri ios init && cargo tauri ios build
```

网络层使用 `rustls`（webpki roots），不依赖 OpenSSL，便于交叉编译。

## 验证

```bash
cargo test -p lycoris-core            # 派生 / vault / 地址 / 交易 / QR / Monero RPC 共 62 项测试
cargo test -p lycoris-wallet          # 会话生命周期、冷却、重置与 Monero 手续费/钱包命名
cargo clippy --all-targets
```

交易签名字节级对照（需要 Python：`eth-account`、`coincurve`、`solders`）：

```bash
cargo run -q -p lycoris-core --example tx_fixtures > out/fixtures.txt
python tools/verify_tx.py out/fixtures.txt        # EVM / Solana / SegWit 逐字节比对
python tools/verify_tx_legacy.py out/fixtures.txt # DOGE 传统 sighash / BCH BIP-143 逐字节比对
```

二维码矩阵用独立解码器回读（需要 Python：`pillow`、`pyzbar`；另装 `segno` 会追加尺寸对照）：

```bash
python tools/verify_qr.py             # 自行调用 qr_fixtures 示例，输出 all QR fixtures decode
cargo run -q -p lycoris-core --example qr_svg_fixtures > out/qr-svg.txt
python tools/verify_qr_svg.py out/qr-svg.txt       # 解析实际渲染的 SVG，再用 zbar 回读
```

演示模式的内核桥接：

```bash
node tools/verify_core_wasm.mjs       # 固定地址向量 + 地址校验 + 生成二维码
python tools/verify_qr_svg.py out/mock-qr-fixtures.txt
```

> 验证产物（fixtures、解码位图、日志等）统一放在 `out/` 目录，该目录已被 `.gitignore` 排除，不会误提交。

## 已知限制

- 交易记录目前只记录本机发起的转账，不索引收款；余额来自公开节点/接口。
- 状态不会自动刷新（记录停留在「待确认」），请以区块浏览器为准。
- Monero 余额与转账依赖本地 `monero-wallet-rpc`；首次扫描需要时间，恢复高度留 0 会从创世块扫起。
- 尚无硬件钱包、多账户、隔离见证找零以外的 UTXO 管理。
- 价格来自 CoinGecko 公共接口，可在设置里关闭。
- `crates/cli` 仍是占位。

## 安全

请阅读 [SECURITY.md](SECURITY.md)。一句话：口令钱包的安全性完全取决于那句话的强度，
不要使用歌词、名言或任何搜索引擎能猜到的句子。

## License

MIT
