use crate::types::*;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;
use sled::Db;

pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "Healthy")
}

pub async fn price_handler(
    State(db): State<Db>,
    Json(id): Json<PriceRequest>,
) -> impl IntoResponse {
    let now = chrono::Utc::now().timestamp() as u64;

    let start = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: now.saturating_add(5),
    };
    let end = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: now.saturating_sub(3_600),
    };

    let range = start..end;

    match db.range(range).next_back() {
        Some(Ok((key, value))) => {
            let price = match bincode::deserialize::<f64>(&value) {
                Ok(p) => p,
                Err(_) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode price")
                        .into_response();
                }
            };
            let response = PriceResponse {
                price,
                last_updated: bincode::deserialize::<TokenPairId>(&key)?.timestamp,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Some(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
        None => (StatusCode::NOT_FOUND, "Price not found").into_response(),
    }
}
