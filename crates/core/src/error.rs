use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("vault already exists")]
    VaultExists,
    #[error("vault not found")]
    VaultNotFound,
    #[error("wallet is locked")]
    Locked,
    #[error("incorrect password")]
    IncorrectPassword,
    #[error("too many attempts, try again in {0} seconds")]
    TooManyAttempts(u64),
    #[error("password must be at least 8 characters")]
    WeakPassword,
    #[error("vault is corrupt or unsupported")]
    CorruptVault,
    #[error("wallet not found in this vault")]
    WalletNotFound,
    #[error("derived account not found in this wallet")]
    BatchAccountNotFound,
    #[error("the last wallet cannot be removed")]
    LastWallet,
    #[error("invalid mnemonic: {0}")]
    InvalidMnemonic(String),
    #[error("invalid passphrase: {0}")]
    InvalidPassphrase(String),
    #[error("invalid private key: {0}")]
    InvalidPrivateKey(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("invalid amount: {0}")]
    InvalidAmount(String),
    #[error("unsupported chain: {0}")]
    UnsupportedChain(String),
    #[error("transaction failed: {0}")]
    Transaction(String),
    #[error("network request failed: {0}")]
    Network(String),
    #[error("local RPC unavailable: {0}")]
    RpcUnavailable(String),
    #[error("cryptography error: {0}")]
    Crypto(String),
    #[error("invalid vault format: {0}")]
    Format(String),
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("operation cancelled")]
    Cancelled,
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("hex decode failed")]
    Hex(#[from] hex::FromHexError),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::VaultExists => "VAULT_EXISTS",
            Error::VaultNotFound => "VAULT_NOT_FOUND",
            Error::Locked => "LOCKED",
            Error::IncorrectPassword => "INCORRECT_PASSWORD",
            Error::TooManyAttempts(_) => "TOO_MANY_ATTEMPTS",
            Error::WeakPassword => "WEAK_PASSWORD",
            Error::CorruptVault | Error::Format(_) => "CORRUPT_VAULT",
            Error::WalletNotFound => "WALLET_NOT_FOUND",
            Error::BatchAccountNotFound => "BATCH_ACCOUNT_NOT_FOUND",
            Error::LastWallet => "LAST_WALLET",
            Error::InvalidMnemonic(_) => "INVALID_MNEMONIC",
            Error::InvalidPassphrase(_) => "INVALID_PASSPHRASE",
            Error::InvalidPrivateKey(_) => "INVALID_PRIVATE_KEY",
            Error::InvalidAddress(_) => "INVALID_ADDRESS",
            Error::InvalidAmount(_) => "INVALID_AMOUNT",
            Error::UnsupportedChain(_) => "UNSUPPORTED_CHAIN",
            Error::Transaction(_) => "TRANSACTION_FAILED",
            Error::Network(_) => "NETWORK",
            Error::RpcUnavailable(_) => "RPC_UNAVAILABLE",
            Error::Crypto(_) => "CRYPTO",
            Error::InsufficientBalance => "INSUFFICIENT_BALANCE",
            Error::Cancelled => "CANCELLED",
            Error::Message(_) => "ERROR",
            Error::Io(_) => "IO",
            Error::Json(_) => "JSON",
            Error::Hex(_) => "HEX",
        }
    }
}
