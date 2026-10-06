use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
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
    #[error("rkyv error: {0}")]
    Rkyv(#[from] rkyv::rancor::Error),
    #[error("Unknown error occured: {0}")]
    Unknown(String),
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let message = match &self {
            ServerError::MissMatchByte { expected, returned } => {
                format!("byte mismatch: expected {expected}, returned {returned}")
            }
            ServerError::AbiDecode(_) => "failed to decode ABI data".to_string(),
            ServerError::Provider(_) => "RPC provider error".to_string(),
            ServerError::Sled(_) => "database error".to_string(),
            ServerError::Rkyv(_) => "serialization error".to_string(),
            ServerError::Unknown(msg) => msg.clone(),
        };

        (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
    }
}
