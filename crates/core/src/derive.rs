use crate::address::{
    cashaddr_encode, compressed_pubkey, evm_address, p2pkh_address, p2wpkh_address, solana_address,
    uncompressed_pubkey,
};
use crate::bip32;
use crate::chains::{ChainId, ChainKind};
use crate::error::{Error, Result};
use crate::hash::{chain_hash, public_fingerprint};
use crate::slip10;
#[cfg(feature = "vault")]
use bip39::Language;
use bip39::Mnemonic;
use ed25519_dalek::SigningKey as Ed25519SigningKey;
use k256::SecretKey;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WalletMode {
    Mnemonic,
    Passphrase,
    PrivateKey,
    /// 聚合钱包:任意数量互不相关的独立私钥,一把管一条链上的一个地址。
    Aggregate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DerivationMode {
    /// The chain is mixed into the hashed message, so every chain gets a
    /// distinct key even when two chains use the same hash function.
    #[default]
    Isolated,
    /// `H(phrase)` exactly. This is the most literal form of the requested
    /// brain-wallet derivation, but chains that share a hash share a key.
    Strict,
}

/// Secret wallet material. This type is never serialised to the frontend; it
/// only exists inside the encrypted vault and the unlocked Rust session.
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum WalletSecret {
    Mnemonic {
        mnemonic: String,
    },
    Passphrase {
        phrase: String,
        derivation: DerivationMode,
    },
    /// 导入私钥组成的钥匙环:secp256k1 槽覆盖 8 条 UTXO/EVM 链,
    /// ed25519 槽覆盖 Solana,CryptoNote 槽覆盖 Monero。三类槽互相独立,
    /// 可以只导入其中一类,也可以随时在设置里补加(全能钱包)。
    /// 反序列化兼容旧格式:字符串或 `key` 字段 → 归入 secp256k1 槽。
    PrivateKey {
        #[serde(alias = "key")]
        keys: KeyRing,
    },
    /// 聚合钱包:大量互不相关的独立私钥的集合。每条记录 = 一把钥匙,
    /// 只管自己那条链上的一个地址;没有派生概念,增删互不影响。
    Aggregate {
        entries: Vec<AggregateEntry>,
    },
}

/// 聚合钱包的单条钥匙记录:一钥一链一址。
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateEntry {
    /// 稳定 id:删除、导出、详情都按它定位。
    pub id: String,
    /// 这把钥匙属于哪条链。决定地址算法与导入格式。
    pub chain: ChainId,
    /// 规范化的小写 hex 私钥(按链的曲线校验过)。
    pub key: String,
    /// 可选备注名;缺省时界面显示「链名钥匙 #序号」。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl AggregateEntry {
    /// 从这把钥匙直接推导该链账户(无派生,原始字节直连)。
    pub fn account(&self) -> Result<Account> {
        aggregate_entry_account(self)
    }
}

/// 钥匙环:每类椭圆曲线一把钥匙,按槽独立增删。全部以小写 hex 存储
/// (secp: 32 字节标量;ed25519: 32 字节种子;monero: 32 字节 spend)。
#[derive(Clone, Default, Serialize)]
pub struct KeyRing {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secp256k1: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ed25519: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monero: Option<String>,
}

impl<'de> Deserialize<'de> for KeyRing {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // 新格式:三槽 map;旧格式:单把钥匙(字符串或 `key` 字段),
        // 归入 secp256k1 槽。各槽的原文都过一遍归一化(WIF/hex)。
        #[derive(serde::Deserialize)]
        #[serde(untagged)]
        enum Input {
            Single(#[serde(with = "single_key")] String),
            Map {
                #[serde(default, with = "single_key_opt")]
                secp256k1: Option<String>,
                #[serde(default, with = "single_key_opt")]
                ed25519: Option<String>,
                #[serde(default, with = "single_key_opt")]
                monero: Option<String>,
            },
        }
        mod single_key {
            pub fn deserialize<'de, D>(deserializer: D) -> Result<String, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let input = String::deserialize(deserializer)?;
                super::parse_imported_private_key(&input).map_err(serde::de::Error::custom)
            }
        }
        mod single_key_opt {
            use super::parse_imported_key_slot;
            pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let input = Option::<String>::deserialize(deserializer)?;
                input
                    .map(|raw| parse_imported_key_slot(&raw).map_err(serde::de::Error::custom))
                    .transpose()
            }
        }
        match Input::deserialize(deserializer)? {
            Input::Single(key) => Ok(KeyRing {
                secp256k1: Some(key),
                ed25519: None,
                monero: None,
            }),
            Input::Map {
                secp256k1,
                ed25519,
                monero,
            } => Ok(KeyRing {
                secp256k1,
                ed25519,
                monero,
            }),
        }
    }
}

/// 钥匙环单槽归一化:secp 槽收 WIF/hex;ed25519/monero 槽只收 64 位 hex
/// (Solana 与 Monero 生态没有统一的 WIF 约定,hex 是最大公约数)。
fn parse_imported_key_slot(input: &str) -> std::result::Result<String, String> {
    let trimmed = input.trim();
    if looks_like_wif(trimmed) {
        if let Some(key) = parse_wif(trimmed) {
            return Ok(key);
        }
        return Err("WIF 校验失败(base58check):请检查是否复制完整".into());
    }
    parse_hex_key(trimmed)
}

/// 钥匙环的「槽是否已填」快照:给 UI 的钥匙环管理页用,绝不含钥匙本体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyRingPresence {
    pub secp256k1: bool,
    pub ed25519: bool,
    pub monero: bool,
}

impl KeyRing {
    pub fn presence(&self) -> KeyRingPresence {
        KeyRingPresence {
            secp256k1: self.secp256k1.is_some(),
            ed25519: self.ed25519.is_some(),
            monero: self.monero.is_some(),
        }
    }
}

/// 聚合钱包条目的「目录」视图:id/链/备注,绝不含钥匙本体。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateEntryView {
    pub id: String,
    pub chain: crate::chains::ChainId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl WalletSecret {
    /// 聚合钱包的钥匙目录(其他模式为 `None`)。
    pub fn aggregate_views(&self) -> Option<Vec<AggregateEntryView>> {
        match self {
            Self::Aggregate { entries } => Some(
                entries
                    .iter()
                    .map(|entry| AggregateEntryView {
                        id: entry.id.clone(),
                        chain: entry.chain,
                        label: entry.label.clone(),
                    })
                    .collect(),
            ),
            _ => None,
        }
    }
}

/// 钥匙环的槽类别(命令层 → 存储层的中性枚举)。前端字符串用 snake_case。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeySlotKind {
    Secp256k1,
    Ed25519,
    Monero,
}

/// 命令层入口的槽钥匙归一化:secp 槽收 WIF/hex,ed25519/monero 槽收 hex;
/// 全部拒绝零钥。
pub fn normalize_key_slot(slot: KeySlotKind, input: &str) -> std::result::Result<String, String> {
    let normalized = match slot {
        KeySlotKind::Secp256k1 => parse_imported_private_key(input)?,
        KeySlotKind::Ed25519 | KeySlotKind::Monero => parse_hex_key(input)?,
    };
    if normalized == hex::encode([0u8; 32]) {
        return Err("零私钥非法".into());
    }
    Ok(normalized)
}

/// 判断输入是否形如 WIF(48-52 位 base58 字符,不含 0OIl)。
fn looks_like_wif(trimmed: &str) -> bool {
    trimmed.len() >= 48
        && trimmed.len() <= 52
        && trimmed
            .chars()
            .all(|c| !c.is_whitespace() && !"0OIl".contains(c) && c.is_ascii_alphanumeric())
}

/// base58check 解码 WIF → 小写 hex(32 字节)。
fn parse_wif(trimmed: &str) -> Option<String> {
    let decoded = bs58::decode(trimmed).with_check(None).into_vec().ok()?;
    // 33 字节 = version+key;34 字节 = version+key+压缩标记。
    if (decoded.len() == 33 || decoded.len() == 34) && decoded[1..33].iter().any(|b| *b != 0) {
        Some(hex::encode(&decoded[1..33]))
    } else {
        None
    }
}

/// 64 位 hex(可带 0x)→ 小写 hex。
fn parse_hex_key(trimmed: &str) -> std::result::Result<String, String> {
    let hex_part = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if hex_part.len() == 64 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        let mut key = [0u8; 32];
        hex::decode_to_slice(hex_part, &mut key).map_err(|_| "hex 私钥解析失败".to_string())?;
        return Ok(hex::encode(key));
    }
    Err("无法识别的私钥格式:需要 WIF(base58)或 64 位 hex".into())
}

impl std::fmt::Debug for WalletSecret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WalletSecret::Mnemonic { .. } => f.write_str("WalletSecret::Mnemonic([redacted])"),
            WalletSecret::Passphrase { derivation, .. } => f
                .debug_struct("WalletSecret::Passphrase")
                .field("phrase", &"[redacted]")
                .field("derivation", derivation)
                .finish(),
            WalletSecret::PrivateKey { .. } => f.write_str("WalletSecret::PrivateKey([redacted])"),
            WalletSecret::Aggregate { entries } => f
                .debug_struct("WalletSecret::Aggregate")
                .field("entries", &entries.len())
                .finish(),
        }
    }
}

impl Drop for WalletSecret {
    fn drop(&mut self) {
        match self {
            WalletSecret::Mnemonic { mnemonic } => mnemonic.zeroize(),
            WalletSecret::Passphrase { phrase, .. } => phrase.zeroize(),
            WalletSecret::PrivateKey { keys } => {
                if let Some(key) = &mut keys.secp256k1 {
                    key.zeroize();
                }
                if let Some(key) = &mut keys.ed25519 {
                    key.zeroize();
                }
                if let Some(key) = &mut keys.monero {
                    key.zeroize();
                }
            }
            WalletSecret::Aggregate { entries } => {
                for entry in entries {
                    entry.key.zeroize();
                }
            }
        }
    }
}

impl WalletSecret {
    pub fn new_mnemonic(input: &str) -> Result<Self> {
        let mnemonic = parse_mnemonic(input)?;
        Ok(Self::Mnemonic {
            mnemonic: mnemonic.to_string(),
        })
    }

    pub fn new_passphrase(input: &str, derivation: DerivationMode) -> Result<Self> {
        let canonical = crate::hash::canonical_phrase(input)?;
        Ok(Self::Passphrase {
            phrase: canonical,
            derivation,
        })
    }

    /// 导入一把原始 secp256k1 私钥(WIF 或 64 位 hex)。
    /// 统一存储为小写 hex;密钥非法(非 32 字节/零钥/越界)直接报错。
    pub fn new_private_key(input: &str) -> Result<Self> {
        let key = parse_imported_private_key(input).map_err(Error::InvalidPrivateKey)?;
        if key == hex::encode([0u8; 32]) {
            return Err(Error::InvalidPrivateKey("零私钥非法".into()));
        }
        Ok(Self::PrivateKey {
            keys: KeyRing {
                secp256k1: Some(key),
                ed25519: None,
                monero: None,
            },
        })
    }

    /// 钥匙环只读访问。
    pub fn key_ring(&self) -> Option<&KeyRing> {
        match self {
            Self::PrivateKey { keys } => Some(keys),
            _ => None,
        }
    }

