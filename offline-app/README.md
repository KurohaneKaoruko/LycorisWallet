# Lycoris Signer — Android 离线签名 App

与 [LycorisWallet](../README.md) 配套的 air-gapped 交易签名器。安装在一台
**永不联网**的旧手机上，接收主钱包构造的 `LYCOSIGN1:` 未签名交易载荷，
核对收款方与金额后用本机密钥签名，产出 `LYCOSIGNED1:` 载荷带回联网电脑广播。

## 安全模型

- **无网络**：release APK 不申请 `INTERNET` 权限（OS 层面断网，不是「不联网」
  的君子协定）；WebView 只通过 Tauri 本地自定义协议加载 APK 内打包的资源。
- **与主钱包同一把核心**：`lycoris-core` 原生编译进 APK（非 wasm），
  助记词→BIP-32/SLIP-10 派生路径、地址编码、签名实现与主钱包逐字节一致，
  派生地址可与主钱包显示地址互相核对。
- **密钥即用即弃**：私钥只在单次命令调用的栈上存在，签名完成即从内存清除，
  不落盘、不进全局状态、无任何持久化存储权限。
- **签名前强制核对**：EVM 载荷自动核对 `from` 地址与本机派生地址，不一致
  红色警告；UTXO 载荷核对派生收款地址；勾选「已逐字核对」才能签名。

## 支持的密钥形态

与主钱包一致的 serde 结构，三种输入：

1. **BIP-39 助记词**（12–24 词）——全链派生，与主钱包助记词钱包地址一致；
2. **口令**（Passphrase，isolated 模式）——与主钱包口令钱包一致；
3. **原始私钥**——`0x` hex（EVM）或 WIF（Bitcoin 系）。

## 构建

依赖：Rust（含 `aarch64-linux-android` target）、Android SDK/NDK、JDK 17+。

```powershell
$env:JAVA_HOME = "C:\Program Files\Java\jdk-21"        # 按 JDK 实际路径
$env:NDK_HOME  = "$env:LOCALAPPDATA\Android\Sdk\ndk\<版本>"
cargo tauri android build --apk --target aarch64 -f offline-app
```

- 产物：`gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk`
  （未签名；用 `apksigner` 或 Android Studio 签名后分发）。
- `gradle/wrapper/gradle-wrapper.properties` 默认走腾讯镜像
  （`mirrors.cloud.tencent.com`）；海外环境可改回 `services.gradle.org`。
- debug 变体（`cargo tauri android dev`）通过 `app/src/debug/AndroidManifest.xml`
  单独补回 `INTERNET` 权限供热重载，release 不含。

## 目录结构

```
offline-app/
├── src/lib.rs          # Rust 壳：signer_prepare / signer_sign / signer_validate
├── tauri.conf.json     # com.lycoris.signer，CSP 全本地
├── ui/                 # 前端单页（index.html + style.css + app.js，零依赖）
└── gen/android/        # Gradle 工程（tauri android init 产物 + 定制）
```
