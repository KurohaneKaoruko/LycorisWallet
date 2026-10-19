//! Lycoris Wallet core.
//!
//! The crate is deliberately platform agnostic: it contains the cryptography,
//! deterministic derivation profiles, encrypted vault format, chain adapters
//! and transaction builders. The Tauri application is a thin shell around it.

pub mod address;
pub mod bip32;
pub mod chains;
pub mod cryptonote;
pub mod derive;
pub mod error;
pub mod hash;
pub mod models;
pub mod network;
pub mod qr;
pub mod slip10;
pub mod tx;
pub mod units;
pub mod vault;

pub use chains::{ChainId, ChainInfo, ChainKind, HashAlgorithm};
pub use derive::{
    derive_accounts, derive_fingerprint, derive_private_key, preview_derivation, Account,
    DerivationMode, PrivateKeyMaterial, WalletMode, WalletSecret,
};
pub use error::{Error, Result};
pub use models::*;
pub use vault::{VaultData, VaultFile, WalletEntry};