    /// 解析存储的 secp 槽 hex 私钥为可用的 secp256k1 密钥。
    /// 零钥在这里被拒绝(解析时无法在纯文本层面判定)。
    pub fn private_key(&self) -> Result<SecretKey> {
        match self {
            Self::PrivateKey { keys } => {
                let hex_key = keys
                    .secp256k1
                    .as_ref()
                    .ok_or_else(|| Error::InvalidPrivateKey("钥匙环没有 secp256k1 槽".into()))?;
                let mut bytes = [0u8; 32];
                hex::decode_to_slice(hex_key, &mut bytes)
                    .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
                SecretKey::from_slice(&bytes)
                    .map_err(|e| Error::InvalidPrivateKey(format!("密钥非法: {e}")))
            }
            _ => Err(Error::InvalidPrivateKey("钱包不是私钥导入钱包".into())),
        }
    }

    pub fn mode(&self) -> WalletMode {
        match self {
            Self::Mnemonic { .. } => WalletMode::Mnemonic,
            Self::Passphrase { .. } => WalletMode::Passphrase,
            Self::PrivateKey { .. } => WalletMode::PrivateKey,
            Self::Aggregate { .. } => WalletMode::Aggregate,
        }
    }

    pub fn derivation_mode(&self) -> Option<DerivationMode> {
        match self {
            Self::Mnemonic { .. } => None,
            Self::Passphrase { derivation, .. } => Some(*derivation),
            Self::PrivateKey { .. } => None,
            Self::Aggregate { .. } => None,
        }
    }

    /// 聚合钱包的钥匙列表只读访问(其他模式为空切片)。
    pub fn aggregate_entries(&self) -> &[AggregateEntry] {
        match self {
            Self::Aggregate { entries } => entries,
            _ => &[],
        }
    }

    pub fn mnemonic(&self) -> Result<Mnemonic> {
        match self {
            Self::Mnemonic { mnemonic } => parse_mnemonic(mnemonic),
            Self::Passphrase { .. } | Self::PrivateKey { .. } | Self::Aggregate { .. } => Err(
                Error::InvalidMnemonic("wallet is not a mnemonic wallet".into()),
            ),
        }
    }

    pub fn passphrase(&self) -> Option<&str> {
        match self {
            Self::Passphrase { phrase, .. } => Some(phrase),
            Self::Mnemonic { .. } | Self::PrivateKey { .. } | Self::Aggregate { .. } => None,
        }
    }

    pub fn word_count(&self) -> Option<usize> {
        match self {
            Self::Mnemonic { mnemonic } => Some(mnemonic.split_whitespace().count()),
            Self::Passphrase { .. } | Self::PrivateKey { .. } | Self::Aggregate { .. } => None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Mnemonic { mnemonic } => {
                parse_mnemonic(mnemonic)?;
                Ok(())
            }
            Self::Passphrase { phrase, .. } => {
                crate::hash::canonical_phrase(phrase)?;
                Ok(())
            }
            Self::PrivateKey { keys } => {
                // 创建/解锁时就校验钥匙环:至少要有一个槽;每个已填槽
                // 都必须是 32 字节 hex。secp 槽额外拒绝零钥。
                let filled = [
                    keys.secp256k1.as_ref(),
                    keys.ed25519.as_ref(),
                    keys.monero.as_ref(),
                ];
                if filled.iter().all(|slot| slot.is_none()) {
                    return Err(Error::InvalidPrivateKey("钥匙环是空的".into()));
                }
                for slot in filled.into_iter().flatten() {
                    let mut bytes = [0u8; 32];
                    hex::decode_to_slice(slot, &mut bytes)
                        .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
                    if bytes == [0u8; 32] {
                        return Err(Error::InvalidPrivateKey("零私钥非法".into()));
                    }
                }
                Ok(())
            }
            Self::Aggregate { entries } => {
                // 至少一把钥匙;每条记录的 id 非空且不重复,钥匙是 32 字节
                // 非零 hex(按各自链的曲线在导入时已校验,这里兜底)。
                if entries.is_empty() {
                    return Err(Error::InvalidPrivateKey("聚合钱包至少要有一把私钥".into()));
                }
                let mut seen = std::collections::HashSet::new();
                for entry in entries {
                    if entry.id.trim().is_empty() {
                        return Err(Error::InvalidPrivateKey("钥匙记录缺少 id".into()));
                    }
                    if !seen.insert(entry.id.as_str()) {
                        return Err(Error::InvalidPrivateKey(format!(
                            "钥匙记录 id 重复: {}",
                            entry.id
                        )));
                    }
                    let mut bytes = [0u8; 32];
                    hex::decode_to_slice(&entry.key, &mut bytes)
                        .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
                    if bytes == [0u8; 32] {
                        return Err(Error::InvalidPrivateKey("零私钥非法".into()));
                    }
                }
                Ok(())
            }
        }
    }

    /// 导入聚合钱包的第一把私钥(链 + 原始钥匙 + 可选备注)。
    /// 钥匙按链的曲线解析并规范化;生成稳定 id。
    pub fn new_aggregate(chain: ChainId, key: &str, label: Option<String>) -> Result<Self> {
        let normalized = parse_aggregate_key(chain, key).map_err(Error::InvalidPrivateKey)?;
        let entry = AggregateEntry {
            id: aggregate_entry_id(),
            chain,
            key: normalized,
            label: label
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty()),
        };
        // 入口即校验:钥匙必须真的能推出地址(拒零钥/非法标量)。
        entry.account()?;
        Ok(Self::Aggregate {
            entries: vec![entry],
        })
    }

    /// 聚合钱包补加一把私钥(链 + 原始钥匙 + 可选备注)。返回新条目 id。
    pub fn add_aggregate_key(
        &mut self,
        chain: ChainId,
        key: &str,
        label: Option<String>,
    ) -> Result<String> {
        let Self::Aggregate { entries } = self else {
            return Err(Error::Message("只有聚合钱包才能添加聚合钥匙".into()));
        };
        let normalized = parse_aggregate_key(chain, key).map_err(Error::InvalidPrivateKey)?;
        let entry = AggregateEntry {
            id: aggregate_entry_id(),
            chain,
            key: normalized,
            label: label
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty()),
        };
        entry.account()?;
        let id = entry.id.clone();
        entries.push(entry);
        Ok(id)
    }

    /// 聚合钱包摘除一把私钥。至少要留一把。
    pub fn remove_aggregate_key(&mut self, id: &str) -> Result<()> {
        let Self::Aggregate { entries } = self else {
            return Err(Error::Message("只有聚合钱包才能管理聚合钥匙".into()));
        };
        if entries.len() <= 1 {
            return Err(Error::Message("聚合钱包至少要保留一把私钥".into()));
        }
        let before = entries.len();
        entries.retain(|entry| entry.id != id);
        if entries.len() == before {
            return Err(Error::Message(format!("找不到钥匙记录: {id}")));
        }
        Ok(())
    }

    /// 聚合钱包重命名一把私钥的备注。
    pub fn rename_aggregate_key(&mut self, id: &str, label: &str) -> Result<()> {
        let Self::Aggregate { entries } = self else {
            return Err(Error::Message("只有聚合钱包才能管理聚合钥匙".into()));
        };
        let trimmed = label.trim();
        let entry = entries
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| Error::Message(format!("找不到钥匙记录: {id}")))?;
        entry.label = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
        Ok(())
    }
}

/// 解析用户输入的原始私钥:WIF(base58check)或 32 字节 hex。
/// 返回规范化的小写 hex 字符串(入库格式)。
fn parse_imported_private_key(input: &str) -> std::result::Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("私钥不能为空".into());
    }
    if looks_like_wif(trimmed) {
        if let Some(key) = parse_wif(trimmed) {
            return Ok(key);
        }
        // 64 位 hex 不可能包含 0OIl,落 base58 分支说明有非法字符;
        // 若全为 hex 字符则按 hex 报错,否则按 base58 报错。
        if trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("WIF 校验失败(base58check):请检查是否复制完整".into());
        }
    }
    parse_hex_key(trimmed)
}

fn parse_mnemonic(input: &str) -> Result<Mnemonic> {
    let normalized = input
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
    if normalized.is_empty() {
        return Err(Error::InvalidMnemonic("mnemonic is empty".into()));
    }
    Mnemonic::parse_normalized(&normalized).map_err(|e| Error::InvalidMnemonic(e.to_string()))
}

/// Generate a fresh BIP-39 mnemonic. Not available in the WebAssembly build,
/// which has no entropy source and only re-derives existing phrases.
#[cfg(feature = "vault")]
pub fn generate_mnemonic(word_count: usize) -> Result<String> {
    if word_count != 12 && word_count != 24 {
        return Err(Error::InvalidMnemonic("word count must be 12 or 24".into()));
    }
    Mnemonic::generate_in(Language::English, word_count)
        .map(|m| m.to_string())
        .map_err(|e| Error::InvalidMnemonic(e.to_string()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub chain: ChainId,
    pub name: String,
    pub symbol: String,
    pub address: String,
    pub path: String,
    pub derivation: String,
    pub hash_algorithm: String,
    pub hash_detail: String,
    pub address_format: String,
    pub public_key: String,
    pub accent: String,
    pub explorer_url: String,
    pub send_supported: bool,
    pub decimals: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DerivationPreview {
    pub mode: WalletMode,
    pub derivation: Option<DerivationMode>,
    pub accounts: Vec<Account>,
    pub fingerprint: String,
    pub formula: String,
}

/// How multiple accounts are spread out from one secret.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Default,
)]
#[serde(rename_all = "snake_case")]
pub enum BatchStrategy {
    /// Mnemonic wallets: bump the last BIP-32 address index
    /// (`m/84'/0'/0'/0/1`, `/2`, …). Passphrase wallets: the chain domain is
    /// mixed in together with the account index.
    #[default]
    Path,
    /// `H(domain‖phrase‖#index)` — an explicit salted hash. For mnemonic
    /// wallets this leaves the standard BIP-32 tree and hashes the phrase.
    Salted,
    /// `H^index(base)` — the chain hash applied `index` times to the *base
    /// account's key seed*, whichever way the base key was produced. Index
    /// starts at 1 because 0 would repeat the base account.
    Iterated,
}

impl BatchStrategy {
    pub fn label(self) -> &'static str {
        match self {
            Self::Path => "路径派生",
            Self::Salted => "加盐派生",
            Self::Iterated => "迭代哈希",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            Self::Path => "BIP-32 地址索引递增 · 标准路径",
            Self::Salted => "H(域名‖口令‖#序号) · 每个序号独立密钥",
            Self::Iterated => "H(…) 连续哈希 N 次 · 基于基础私钥",
        }
    }

    /// Upper bound for user-supplied indices. Keeps a mistyped count from
    /// freezing the UI (iterated scrypt at 14 rounds is ~50 ms per account,
    /// and BIP-39 to_seed is ~30 ms).
    pub fn max_count(self) -> u32 {
        match self {
            Self::Path | Self::Salted => 100,
            Self::Iterated => 50,
        }
    }

    /// Which strategies the UI should offer for a wallet mode. Mnemonic
    /// wallets walk the BIP-32 tree; salted derivation mixes the *passphrase*
    /// into the hash input, so it belongs to passphrase wallets only.
    /// Iterated hashes work on top of any base key.
    pub fn available_for(mode: WalletMode) -> &'static [BatchStrategy] {
        match mode {
            WalletMode::Mnemonic => &[Self::Path, Self::Iterated],
            WalletMode::Passphrase => &[Self::Salted, Self::Iterated],
            WalletMode::PrivateKey => &[Self::Salted, Self::Iterated],
            // 聚合钱包是独立钥匙的集合,没有任何派生。
            WalletMode::Aggregate => &[],
        }
    }

    /// The strategies a stored vault may still reference from older versions.
    /// `Path` entries in a passphrase wallet keep deriving as salted hashes.
    #[allow(dead_code)]
    pub fn is_valid_for(self, mode: WalletMode) -> bool {
        match mode {
            WalletMode::Mnemonic => true,
            // Tolerated for legacy specs: `Path` derives as a salted hash.
            WalletMode::Passphrase => true,
            // Path 派生对私钥钱包没有意义,derive_chain_account 里显式报错。
            WalletMode::PrivateKey => true,
            // 聚合钱包没有任何合法策略。
            WalletMode::Aggregate => false,
        }
    }
}

