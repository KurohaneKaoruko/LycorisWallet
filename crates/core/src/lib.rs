//! Lycoris Wallet core.
//!
//! The crate is deliberately platform agnostic: it contains the cryptography,
//! deterministic derivation profiles, encrypted vault format, chain adapters
//! and transaction builders. The Tauri application is a thin shell around it.

pub mod address;
pub mod bip32;
pub mod chains;
pub mod cryptonote;
pub mod cryptonote_mnemonic;
pub mod cryptonote_words;
pub mod derive;
pub mod error;
pub mod hash;
pub mod models;
#[cfg(feature = "network")]
pub mod network;
pub mod qr;
pub mod slip10;
pub mod tools;
pub mod tx;
pub mod units;
#[cfg(feature = "vault")]
pub mod vault;

pub use chains::{ChainId, ChainInfo, ChainKind, HashAlgorithm};
pub use derive::{
    derive_accounts, derive_accounts_batch, derive_accounts_for_spec, derive_chain_account,
    derive_chain_private_key_for_spec, derive_fingerprint, derive_private_key, preview_derivation,
    Account, BatchAccountSpec, BatchRequest, BatchStrategy, DerivationMode, DerivationPreview,
    PrivateKeyMaterial, WalletMode, WalletSecret,
};
pub use error::{Error, Result};
pub use models::*;
#[cfg(feature = "vault")]
pub use vault::{VaultData, VaultFile, WalletEntry};
