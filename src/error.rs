use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("byte mismatch: expected {expected}, returned {returned}")]
    MissMatchByte { expected: usize, returned: usize },
    #[error("ABI decode error: {0}")]
    AbiDecode(#[from] alloy::sol_types::Error),
    #[error("alloy RPC error: {0}")]
    Provider(#[from] alloy::transports::RpcError<alloy::transports::TransportErrorKind>),
    #[error("sled database error: {0}")]
    Sled(#[from] sled::Error),
    #[error("Unknown error occured: {0}")]
    Unknown(String),
}