/// Request for [`derive_accounts_batch`]: which accounts to derive and how.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchRequest {
    #[serde(default)]
    pub strategy: BatchStrategy,
    /// The chain to derive on. One request adds addresses on one chain only.
    pub chain: ChainId,
    /// Number of accounts, 1..=strategy.max_count().
    pub count: u32,
    /// First account index (0-based). Defaults to 0.
    #[serde(default)]
    pub start: u32,
}

/// A derived account pinned to a wallet. Only the *recipe* is stored —
/// strategy + index — never key material; the addresses are recomputed
/// deterministically from the wallet secret on demand. The fingerprint is a
/// public value kept for the wallet list.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchAccountSpec {
    /// Stable identifier (`"{strategy}:{chain}:{index}"`) used for switching.
    pub id: String,
    pub strategy: BatchStrategy,
    /// The single chain this account belongs to. A derived account adds one
    /// address on one chain; other chains keep using the base derivation.
    pub chain: ChainId,
    pub index: u32,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub fingerprint: String,
}

impl BatchAccountSpec {
    pub fn new(strategy: BatchStrategy, chain: ChainId, index: u32) -> Self {
        Self {
            id: format!("{}:{}:{}", strategy_id(strategy), chain.as_str(), index),
            strategy,
            chain,
            index,
            label: None,
            fingerprint: String::new(),
        }
    }

    pub fn display_label(&self) -> String {
        self.label
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("#{}", self.index))
    }
}

/// The snake_case spelling used in [`BatchAccountSpec::id`].
pub fn strategy_id(strategy: BatchStrategy) -> &'static str {
    match strategy {
        BatchStrategy::Path => "path",
        BatchStrategy::Salted => "salted",
        BatchStrategy::Iterated => "iterated",
    }
}

/// 聚合钱包条目的按链解析:secp 链(BTC/LTC/DOGE/BCH 收 WIF 或 hex;ETH 系
/// 收 hex);Solana 收 64 字节 base58 keypair( Phantom 等钱包导出格式)、
/// 32 字节 base58 或 hex;Monero 收 32 字节 hex spend key。
/// 返回规范化的小写 hex。
pub fn parse_aggregate_key(chain: ChainId, input: &str) -> std::result::Result<String, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("私钥不能为空".into());
    }
    match chain.info().kind {
        ChainKind::Utxo => {
            // UTXO 链社区通用 WIF,兼容纯 hex。
            parse_imported_private_key(trimmed)
        }
        ChainKind::Evm => parse_hex_key(trimmed),
        ChainKind::Ed25519 => parse_solana_key(trimmed),
        ChainKind::Cryptonote => parse_hex_key(trimmed),
    }
}

/// Solana 生态三种主流导出格式 → 32 字节种子 hex:
/// - 64 字节 base58 keypair(Phantom/Solflare CLI 导出,后 32 字节是种子);
/// - 32 字节 base58 种子;
/// - 64 位 hex(可带 0x)。
fn parse_solana_key(trimmed: &str) -> std::result::Result<String, String> {
    let hex_part = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
        .unwrap_or(trimmed);
    if hex_part.len() == 64 && hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
        return parse_hex_key(trimmed);
    }
    // base58:先试 64 字节 keypair,再试 32 字节种子。
    if let Ok(decoded) = bs58::decode(trimmed).into_vec() {
        if decoded.len() == 64 {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(&decoded[32..]);
            return Ok(hex::encode(seed));
        }
        if decoded.len() == 32 {
            return Ok(hex::encode(decoded));
        }
        return Err(format!(
            "Solana 私钥应为 64 字节 base58 keypair 或 32 字节种子,收到 {} 字节",
            decoded.len()
        ));
    }
    Err("无法识别的 Solana 私钥格式".into())
}

/// 聚合钱包条目 → 该链账户。原始钥匙字节直连,无任何派生。
fn aggregate_entry_account(entry: &AggregateEntry) -> Result<Account> {
    let chain = entry.chain;
    let info = chain.info();
    let path = "imported · direct";
    let derivation = "聚合导入 · 直接使用";
    match info.kind {
        ChainKind::Utxo | ChainKind::Evm => {
            let mut bytes = [0u8; 32];
            hex::decode_to_slice(&entry.key, &mut bytes)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
            let secret_key = SecretKey::from_slice(&bytes)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥非法: {e}")))?;
            account_from_secp(
                chain,
                &secret_key,
                path,
                derivation,
                info.hash.label(),
                "原始密钥字节 · 无路径派生",
            )
        }
        ChainKind::Ed25519 => {
            let mut seed = [0u8; 32];
            hex::decode_to_slice(&entry.key, &mut seed)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
            account_from_ed25519(
                chain,
                &seed,
                path,
                derivation,
                info.hash.label(),
                "原始密钥种子 · 无路径派生",
            )
        }
        ChainKind::Cryptonote => {
            let mut spend = [0u8; 32];
            hex::decode_to_slice(&entry.key, &mut spend)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
            let keys = crate::cryptonote::MoneroKeys::from_spend_seed(&spend);
            account_from_monero(
                chain,
                &keys,
                path,
                derivation,
                "Keccak-256 · CryptoNote",
                "spend = 导入字节 · view = reduce(Keccak(spend))",
            )
        }
    }
}

/// 聚合钱包条目的稳定 id:16 字节随机 hex(vault 特性关闭时退化为
/// 时间+进程 id 哈希,wasm 演示桥不依赖宿主随机源)。
fn aggregate_entry_id() -> String {
    #[cfg(feature = "vault")]
    {
        let mut bytes = [0u8; 16];
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut bytes);
        hex::encode(bytes)
    }
    #[cfg(not(feature = "vault"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let mut hasher = Sha256::new();
        hasher.update(nanos.to_le_bytes());
        hasher.update(std::process::id().to_le_bytes());
        hex::encode(&hasher.finalize()[..16])
    }
}

/// Human-readable formula for one derived account, shown in the UI next to
/// the derivation path.
pub fn spec_formula(spec: &BatchAccountSpec, mode: WalletMode) -> String {
    match (spec.strategy, mode) {
        (BatchStrategy::Path, WalletMode::Mnemonic) => {
            format!("BIP-32 地址索引 {} · 路径派生", spec.index)
        }
        (BatchStrategy::Path, _) => format!("H_chain(密语, #{}) · 加盐派生", spec.index),
        (BatchStrategy::Salted, _) => format!("H_chain(密语, #{}) · 加盐派生", spec.index),
        (BatchStrategy::Iterated, _) => format!("H_chain^{}(基础私钥) · 迭代哈希", spec.index),
    }
}

pub struct PrivateKeyMaterial {
    pub chain: ChainId,
    inner: PrivateKeyInner,
}

enum PrivateKeyInner {
    Secp(SecretKey),
    Ed25519([u8; 32]),
    Monero(crate::cryptonote::MoneroKeys),
}

impl std::fmt::Debug for PrivateKeyMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrivateKeyMaterial")
            .field("chain", &self.chain)
            .field("key", &"[redacted]")
            .finish()
    }
}

impl Drop for PrivateKeyMaterial {
    fn drop(&mut self) {
        if let PrivateKeyInner::Ed25519(bytes) = &mut self.inner {
            bytes.zeroize();
        }
        // MoneroKeys zeroises itself in its own Drop impl.
    }
}

impl PrivateKeyMaterial {
    pub fn secp(&self) -> Result<&SecretKey> {
        match &self.inner {
            PrivateKeyInner::Secp(key) => Ok(key),
            _ => Err(Error::Crypto("key is not secp256k1".into())),
        }
    }

    pub fn ed25519_seed(&self) -> Result<[u8; 32]> {
        match &self.inner {
            PrivateKeyInner::Ed25519(seed) => Ok(*seed),
            _ => Err(Error::Crypto("key is not ed25519".into())),
        }
    }

    /// The Monero spend/view keypair. `monero-wallet-rpc` needs both halves
    /// (and the address) to import a wallet that belongs to this phrase.
    pub fn monero_keys(&self) -> Result<&crate::cryptonote::MoneroKeys> {
        match &self.inner {
            PrivateKeyInner::Monero(keys) => Ok(keys),
            _ => Err(Error::Crypto("key is not a Monero keypair".into())),
        }
    }

    /// Export format. UTXO chains return WIF, EVM chains return a 0x-prefixed
    /// hex key, and Solana returns a base58 64-byte keypair (the format most
    /// Solana wallets import).
    pub fn export(&self) -> Result<String> {
        match &self.inner {
            PrivateKeyInner::Secp(key) => match self.chain {
                ChainId::Bitcoin | ChainId::Litecoin => crate::address::wif(key, self.chain),
                _ => Ok(format!("0x{}", hex::encode(key.to_bytes()))),
            },
            PrivateKeyInner::Ed25519(seed) => {
                let signing = Ed25519SigningKey::from_bytes(seed);
                let mut pair = [0u8; 64];
                pair[..32].copy_from_slice(seed);
                pair[32..].copy_from_slice(signing.verifying_key().as_bytes());
                let encoded = bs58::encode(pair).into_string();
                pair.zeroize();
                Ok(encoded)
            }
            PrivateKeyInner::Monero(keys) => {
                // Monero wallets conventionally export `spend‖view` hex.
                let mut raw = [0u8; 64];
                raw[..32].copy_from_slice(&keys.private_spend);
                raw[32..].copy_from_slice(&keys.private_view);
                let encoded = hex::encode(raw);
                raw.zeroize();
                Ok(encoded)
            }
        }
    }
}

fn secp_from_digest(digest: [u8; 32]) -> Result<SecretKey> {
    if let Ok(key) = SecretKey::from_slice(&digest) {
        return Ok(key);
    }
    // The probability of this branch is ~2^-128, but it is specified so the
    // derivation is total and deterministic for any 32-byte digest.
    let mut candidate = digest;
    for counter in 0u8..=255 {
        let mut hasher = Sha256::new();
        hasher.update(b"lycoris/key-fallback/v1");
        hasher.update(candidate);
        hasher.update([counter]);
        candidate.copy_from_slice(&hasher.finalize());
        if let Ok(key) = SecretKey::from_slice(&candidate) {
            return Ok(key);
        }
    }
    Err(Error::Crypto(
        "unable to derive a valid secp256k1 key".into(),
    ))
}

fn account_from_secp(
    chain: ChainId,
    key: &SecretKey,
    path: &str,
    derivation: &str,
    hash_algorithm: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    let address = match chain {
        ChainId::Bitcoin
        | ChainId::Litecoin
        | ChainId::Viacoin
        | ChainId::Groestlcoin
        | ChainId::Vertcoin
        // Monacoin and Fujicoin run native SegWit with their own hrp
        // (mona / fc), exactly like Litecoin does with ltc.
        | ChainId::Monacoin
        | ChainId::Fujicoin => {
            let params = chain.utxo_params().expect("utxo params");
            p2wpkh_address(&compressed_pubkey(key), params.hrp)?
        }
        ChainId::Dogecoin
        | ChainId::BitcoinSv
        | ChainId::Namecoin
        | ChainId::LitecoinCash
        | ChainId::Feathercoin
        | ChainId::Potcoin
        | ChainId::EGulden
        | ChainId::Dash
        // Ravencoin never activated SegWit: plain P2PKH, R… prefix.
        | ChainId::Ravencoin => {
            let params = chain.utxo_params().expect("utxo params");
            p2pkh_address(&compressed_pubkey(key), params.p2pkh_version)
        }
        ChainId::BitcoinCash => {
            let params = chain.utxo_params().expect("utxo params");
            cashaddr_encode(
                &crate::address::hash160(&compressed_pubkey(key)),
                params.hrp,
            )?
        }
        ChainId::Ethereum
        | ChainId::EthereumClassic
        | ChainId::Ethw
        | ChainId::Bsc
        | ChainId::Polygon => evm_address(&uncompressed_pubkey(key))?,
        ChainId::Solana | ChainId::Monero => {
            return Err(Error::UnsupportedChain(chain.to_string()))
        }
    };
    Ok(Account {
        chain,
        name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        address,
        path: path.to_string(),
        derivation: derivation.to_string(),
        hash_algorithm: hash_algorithm.to_string(),
        hash_detail: hash_detail.to_string(),
        address_format: info.kind.address_format().to_string(),
        public_key: hex::encode(compressed_pubkey(key)),
        accent: info.accent.to_string(),
        explorer_url: info.explorer_address.to_string(),
        send_supported: info.send_supported(),
        decimals: info.decimals,
    })
}

fn account_from_ed25519(
    chain: ChainId,
    seed: &[u8; 32],
    path: &str,
    derivation: &str,
    hash_algorithm: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    let signing = Ed25519SigningKey::from_bytes(seed);
    let pubkey = signing.verifying_key().to_bytes();
    let address = solana_address(&pubkey);
    Ok(Account {
        chain,
        name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        address,
        path: path.to_string(),
        derivation: derivation.to_string(),
        hash_algorithm: hash_algorithm.to_string(),
        hash_detail: hash_detail.to_string(),
        address_format: info.kind.address_format().to_string(),
        public_key: hex::encode(pubkey),
        accent: info.accent.to_string(),
        explorer_url: info.explorer_address.to_string(),
        send_supported: info.send_supported(),
        decimals: info.decimals,
    })
}

fn account_from_monero(
    chain: ChainId,
    keys: &crate::cryptonote::MoneroKeys,
    path: &str,
    derivation: &str,
    hash_algorithm: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    Ok(Account {
        chain,
        name: info.name.to_string(),
        symbol: info.symbol.to_string(),
        address: keys.primary_address(),
        path: path.to_string(),
        derivation: derivation.to_string(),
        hash_algorithm: hash_algorithm.to_string(),
        hash_detail: hash_detail.to_string(),
        address_format: info.kind.address_format().to_string(),
        public_key: hex::encode(keys.public_spend),
        accent: info.accent.to_string(),
        explorer_url: info.explorer_address.to_string(),
        send_supported: info.send_supported(),
        decimals: info.decimals,
    })
}

pub fn derive_accounts(secret: &WalletSecret) -> Result<Vec<Account>> {
    secret.validate()?;
    match secret {
        WalletSecret::Mnemonic { .. } => {
            let mnemonic = secret.mnemonic()?;
            let seed = mnemonic.to_seed("");
            let mut accounts = Vec::with_capacity(ChainId::ALL.len());
            for chain in ChainId::ALL {
                let info = chain.info();
                match info.kind {
                    ChainKind::Utxo | ChainKind::Evm => {
                        let derived = bip32::derive_path(&seed, info.derivation_path)?;
                        let algorithm = match info.kind {
                            ChainKind::Utxo => "BIP-32 / BIP-84",
                            ChainKind::Evm => "BIP-32 / BIP-44",
                            ChainKind::Ed25519 | ChainKind::Cryptonote => unreachable!(),
                        };
                        let detail = if chain == ChainId::Bsc || chain == ChainId::Polygon {
                            "BIP-44 coin 60 · 与 Ethereum 共享账户"
                        } else {
                            "BIP-39 seed · standard path"
                        };
                        accounts.push(account_from_secp(
                            chain,
                            &derived.key,
                            info.derivation_path,
                            algorithm,
                            info.hash.label(),
                            detail,
                        )?);
                    }
                    ChainKind::Ed25519 => {
                        let derived = slip10::derive_path(&seed, info.derivation_path)?;
                        accounts.push(account_from_ed25519(
                            chain,
                            &derived.key,
                            info.derivation_path,
                            "BIP-39 / SLIP-0010",
                            info.hash.label(),
                            "Hardened ed25519 derivation",
                        )?);
                    }
                    ChainKind::Cryptonote => {
                        // SLIP-10 ed25519-kholaw (Monero's unclamped variant),
                        // then the standard spend/view expansion:
                        // spend = reduce(Keccak(kL ‖ kR)), view = reduce(Keccak(spend)).
                        let mut k64 = [0u8; 64];
                        k64.copy_from_slice(&crate::slip10::derive_kholaw_path(
                            &seed,
                            info.derivation_path,
                        )?);
                        let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                        k64.zeroize();
                        accounts.push(account_from_monero(
                            chain,
                            &keys,
                            info.derivation_path,
                            "BIP-39 / SLIP-0010 kholaw",
                            "Keccak-256 · CryptoNote",
                            "spend = reduce(kholaw) · view = reduce(Keccak(spend))",
                        )?);
                    }
                }
            }
            Ok(accounts)
        }
        WalletSecret::PrivateKey { keys } => {
            // 钥匙环:每类曲线的槽各自直连自己的链。secp256k1 覆盖
            // 8 条 UTXO/EVM,ed25519 覆盖 Solana,CryptoNote 覆盖 Monero。
            // 没有对应槽的链不生成账户;设置里随时可以补加。
            let mut accounts = Vec::with_capacity(ChainId::ALL.len());
            let secp_key = match &keys.secp256k1 {
                Some(hex_key) => {
                    let mut bytes = [0u8; 32];
                    hex::decode_to_slice(hex_key, &mut bytes)
                        .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
                    Some(
                        SecretKey::from_slice(&bytes)
                            .map_err(|e| Error::InvalidPrivateKey(format!("密钥非法: {e}")))?,
                    )
                }
                None => None,
            };
            for chain in ChainId::ALL {
                let info = chain.info();
                match info.kind {
                    ChainKind::Utxo | ChainKind::Evm => {
                        if let Some(secret_key) = &secp_key {
                            accounts.push(account_from_secp(
                                chain,
                                secret_key,
                                "imported · direct",
                                "导入私钥 · 直接使用",
                                info.hash.label(),
                                "原始密钥字节 · 无路径派生",
                            )?);
                        }
                    }
                    ChainKind::Ed25519 => {
                        if let Some(hex_key) = &keys.ed25519 {
                            let mut seed = [0u8; 32];
                            hex::decode_to_slice(hex_key, &mut seed).map_err(|e| {
                                Error::InvalidPrivateKey(format!("密钥数据损坏: {e}"))
                            })?;
                            accounts.push(account_from_ed25519(
                                chain,
                                &seed,
                                "imported · direct",
                                "导入私钥 · 直接使用",
                                info.hash.label(),
                                "原始密钥种子 · 无路径派生",
                            )?);
                        }
                    }
                    ChainKind::Cryptonote => {
                        if let Some(hex_key) = &keys.monero {
                            let mut spend = [0u8; 32];
                            hex::decode_to_slice(hex_key, &mut spend).map_err(|e| {
                                Error::InvalidPrivateKey(format!("密钥数据损坏: {e}"))
                            })?;
                            let monero_keys =
                                crate::cryptonote::MoneroKeys::from_spend_seed(&spend);
                            accounts.push(account_from_monero(
                                chain,
                                &monero_keys,
                                "imported · direct",
                                "导入私钥 · 直接使用",
                                "Keccak-256 · CryptoNote",
                                "spend = 导入字节 · view = reduce(Keccak(spend))",
                            )?);
                        }
                    }
                }
            }
            Ok(accounts)
        }
        WalletSecret::Aggregate { entries } => {
            // 聚合钱包:每把钥匙一条账户,一把管一条链上的一个地址。
            // 两条记录可以是同一条链(同链多钱包正是聚合钱包的本意)。
            entries
                .iter()
                .map(|entry| {
                    entry.account().map(|mut account| {
                        // path 列展示钥匙备注或链名,帮助区分同链多钥。
                        if let Some(label) = &entry.label {
                            account.path = format!("imported · {label}");
                        }
                        account
                    })
                })
                .collect()
        }
        WalletSecret::Passphrase { phrase, derivation } => {
            let isolated = *derivation == DerivationMode::Isolated;
            let derivation_label = if isolated {
                "H_chain(phrase)"
            } else {
                "H(phrase)"
            };
            let mut accounts = Vec::with_capacity(ChainId::ALL.len());
            for chain in ChainId::ALL {
                let info = chain.info();
                let digest = chain_hash(phrase, chain, isolated)?;
                let path = if isolated {
                    "direct · isolated"
                } else {
                    "direct · strict"
                };
                match info.kind {
                    ChainKind::Utxo | ChainKind::Evm => {
                        let key = secp_from_digest(digest)?;
                        accounts.push(account_from_secp(
                            chain,
                            &key,
                            path,
                            derivation_label,
                            info.hash.label(),
                            info.hash.detail(),
                        )?);
                    }
                    ChainKind::Ed25519 => {
                        accounts.push(account_from_ed25519(
                            chain,
                            &digest,
                            path,
                            derivation_label,
                            info.hash.label(),
                            info.hash.detail(),
                        )?);
                    }
                    ChainKind::Cryptonote => {
                        let keys = crate::cryptonote::MoneroKeys::from_spend_seed(&digest);
                        accounts.push(account_from_monero(
                            chain,
                            &keys,
                            path,
                            derivation_label,
                            info.hash.label(),
                            // Monero shares Keccak-256 with the EVM chains, but
                            // not their description.
                            "CryptoNote Keccak-256 · 32 byte digest",
                        )?);
                    }
                }
            }
            Ok(accounts)
        }
    }
}

/// Derive one account of `chain` at 0-based `index` for a mnemonic wallet,
/// bumping the last (address) component of the standard path.
fn mnemonic_account_at_index(seed: &[u8; 64], chain: ChainId, index: u32) -> Result<Account> {
    let info = chain.info();
    let base = info.derivation_path;
    match info.kind {
        ChainKind::Utxo | ChainKind::Evm => {
            let path = replace_last_index(base, index);
            let derived = bip32::derive_path(seed, &path)?;
            let algorithm = match info.kind {
                ChainKind::Utxo => "BIP-32 / BIP-84",
                ChainKind::Evm => "BIP-32 / BIP-44",
                ChainKind::Ed25519 | ChainKind::Cryptonote => unreachable!(),
            };
            account_from_secp(
                chain,
                &derived.key,
                &path,
                algorithm,
                info.hash.label(),
                "地址索引递增",
            )
        }
        ChainKind::Ed25519 => {
            let path = replace_last_index(base, index);
            let derived = slip10::derive_path(seed, &path)?;
            account_from_ed25519(
                chain,
                &derived.key,
                &path,
                "BIP-39 / SLIP-0010",
                info.hash.label(),
                "Hardened ed25519 derivation",
            )
        }
        ChainKind::Cryptonote => {
            // Monero has no usable multi-account convention here: its standard
            // path ends in non-hardened steps that kholaw rejects, and the
            // ecosystem treats one (spend, view) pair as *the* wallet. Account
            // 0 is the normal wallet; further indices derive subaddresses via
            // the account index where support exists. Keep repeating account 0
            // so the batch stays aligned; the UI explains this.
            let _ = index;
            let mut k64 = [0u8; 64];
            k64.copy_from_slice(&crate::slip10::derive_kholaw_path(seed, base)?);
            let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
            k64.zeroize();
            account_from_monero(
                chain,
                &keys,
                base,
                "BIP-39 / SLIP-0010 kholaw",
                "Keccak-256 · CryptoNote",
                "spend = reduce(kholaw) · view = reduce(Keccak(spend))",
            )
        }
    }
}

fn account_from_digest(
    chain: ChainId,
    digest: [u8; 32],
    path: &str,
    derivation_label: &str,
    hash_detail: &str,
) -> Result<Account> {
    let info = chain.info();
    match info.kind {
        ChainKind::Utxo | ChainKind::Evm => {
            let key = secp_from_digest(digest)?;
            account_from_secp(
                chain,
                &key,
                path,
                derivation_label,
                info.hash.label(),
                hash_detail,
            )
        }
        ChainKind::Ed25519 => account_from_ed25519(
            chain,
            &digest,
            path,
            derivation_label,
            info.hash.label(),
            hash_detail,
        ),
        ChainKind::Cryptonote => {
            let keys = crate::cryptonote::MoneroKeys::from_spend_seed(&digest);
            account_from_monero(
                chain,
                &keys,
                path,
                derivation_label,
                info.hash.label(),
                hash_detail,
            )
        }
    }
}

/// Derive every chain for one account `index` of a mnemonic wallet using the
/// salted strategy. The mnemonic words are hashed with the chain domain and
/// the index mixed in; this deliberately leaves the BIP-32 tree.
fn mnemonic_salted_account(mnemonic: &str, chain: ChainId, index: u32) -> Result<Account> {
    let digest = crate::hash::chain_hash_salted(mnemonic, chain, true, index)?;
    account_from_digest(
        chain,
        digest,
        &format!("salted · #{}", index),
        "H(phrase, #index)",
        "加盐哈希 · 32 byte digest",
    )
}

fn mnemonic_iterated_account(seed: &[u8; 64], chain: ChainId, index: u32) -> Result<Account> {
    let digest = mnemonic_iterated_digest(seed, chain, index)?;
    account_from_digest(
        chain,
        digest,
        &format!("iterated · #{}", index),
        "H^index(base)",
        "迭代哈希 · 32 byte digest",
    )
}

/// The iterated strategy for mnemonic wallets: hash the *base account's key
/// seed* `index` times. The base key comes from the same derivation the base
/// account uses (BIP-32/SLIP-0010 path; Monero via its kholaw spend seed), so
/// the chain is anchored to the wallet's base key, not to the bare words.
fn mnemonic_iterated_digest(seed: &[u8; 64], chain: ChainId, index: u32) -> Result<[u8; 32]> {
    let base = match chain.info().kind {
        ChainKind::Utxo | ChainKind::Evm => bip32::derive_path(seed, chain.info().derivation_path)?
            .key
            .to_bytes()
            .into(),
        ChainKind::Ed25519 => slip10::derive_path(seed, chain.info().derivation_path)?.key,
        ChainKind::Cryptonote => {
            // Anchor to the base account's actual spend key, the same value
            // `from_kholaw_key` reduces to for the base address.
            let mut k64 = slip10::derive_kholaw_path(seed, chain.info().derivation_path)?;
            let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
            k64.zeroize();
            keys.private_spend
        }
    };
    crate::hash::chain_key_iterated(base, chain, index)
}

/// The iterated strategy for passphrase wallets: hash the *base account's
/// 32-byte digest* `index` times, so account #1 already differs from the base.
fn passphrase_iterated_digest(
    phrase: &str,
    chain: ChainId,
    isolated: bool,
    index: u32,
) -> Result<[u8; 32]> {
    let base = crate::hash::chain_hash(phrase, chain, isolated)?;
    crate::hash::chain_key_iterated(base, chain, index)
}

fn passphrase_account_at_index(
    phrase: &str,
    chain: ChainId,
    isolated: bool,
    strategy: BatchStrategy,
    index: u32,
) -> Result<Account> {
    let (digest, path, label, detail) = match strategy {
        BatchStrategy::Path | BatchStrategy::Salted => (
            crate::hash::chain_hash_salted(phrase, chain, isolated, index)?,
            format!("salted · #{}", index),
            if isolated {
                "H_chain(phrase, #i)"
            } else {
                "H(phrase, #i)"
            },
            "加盐哈希 · 32 byte digest",
        ),
        BatchStrategy::Iterated => (
            passphrase_iterated_digest(phrase, chain, isolated, index)?,
            format!("iterated · #{}", index),
            "H_chain^i(base)",
            "迭代哈希 · 32 byte digest",
        ),
    };
    account_from_digest(chain, digest, &path, label, detail)
}

/// Derive `request.count` batch accounts starting at `request.start` for the
/// single chain in the request. The returned vector has one entry per account.
pub fn derive_accounts_batch(
    secret: &WalletSecret,
    request: &BatchRequest,
) -> Result<Vec<Account>> {
    secret.validate()?;
    let max = request.strategy.max_count();
    if request.count == 0 || request.count > max {
        return Err(Error::Message(format!("账户数量必须在 1..={max} 之间")));
    }
    // The iterated chain is anchored at the base key: index 0 would repeat the
    // base account exactly, so it is not a valid starting point.
    if request.strategy == BatchStrategy::Iterated && request.start == 0 {
        return Err(Error::Message(
            "迭代哈希从序号 1 开始（0 号即基础账户）".into(),
        ));
    }
    let end = request.start.saturating_add(request.count).min(max);
    if end - request.start < request.count {
        return Err(Error::Message(format!("起始序号 + 数量不能超过 {max}")));
    }
    (request.start..end)
        .map(|index| derive_chain_account(secret, request.strategy, request.chain, index))
        .collect()
}

/// Derive one chain's account for one derived-account index. This is the only
/// derivation a stored spec needs: a derived account adds one address on one
/// chain, leaving every other chain on the base derivation.
pub fn derive_chain_account(
    secret: &WalletSecret,
    strategy: BatchStrategy,
    chain: ChainId,
    index: u32,
) -> Result<Account> {
    match secret {
        WalletSecret::Mnemonic { .. } => match strategy {
            BatchStrategy::Path => {
                let seed = secret.mnemonic()?.to_seed("");
                mnemonic_account_at_index(&seed, chain, index)
            }
            BatchStrategy::Salted => {
                let mnemonic = secret.mnemonic()?.to_string();
                mnemonic_salted_account(&mnemonic, chain, index)
            }
            BatchStrategy::Iterated => {
                let seed = secret.mnemonic()?.to_seed("");
                mnemonic_iterated_account(&seed, chain, index)
            }
        },
        WalletSecret::Passphrase { phrase, derivation } => {
            let isolated = *derivation == DerivationMode::Isolated;
            passphrase_account_at_index(phrase, chain, isolated, strategy, index)
        }
        WalletSecret::PrivateKey { keys } => {
            private_key_account_at_index(keys, chain, strategy, index)
        }
        WalletSecret::Aggregate { .. } => {
            // 聚合钱包没有派生概念:每把钥匙就是一个固定地址。
            Err(Error::Message("聚合钱包不支持派生地址".into()))
        }
    }
}

/// 导入私钥钱包的派生地址:以对应槽的原始密钥字节为基,加盐/迭代两种
/// 策略。同一条链的多把钥匙不叠加在派生上——派生账户永远锚定该链的
/// 槽钥匙;同链多账户需求由「多把钥匙 → 多个私钥钱包」承载。
fn private_key_account_at_index(
    keys: &KeyRing,
    chain: ChainId,
    strategy: BatchStrategy,
    index: u32,
) -> Result<Account> {
    let info = chain.info();
    match info.kind {
        ChainKind::Utxo | ChainKind::Evm => {
            let hex_key = keys
                .secp256k1
                .as_ref()
                .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
            let mut bytes = [0u8; 32];
            hex::decode_to_slice(hex_key, &mut bytes)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
            let digest = match strategy {
                BatchStrategy::Salted => {
                    crate::hash::chain_hash_salted_bytes(&bytes, chain, index)?
                }
                BatchStrategy::Iterated => crate::hash::chain_key_iterated(bytes, chain, index)?,
                BatchStrategy::Path => {
                    return Err(Error::Message("路径派生不适用于私钥导入钱包".into()));
                }
            };
            let derived = secp_from_digest(digest)?;
            account_from_secp(
                chain,
                &derived,
                &format!("imported · #{}", index),
                match strategy {
                    BatchStrategy::Salted => "H(key, #i)",
                    _ => "H^i(key)",
                },
                info.hash.label(),
                match strategy {
                    BatchStrategy::Salted => "加盐哈希 · 32 byte digest",
                    _ => "迭代哈希 · 32 byte digest",
                },
            )
        }
        ChainKind::Ed25519 | ChainKind::Cryptonote => {
            Err(Error::UnsupportedChain(chain.to_string()))
        }
    }
}

/// Derive the full per-chain account list for one stored
/// [`BatchAccountSpec`].
/// A spec covers a single chain, so the result is the base derivation with
/// that one chain replaced by the derived account.
pub fn derive_accounts_for_spec(
    secret: &WalletSecret,
    spec: &BatchAccountSpec,
) -> Result<Vec<Account>> {
    let mut accounts = derive_accounts(secret)?;
    if let Some(slot) = accounts
        .iter_mut()
        .find(|account| account.chain == spec.chain)
    {
        *slot = derive_chain_account(secret, spec.strategy, spec.chain, spec.index)?;
    }
    Ok(accounts)
}

/// Derive one chain's private key for a stored [`BatchAccountSpec`] (or the
/// base account when `strategy`/`index` are `None`). Powers the per-address
/// detail dialog; the caller must gate it behind a password check like any
/// other private-key export.
pub fn derive_chain_private_key_for_spec(
    secret: &WalletSecret,
    spec: Option<&BatchAccountSpec>,
    chain: ChainId,
) -> Result<PrivateKeyMaterial> {
    secret.validate()?;
    let (strategy, index) = match spec {
        None => (None, 0u32),
        Some(spec) => (Some(spec.strategy), spec.index),
    };
    match secret {
        WalletSecret::Mnemonic { .. } => {
            let mnemonic = secret.mnemonic()?;
            match (chain.info().kind, strategy) {
                (ChainKind::Utxo | ChainKind::Evm, None) => {
                    let seed = mnemonic.to_seed("");
                    let path = chain.info().derivation_path;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Secp(bip32::derive_path(&seed, path)?.key),
                    })
                }
                (ChainKind::Ed25519, None) => {
                    let seed = mnemonic.to_seed("");
                    let path = chain.info().derivation_path;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Ed25519(slip10::derive_path(&seed, path)?.key),
                    })
                }
                (ChainKind::Cryptonote, None) => {
                    let seed = mnemonic.to_seed("");
                    let base = chain.info().derivation_path;
                    let mut k64 = [0u8; 64];
                    k64.copy_from_slice(&crate::slip10::derive_kholaw_path(&seed, base)?);
                    let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                    k64.zeroize();
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Monero(keys),
                    })
                }
                // Derived accounts below: Path shares the seed, the rest are
                // pure hashes of the phrase.
                (ChainKind::Utxo | ChainKind::Evm, Some(BatchStrategy::Path)) => {
                    let seed = mnemonic.to_seed("");
                    let info = chain.info();
                    let path = replace_last_index(info.derivation_path, index);
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Secp(bip32::derive_path(&seed, &path)?.key),
                    })
                }
                (ChainKind::Ed25519, Some(BatchStrategy::Path)) => {
                    let seed = mnemonic.to_seed("");
                    let path = replace_last_index(chain.info().derivation_path, index);
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Ed25519(slip10::derive_path(&seed, &path)?.key),
                    })
                }
                (ChainKind::Cryptonote, Some(BatchStrategy::Path)) => {
                    // Same convention as `mnemonic_account_at_index`: repeat
                    // the base Monero account for every index.
                    let seed = mnemonic.to_seed("");
                    let base = chain.info().derivation_path;
                    let mut k64 = [0u8; 64];
                    k64.copy_from_slice(&crate::slip10::derive_kholaw_path(&seed, base)?);
                    let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                    k64.zeroize();
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Monero(keys),
                    })
                }
                (_, Some(BatchStrategy::Salted)) => {
                    let digest =
                        crate::hash::chain_hash_salted(&mnemonic.to_string(), chain, true, index)?;
                    Ok(digest_to_key(chain, digest))
                }
                (_, Some(BatchStrategy::Iterated)) => {
                    let seed = mnemonic.to_seed("");
                    let digest = mnemonic_iterated_digest(&seed, chain, index)?;
                    Ok(digest_to_key(chain, digest))
                }
            }
        }
        WalletSecret::Passphrase { phrase, derivation } => {
            let isolated = *derivation == DerivationMode::Isolated;
            let digest = match strategy {
                None => crate::hash::chain_hash(phrase, chain, isolated)?,
                Some(BatchStrategy::Path | BatchStrategy::Salted) => {
                    crate::hash::chain_hash_salted(phrase, chain, isolated, index)?
                }
                Some(BatchStrategy::Iterated) => {
                    passphrase_iterated_digest(phrase, chain, isolated, index)?
                }
            };
            Ok(digest_to_key(chain, digest))
        }
        WalletSecret::PrivateKey { keys } => {
            match chain.info().kind {
                ChainKind::Utxo | ChainKind::Evm => {
                    let hex_key = keys
                        .secp256k1
                        .as_ref()
                        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
                    let mut bytes = [0u8; 32];
                    hex::decode_to_slice(hex_key, &mut bytes)
                        .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
                    let digest = match strategy {
                        None => bytes,
                        Some(BatchStrategy::Salted) => {
                            crate::hash::chain_hash_salted_bytes(&bytes, chain, index)?
                        }
                        Some(BatchStrategy::Iterated) => {
                            crate::hash::chain_key_iterated(bytes, chain, index)?
                        }
                        // Path 派生没有 BIP-32 树可走,回落为原始密钥。
                        Some(BatchStrategy::Path) => bytes,
                    };
                    let key = secp_from_digest(digest)?;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Secp(key),
                    })
                }
                ChainKind::Ed25519 => {
                    // 直接导出的密钥:原始种子即 ed25519 密钥,派生策略不适用。
                    let hex_key = keys
                        .ed25519
                        .as_ref()
                        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
                    let mut seed = [0u8; 32];
                    hex::decode_to_slice(hex_key, &mut seed)
                        .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Ed25519(seed),
                    })
                }
                ChainKind::Cryptonote => {
                    let hex_key = keys
                        .monero
                        .as_ref()
                        .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
                    let mut spend = [0u8; 32];
                    hex::decode_to_slice(hex_key, &mut spend)
                        .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Monero(
                            crate::cryptonote::MoneroKeys::from_spend_seed(&spend),
                        ),
                    })
                }
            }
        }
        WalletSecret::Aggregate { entries } => {
            // 聚合钱包的导出按条目 id 定位(spec 机制不适用);同链多钥
            // 时由调用方保证 id 唯一指向一把钥匙。
            let entry = entries
                .iter()
                .find(|entry| entry.chain == chain)
                .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
            let material = entry_key_material(entry)?;
            Ok(material)
        }
    }
}

/// 聚合条目的原始钥匙 → 可导出的密钥材料。
fn entry_key_material(entry: &AggregateEntry) -> Result<PrivateKeyMaterial> {
    let chain = entry.chain;
    match chain.info().kind {
        ChainKind::Utxo | ChainKind::Evm => {
            let mut bytes = [0u8; 32];
            hex::decode_to_slice(&entry.key, &mut bytes)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
            let secret_key = SecretKey::from_slice(&bytes)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥非法: {e}")))?;
            Ok(PrivateKeyMaterial {
                chain,
                inner: PrivateKeyInner::Secp(secret_key),
            })
        }
        ChainKind::Ed25519 => {
            let mut seed = [0u8; 32];
            hex::decode_to_slice(&entry.key, &mut seed)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
            Ok(PrivateKeyMaterial {
                chain,
                inner: PrivateKeyInner::Ed25519(seed),
            })
        }
        ChainKind::Cryptonote => {
            let mut spend = [0u8; 32];
            hex::decode_to_slice(&entry.key, &mut spend)
                .map_err(|e| Error::InvalidPrivateKey(format!("密钥数据损坏: {e}")))?;
            Ok(PrivateKeyMaterial {
                chain,
                inner: PrivateKeyInner::Monero(crate::cryptonote::MoneroKeys::from_spend_seed(
                    &spend,
                )),
            })
        }
    }
}

/// Wrap a raw 32-byte digest into chain-appropriate key material. Mirrors the
/// key branches of [`derive_private_key`].
fn digest_to_key(chain: ChainId, digest: [u8; 32]) -> PrivateKeyMaterial {
    match chain.info().kind {
        ChainKind::Utxo | ChainKind::Evm => PrivateKeyMaterial {
            chain,
            // `secp_from_digest` is total: it rehashes in the ~2^-128 case.
            inner: PrivateKeyInner::Secp(
                secp_from_digest(digest).expect("digest -> secp256k1 is total"),
            ),
        },
        ChainKind::Ed25519 => PrivateKeyMaterial {
            chain,
            inner: PrivateKeyInner::Ed25519(digest),
        },
        ChainKind::Cryptonote => PrivateKeyMaterial {
            chain,
            inner: PrivateKeyInner::Monero(crate::cryptonote::MoneroKeys::from_spend_seed(&digest)),
        },
    }
}

/// Replace the final path component: `m/84'/0'/0'/0/0` + index 3 becomes
/// `m/84'/0'/0'/0/3`. Hardened markers on the replaced component are kept:
/// `m/44'/501'/0'/0'` + 2 becomes `m/44'/501'/0'/2'`.
fn replace_last_index(path: &str, index: u32) -> String {
    match path.rsplit_once('/') {
        Some((head, last)) => {
            let marker = if last.ends_with(['\'', 'h']) {
                &last[last.len() - 1..]
            } else {
                ""
            };
            format!("{head}/{index}{marker}")
        }
        None => path.to_string(),
    }
}

pub fn derive_private_key(secret: &WalletSecret, chain: ChainId) -> Result<PrivateKeyMaterial> {
    secret.validate()?;
    match secret {
        WalletSecret::Aggregate { entries } => {
            // 无 spec 的按链导出:同链多钥时取第一条(界面路径永远带 id)。
            let entry = entries
                .iter()
                .find(|entry| entry.chain == chain)
                .ok_or_else(|| Error::UnsupportedChain(chain.to_string()))?;
            entry_key_material(entry)
        }
        WalletSecret::Mnemonic { .. } => {
            let mnemonic = secret.mnemonic()?;
            let seed = mnemonic.to_seed("");
            let info = chain.info();
            match info.kind {
                ChainKind::Utxo | ChainKind::Evm => {
                    let derived = bip32::derive_path(&seed, info.derivation_path)?;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Secp(derived.key),
                    })
                }
                ChainKind::Ed25519 => {
                    let derived = slip10::derive_path(&seed, info.derivation_path)?;
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Ed25519(derived.key),
                    })
                }
                ChainKind::Cryptonote => {
                    let mut k64 = [0u8; 64];
                    k64.copy_from_slice(&slip10::derive_kholaw_path(&seed, info.derivation_path)?);
                    let keys = crate::cryptonote::MoneroKeys::from_kholaw_key(&k64);
                    k64.zeroize();
                    Ok(PrivateKeyMaterial {
                        chain,
                        inner: PrivateKeyInner::Monero(keys),
                    })
                }
            }
        }
        WalletSecret::Passphrase { phrase, derivation } => {
            let digest = chain_hash(phrase, chain, *derivation == DerivationMode::Isolated)?;
            match chain.info().kind {
                ChainKind::Utxo | ChainKind::Evm => Ok(PrivateKeyMaterial {
                    chain,
                    inner: PrivateKeyInner::Secp(secp_from_digest(digest)?),
                }),
                ChainKind::Ed25519 => Ok(PrivateKeyMaterial {
                    chain,
                    inner: PrivateKeyInner::Ed25519(digest),
                }),
                ChainKind::Cryptonote => Ok(PrivateKeyMaterial {
                    chain,
                    inner: PrivateKeyInner::Monero(crate::cryptonote::MoneroKeys::from_spend_seed(
                        &digest,
                    )),
                }),
            }
        }
        WalletSecret::PrivateKey { .. } => match chain.info().kind {
            ChainKind::Utxo | ChainKind::Evm => Ok(PrivateKeyMaterial {
                chain,
                inner: PrivateKeyInner::Secp(secret.private_key()?),
            }),
            ChainKind::Ed25519 | ChainKind::Cryptonote => {
                Err(Error::UnsupportedChain(chain.to_string()))
            }
        },
    }
}

pub fn derive_fingerprint(accounts: &[Account]) -> String {
    public_fingerprint(
        &accounts
            .iter()
            .map(|account| account.address.clone())
            .collect::<Vec<_>>(),
    )
}

pub fn preview_derivation(secret: &WalletSecret) -> Result<DerivationPreview> {
    let accounts = derive_accounts(secret)?;
    let fingerprint = derive_fingerprint(&accounts);
    let mode = secret.mode();
    let derivation = secret.derivation_mode();
    let formula = match secret {
        WalletSecret::Mnemonic { .. } => {
            "BIP-39 mnemonic → 512-bit seed → BIP-32 / SLIP-0010 paths".to_string()
        }
        WalletSecret::Passphrase { derivation, .. } => {
            if *derivation == DerivationMode::Isolated {
                "H_chain(\"lycoris/v1/<chain>\\0\" + phrase) → 32 bytes → private key".to_string()
            } else {
                "H_chain(phrase) → 32 bytes → private key".to_string()
            }
        }
        WalletSecret::PrivateKey { .. } => {
            "imported secp256k1 key → same key bytes on every supported chain".to_string()
        }
        WalletSecret::Aggregate { entries } => {
            format!(
                "aggregate of {} independent keys → one address each, no derivation",
                entries.len()
            )
        }
    };
    Ok(DerivationPreview {
        mode,
        derivation,
        accounts,
        fingerprint,
        formula,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_path_bumps_the_bip32_address_index() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Path,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 0,
            },
        )
        .unwrap();
        assert_eq!(batch.len(), 2);
        // Account #0 must equal the plain derivation on that chain.
        let plain = derive_accounts(&secret)
            .unwrap()
            .into_iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        assert_eq!(plain.address, batch[0].address);
        // Account #1 must differ and bump the address index.
        assert_ne!(plain.address, batch[1].address);
        assert!(
            batch[1].path.ends_with("/1") || batch[1].path.ends_with("/1'"),
            "path should end in /1 or /1', got {}",
            batch[1].path
        );
        // Other chains are untouched by a single-chain batch: their base
        // derivation has no "/1" variant because no spec was created for them.
        let spec = BatchAccountSpec::new(BatchStrategy::Path, ChainId::Bitcoin, 1);
        let accounts = derive_accounts_for_spec(&secret, &spec).unwrap();
        let eth_base = derive_accounts(&secret)
            .unwrap()
            .into_iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        let eth = accounts
            .into_iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        assert_eq!(
            eth.address, eth_base.address,
            "non-target chains stay on base"
        );
    }

    #[test]
    fn batch_salted_is_deterministic_and_index_disjoint() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 3,
                start: 0,
            },
        )
        .unwrap();
        let again = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 3,
                start: 0,
            },
        )
        .unwrap();
        for (a, b) in batch.iter().zip(&again) {
            assert_eq!(a.address, b.address);
        }
        // All three accounts differ.
        assert_ne!(batch[0].address, batch[1].address);
        assert_ne!(batch[1].address, batch[2].address);
        assert_ne!(batch[0].address, batch[2].address);
    }

    #[test]
    fn batch_iterated_is_anchored_to_base_key() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let plain = derive_accounts(&secret)
            .unwrap()
            .into_iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        // Index 1 must already differ from the base account: the chain starts
        // at the base key and hashes it once more.
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Iterated,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 1,
            },
        )
        .unwrap();
        assert_ne!(
            plain.address, batch[0].address,
            "H(base) must differ from base"
        );
        // And index 0 is rejected outright — it would repeat the base.
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Iterated,
                chain: ChainId::Bitcoin,
                count: 1,
                start: 0,
            },
        )
        .is_err());
    }

    #[test]
    fn batch_start_window_is_respected_and_bounded() {
        let secret = WalletSecret::new_passphrase("seed phrase", DerivationMode::Isolated).unwrap();
        let batch = derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 5,
            },
        )
        .unwrap();
        assert_eq!(batch.len(), 2);
        assert!(batch[0].path.ends_with("#5"), "got {}", batch[0].path);
        assert!(batch[1].path.ends_with("#6"), "got {}", batch[1].path);

        // Out-of-range requests are rejected.
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 0,
                start: 0,
            }
        )
        .is_err());
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 101,
                start: 0,
            }
        )
        .is_err());
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Iterated,
                chain: ChainId::Bitcoin,
                count: 2,
                start: 49,
            }
        )
        .is_err());
    }

    #[test]
    fn private_key_wallet_imports_wif_and_hex() {
        // hex "0...01" 的标准 WIF(压缩,主网):众所周知的测试向量。
        let wif = "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn";
        let from_wif = WalletSecret::new_private_key(wif).unwrap();
        // 同一把钥匙的 hex 形态,两者必须归一为同一存储。
        let from_hex = WalletSecret::new_private_key(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        match (&from_wif, &from_hex) {
            (
                WalletSecret::PrivateKey {
                    keys:
                        KeyRing {
                            secp256k1: Some(a), ..
                        },
                },
                WalletSecret::PrivateKey {
                    keys:
                        KeyRing {
                            secp256k1: Some(b), ..
                        },
                },
            ) => {
                assert_eq!(a, b, "WIF 与 hex 必须归一为同一把钥匙");
            }
            _ => panic!("wrong variant"),
        }

        let accounts = derive_accounts(&from_wif).unwrap();
        // secp 链(原 8 条 + 2026 追加的 10 条 UTXO 链 + 2026 二批的
        // RVN/MONA/FJC),无 Solana/Monero(钥匙环只导入了 secp 槽)。
        assert_eq!(accounts.len(), 22);
        assert!(accounts.iter().all(|a| a.chain != ChainId::Solana));
        assert!(accounts.iter().all(|a| a.chain != ChainId::Monero));
        // 私钥 1 的以太坊地址是众所周知的向量。
        let eth = accounts
            .iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        assert_eq!(
            eth.address.to_lowercase(),
            "0x7e5f4552091a69125d5dfcb7b8c2659029395bdf"
        );
        let btc = accounts
            .iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        assert_eq!(btc.path, "imported · direct");
        assert_eq!(btc.derivation, "导入私钥 · 直接使用");
    }

    #[test]
    fn private_key_wallet_rejects_bad_input() {
        // 非法 base58(含校验错误的 WIF 形状)。
        assert!(WalletSecret::new_private_key("1BgGZ9tcN4rm9KBzDn7KprQz87SZ26SAMH").is_err());
        // 空与短 hex。
        assert!(WalletSecret::new_private_key("").is_err());
        assert!(WalletSecret::new_private_key("abcd").is_err());
        // 63 位 hex。
        assert!(WalletSecret::new_private_key(
            "000000000000000000000000000000000000000000000000000000000000001"
        )
        .is_err());
        // 零私钥非法。
        assert!(WalletSecret::new_private_key(
            "0000000000000000000000000000000000000000000000000000000000000000"
        )
        .is_err());
    }

    #[test]
    fn aggregate_wallet_one_key_one_address() {
        // 三把互不相关的钥匙:BTC(WIF)+ EVM(hex)+ Monero(hex spend)。
        // 其中 BTC 与 EVM 链上各一把;无派生,每把只出一个地址。
        let mut secret = WalletSecret::new_aggregate(
            ChainId::Bitcoin,
            "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn",
            Some("冷存储".into()),
        )
        .unwrap();
        secret
            .add_aggregate_key(
                ChainId::Ethereum,
                "0x0000000000000000000000000000000000000000000000000000000000000001",
                None,
            )
            .unwrap();
        let monero_id = secret
            .add_aggregate_key(
                ChainId::Monero,
                "0b2f7b6c4f3ea25da6d54f0d1ec2ab8a3f4c5d6e7f8091a2b3c4d5e6f708192a",
                None,
            )
            .unwrap();

        let accounts = derive_accounts(&secret).unwrap();
        assert_eq!(accounts.len(), 3, "一把钥匙一个地址");
        assert!(accounts.iter().any(|a| a.chain == ChainId::Bitcoin));
        assert!(accounts.iter().any(|a| a.chain == ChainId::Ethereum));
        assert!(accounts.iter().any(|a| a.chain == ChainId::Monero));
        // 备注显示在 path 列。
        assert!(accounts[0].path.contains("冷存储"));

        // 同链多钥:再补一把 BTC,账户数 +1。
        secret
            .add_aggregate_key(
                ChainId::Bitcoin,
                "0x0000000000000000000000000000000000000000000000000000000000000002",
                None,
            )
            .unwrap();
        assert_eq!(derive_accounts(&secret).unwrap().len(), 4);

        // 无派生:批量派生一律报错;可用策略为空。
        assert!(BatchStrategy::available_for(WalletMode::Aggregate).is_empty());
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Salted,
                chain: ChainId::Bitcoin,
                count: 1,
                start: 1,
            },
        )
        .is_err());

        // 摘除与空钱包校验。
        secret.remove_aggregate_key(&monero_id).unwrap();
        assert!(secret.aggregate_entries().len() == 3);
        let err = secret.remove_aggregate_key(&monero_id).unwrap_err();
        assert!(err.to_string().contains("找不到钥匙记录"));

        // Solana base58 keypair(64 字节)→ 种子归一。
        // 0x000…001 私钥的 64 字节 keypair 形式(0…01 ‖ 公钥占位)不做真实验证,
        // 只验证 hex 路径;keypair 路径由解析单元保证。
        let sol = WalletSecret::new_aggregate(
            ChainId::Solana,
            "d97db1d57e12e19b45aad225ea140f38d0f0dae9fdf5bbbb60b5a90f09990a2f",
            None,
        )
        .unwrap();
        assert_eq!(derive_accounts(&sol).unwrap().len(), 1);
        assert_eq!(derive_accounts(&sol).unwrap()[0].chain, ChainId::Solana);

        // 导入即拒非法钥匙。
        assert!(WalletSecret::new_aggregate(ChainId::Ethereum, "zz", None).is_err());
        assert!(WalletSecret::new_aggregate(
            ChainId::Ethereum,
            "0000000000000000000000000000000000000000000000000000000000000000",
            None
        )
        .is_err());
    }

    #[test]
    fn key_ring_covers_all_chains_and_reads_legacy_format() {
        // 三槽齐全:10 条链全部生成账户。
        let hex32 = |b: u8| hex::encode([b; 32]);
        let full = WalletSecret::PrivateKey {
            keys: KeyRing {
                secp256k1: Some(hex32(1)),
                ed25519: Some(hex32(2)),
                monero: Some(hex32(3)),
            },
        };
        assert!(full.validate().is_ok());
        let accounts = derive_accounts(&full).unwrap();
        assert_eq!(accounts.len(), ChainId::ALL.len(), "三槽齐全应覆盖全部链");
        assert!(accounts.iter().any(|a| a.chain == ChainId::Solana));
        assert!(accounts.iter().any(|a| a.chain == ChainId::Monero));

        // 旧格式(单把 WIF 字符串 / `key` 字段)读入后归入 secp 槽。
        let legacy: WalletSecret = serde_json::from_str(
            r#"{ "mode": "private_key",
                 "key": "KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73sVHnoWn" }"#,
        )
        .unwrap();
        let ring = legacy.key_ring().unwrap();
        assert_eq!(
            ring.secp256k1.as_deref(),
            Some("0000000000000000000000000000000000000000000000000000000000000001")
        );
        assert!(ring.ed25519.is_none() && ring.monero.is_none());

        // 新格式 map + 归一化(ed25519/monero 槽收 hex)。
        let modern: WalletSecret = serde_json::from_str(&format!(
            r#"{{ "mode": "private_key",
                 "keys": {{ "secp256k1": "0x{}", "ed25519": "{}", "monero": "{}" }} }}"#,
            hex32(1),
            hex32(2),
            hex32(3)
        ))
        .unwrap();
        assert_eq!(
            modern.key_ring().unwrap().secp256k1.as_deref(),
            Some(hex32(1).as_str()),
            "0x 前缀应被剥掉"
        );

        // 全空钥匙环非法。
        let empty = WalletSecret::PrivateKey {
            keys: KeyRing::default(),
        };
        assert!(empty.validate().is_err());
    }

    #[test]
    fn private_key_wallet_batch_derives_salted_and_iterated() {
        let secret = WalletSecret::new_private_key(
            "0000000000000000000000000000000000000000000000000000000000000001",
        )
        .unwrap();
        let base = derive_accounts(&secret)
            .unwrap()
            .into_iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        for strategy in [BatchStrategy::Salted, BatchStrategy::Iterated] {
            let batch = derive_accounts_batch(
                &secret,
                &BatchRequest {
                    strategy,
                    chain: ChainId::Bitcoin,
                    count: 1,
                    start: 1,
                },
            )
            .unwrap();
            assert_ne!(
                batch[0].address, base.address,
                "{strategy:?} 必须异于基础账户"
            );
        }
        // Path 策略不适用。
        assert!(derive_accounts_batch(
            &secret,
            &BatchRequest {
                strategy: BatchStrategy::Path,
                chain: ChainId::Bitcoin,
                count: 1,
                start: 1,
            },
        )
        .is_err());
        // 详情导出私钥:基础账户返回原始密钥。
        let material = derive_chain_private_key_for_spec(&secret, None, ChainId::Ethereum).unwrap();
        assert_eq!(
            material.export().unwrap(),
            "0x0000000000000000000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    fn replace_last_index_swaps_only_the_final_component() {
        assert_eq!(replace_last_index("m/84'/0'/0'/0/0", 7), "m/84'/0'/0'/0/7");
        assert_eq!(
            replace_last_index("m/44'/60'/0'/0/0", 12),
            "m/44'/60'/0'/0/12"
        );
        // Hardened final components keep their marker.
        assert_eq!(
            replace_last_index("m/44'/501'/0'/0'", 2),
            "m/44'/501'/0'/2'"
        );
    }

    #[test]
    fn bip39_wallet_has_known_btc_and_eth_addresses() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let btc = accounts
            .iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        let eth = accounts
            .iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        assert_eq!(btc.address, "bc1qcr8te4kr609gcawutmrza0j4xv80jy8z306fyu");
        assert_eq!(
            eth.address.to_lowercase(),
            "0x9858effd232b4033e47d90003d41ec34ecaeda94"
        );
    }

    #[test]
    fn passphrase_is_deterministic_and_isolated() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let a = derive_accounts(&secret).unwrap();
        let b = derive_accounts(&secret).unwrap();
        assert_eq!(a[0].address, b[0].address);
        let btc = &a
            .iter()
            .find(|x| x.chain == ChainId::Bitcoin)
            .unwrap()
            .address;
        let ltc = &a
            .iter()
            .find(|x| x.chain == ChainId::Litecoin)
            .unwrap()
            .address;
        assert_ne!(btc, ltc);
    }

    #[test]
    fn passphrase_accounts_expose_the_chain_hash_not_the_derivation_detail() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let accounts = derive_accounts(&secret).unwrap();

        let ltc = accounts
            .iter()
            .find(|x| x.chain == ChainId::Litecoin)
            .unwrap();
        assert_eq!(ltc.derivation, "H_chain(phrase)");
        assert_eq!(ltc.hash_algorithm, "scrypt");
        assert_eq!(ltc.hash_detail, "N=16384, r=8, p=1 · 32 byte digest");

        let xmr = accounts
            .iter()
            .find(|x| x.chain == ChainId::Monero)
            .unwrap();
        assert_eq!(xmr.derivation, "H_chain(phrase)");
        assert_eq!(xmr.hash_algorithm, "Keccak-256");
        assert_eq!(xmr.hash_detail, "CryptoNote Keccak-256 · 32 byte digest");
    }

    #[test]
    fn bip39_wallet_has_known_ltc_and_sol_addresses() {
        let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let ltc = accounts
            .iter()
            .find(|a| a.chain == ChainId::Litecoin)
            .unwrap();
        let sol = accounts
            .iter()
            .find(|a| a.chain == ChainId::Solana)
            .unwrap();
        assert_eq!(ltc.address, "ltc1qjmxnz78nmc8nq77wuxh25n2es7rzm5c2rkk4wh");
        assert_eq!(sol.address, "HAgk14JpMQLgt6rVgv7cBQFJWFto5Dqxi472uT3DKpqk");
        // 哈希算法必须报真实哈希(BIP-39 钱包与口令钱包同一套 label),而不是派生方式串。
        assert_eq!(ltc.hash_algorithm, "scrypt");
        assert_eq!(sol.hash_algorithm, "SHA-256");
        let eth = accounts
            .iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        assert_eq!(eth.hash_algorithm, "Keccak-256");
        assert_eq!(eth.derivation, "BIP-32 / BIP-44");
    }

    #[test]
    fn passphrase_isolated_vectors() {
        // Cross-checked against an independent Python implementation
        // (bip_utils) of the same formula.
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let address = |chain: ChainId| {
            accounts
                .iter()
                .find(|a| a.chain == chain)
                .unwrap()
                .address
                .clone()
        };
        assert_eq!(
            address(ChainId::Bitcoin),
            "bc1qx0q0wdh4z5a64twecyg0kqyqkpc3h66nv8n2gt"
        );
        assert_eq!(
            address(ChainId::Litecoin),
            "ltc1qg06z6x74fe6f2qyqkfqk7kle8llgty3s78tfql"
        );
        assert_eq!(
            address(ChainId::Ethereum),
            "0x9231e3Fa53ABF00CeC08DB45a7dEDe19f939a0CB"
        );
        assert_eq!(
            address(ChainId::Solana),
            "5HeimWLA7nee4kKKL8mWbPjMHPXKYTY6gM43LgMk9Q4a"
        );
    }

    #[test]
    fn strict_mode_shares_evm_key() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Strict).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let eth = accounts
            .iter()
            .find(|a| a.chain == ChainId::Ethereum)
            .unwrap();
        let bsc = accounts.iter().find(|a| a.chain == ChainId::Bsc).unwrap();
        assert_eq!(eth.address, bsc.address);
        // Strict mode hashes the phrase alone, so every chain that uses the
        // same native hash ends up with the very same key — Ethereum Classic
        // and Polygon included. Only the chain's own parameters (chain id for
        // replay protection, explorer, symbol) differ.
        for chain in [ChainId::EthereumClassic, ChainId::Polygon] {
            let account = accounts.iter().find(|a| a.chain == chain).unwrap();
            assert_eq!(
                account.address, eth.address,
                "{chain:?} must share the ETH address"
            );
        }
        // Keccak chains are unaffected by the chain list; a SHA-256 chain is not.
        let btc = accounts
            .iter()
            .find(|a| a.chain == ChainId::Bitcoin)
            .unwrap();
        assert_ne!(btc.address, eth.address);
    }

    #[test]
    fn bip39_extended_chain_vectors() {
        // Independent vectors generated with bip_utils (BIP-44 account 0,
        // external index 0) for the mnemonic used across the test suite.
        let mnemonic =
            "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let address = |chain: ChainId| {
            accounts
                .iter()
                .find(|a| a.chain == chain)
                .unwrap()
                .address
                .clone()
        };

        // EVM chains sharing coin type 60 reuse the ETH address.
        let eth = address(ChainId::Ethereum);
        assert_eq!(eth, "0x58A57ed9d8d624cBD12e2C467D34787555bB1b25");
        for chain in [ChainId::Bsc, ChainId::Polygon] {
            assert_eq!(address(chain), eth, "{chain:?} must share the ETH address");
        }
        // Ethereum Classic uses its own coin type 61.
        assert_eq!(
            address(ChainId::EthereumClassic),
            "0x80857C0c11b30003Cf74F99Ed4D68DA41C7224aa"
        );
        // Legacy P2PKH (base58) and cashaddr formats.
        assert_eq!(
            address(ChainId::Dogecoin),
            "DMM78DyiwEPzWmykiQQUFcPcUv13YPe2ZK"
        );
        assert_eq!(
            address(ChainId::BitcoinCash),
            "bitcoincash:qpdtccrxx78kcuc65mceurfwg60gmmqu9cwpjdt25n"
        );
        // CryptoNote dual-key address (SLIP-10 kholaw m/44'/128'/0'/0/0,
        // spend = reduce(Keccak(kL ‖ kR)) — verified against bip_utils).
        assert_eq!(
            address(ChainId::Monero),
            "46XrxrAa5aXcfkFrdSACS2EfmfxHdJkXLcGuDD2ezLNFfrfMgBJPoP5NAEz3N9WSJuhPox5S78AY3PGVeZQczueyDDT66tY"
        );
    }

    #[test]
    fn passphrase_extended_chains_are_isolated() {
        let secret =
            WalletSecret::new_passphrase("a quiet garden at dusk", DerivationMode::Isolated)
                .unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let find = |chain: ChainId| accounts.iter().find(|a| a.chain == chain).unwrap();
        // Keccak chains in isolated mode mix the chain name into the message,
        // so every EVM chain gets its own key.
        let eth = find(ChainId::Ethereum).address.clone();
        for chain in [ChainId::Bsc, ChainId::Polygon, ChainId::EthereumClassic] {
            assert_ne!(
                find(chain).address,
                eth,
                "{chain:?} must differ from Ethereum in isolated mode"
            );
        }
        // Dogecoin shares the scrypt parameters with Litecoin but is still a
        // different chain name, hence a different key.
        assert_ne!(
            find(ChainId::Dogecoin).address,
            find(ChainId::Litecoin).address
        );
        assert!(find(ChainId::Dogecoin).send_supported);
        assert!(find(ChainId::BitcoinCash).send_supported);
        // Monero spends are signed by a local monero-wallet-rpc.
        assert!(find(ChainId::Monero).send_supported);
    }

    #[test]
    fn extended_utxo_chains_derive_addresses_with_expected_prefixes() {
        // 2026 追加的十条链:助记词派生(BIP-44 各自 coin type)+ 地址前缀。
        let mnemonic =
            "legal winner thank year wave sausage worth useful legal winner thank yellow";
        let secret = WalletSecret::new_mnemonic(mnemonic).unwrap();
        let accounts = derive_accounts(&secret).unwrap();
        let address = |chain: ChainId| {
            accounts
                .iter()
                .find(|a| a.chain == chain)
                .unwrap_or_else(|| panic!("{chain:?} account missing"))
                .address
                .clone()
        };

        // bech32(SegWit)三条 + 2026 二批的 MONA/FJC。
        assert!(
            address(ChainId::Viacoin).starts_with("via1"),
            "{}",
            address(ChainId::Viacoin)
        );
        assert!(address(ChainId::Groestlcoin).starts_with("grs1"));
        assert!(address(ChainId::Vertcoin).starts_with("vtc1"));
        assert!(address(ChainId::Monacoin).starts_with("mona1"));
        assert!(address(ChainId::Fujicoin).starts_with("fc1"));
        // P2PKH 前缀由 base58 版本字节决定。
        assert!(address(ChainId::BitcoinSv).starts_with('1'));
        assert!(
            address(ChainId::Namecoin).starts_with('N')
                || address(ChainId::Namecoin).starts_with('M')
        );
        assert!(address(ChainId::LitecoinCash).starts_with('C'));
        assert!(
            address(ChainId::Feathercoin).starts_with('6')
                || address(ChainId::Feathercoin).starts_with('7')
        );
        assert!(address(ChainId::Potcoin).starts_with('P'));
        assert!(address(ChainId::EGulden).starts_with('L'));
        assert!(address(ChainId::Dash).starts_with('X'));
        // Ravencoin P2PKH 版本字节 60 → R 前缀。
        assert!(address(ChainId::Ravencoin).starts_with('R'));
        // 每条链的地址都能通过自己的校验。
        for chain in [
            ChainId::BitcoinSv,
            ChainId::Namecoin,
            ChainId::LitecoinCash,
            ChainId::Feathercoin,
            ChainId::Viacoin,
            ChainId::Potcoin,
            ChainId::EGulden,
            ChainId::Dash,
            ChainId::Groestlcoin,
            ChainId::Vertcoin,
            ChainId::Ravencoin,
            ChainId::Monacoin,
            ChainId::Fujicoin,
        ] {
            let value = address(chain);
            assert!(
                crate::address::validate_address(chain, &value).is_ok(),
                "{chain:?} address failed self-validation: {value}"
            );
            assert!(accounts
                .iter()
                .any(|a| a.chain == chain && a.send_supported));
        }
        // VIA/GRS/VTC 的 44' 路径与 84' 前缀并存:路径字符串应与配置一致。
        let via = accounts
            .iter()
            .find(|a| a.chain == ChainId::Viacoin)
            .unwrap();
        assert_eq!(via.path, "m/44'/14'/0'/0/0");
    }
}
