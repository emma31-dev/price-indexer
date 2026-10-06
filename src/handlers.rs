use crate::types::*;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use sled::Db;

pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, "Healthy")
}

pub async fn price_handler(
    State(db): State<Db>,
    Json(id): Json<PriceRequest>,
) -> impl IntoResponse {
    let now = chrono::Utc::now().timestamp() as u64;
    price_at(&db, &id.pair_address, &id.version, now)
}

pub async fn price_at_handler(
    State(db): State<Db>,
    Json(id): Json<PriceAtRequest>,
) -> impl IntoResponse {
    price_at(&db, &id.pair_address, &id.version, id.timestamp)
}

fn price_at(
    db: &Db,
    pair_address: &PairAddress,
    version: &UniswapVersion,
    from_timestamp: u64,
) -> axum::response::Response {
    let start = TokenPairId {
        pair_address: pair_address.clone(),
        version: version.clone(),
        timestamp: from_timestamp.saturating_add(1),
    };
    let end = TokenPairId {
        pair_address: pair_address.clone(),
        version: version.clone(),
        timestamp: from_timestamp.saturating_sub(3_600),
    };

    let range = start..end;

    match db.range(range).next_back() {
        Some(Ok((key, value))) => {
            let price = match rkyv::from_bytes::<TickMeta, rkyv::rancor::Error>(&value) {
                Ok(p) => p.price,
                Err(_) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode price")
                        .into_response();
                }
            };
            let timestamp = match rkyv::from_bytes::<TokenPairId, rkyv::rancor::Error>(&key) {
                Ok(k) => k.timestamp,
                Err(_) => {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode key")
                        .into_response();
                }
            };
            let response = PriceResponse {
                price,
                last_updated: timestamp,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Some(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response(),
        None => (StatusCode::NOT_FOUND, "Price not found").into_response(),
    }
}

pub async fn prices_range_handler(
    State(db): State<Db>,
    Json(id): Json<PricesRangeRequest>,
) -> impl IntoResponse {
    let start_ts = id
        .start_timestamp
        .max(id.end_timestamp.saturating_sub(3_600));
    let end_ts = id.end_timestamp;

    let start = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: start_ts,
    };
    let end = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: end_ts,
    };

    let mut prices = Vec::new();
    for item in db.range(start..end) {
        let (key, value) = match item {
            Ok(kv) => kv,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
            }
        };
        let price = match rkyv::from_bytes::<TickMeta, rkyv::rancor::Error>(&value) {
            Ok(p) => p.price,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode price")
                    .into_response();
            }
        };
        let timestamp = match rkyv::from_bytes::<TokenPairId, rkyv::rancor::Error>(&key) {
            Ok(k) => k.timestamp,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode key").into_response();
            }
        };
        prices.push(TimestampedPrice { price, timestamp });
    }

    (StatusCode::OK, Json(PricesRangeResponse { prices })).into_response()
}

pub async fn ohlc_handler(
    State(db): State<Db>,
    Json(id): Json<PricesRangeRequest>,
) -> impl IntoResponse {
    let start = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: id.start_timestamp,
    };
    let end = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: id.end_timestamp,
    };

    let mut ohlc: Option<Ohlc> = None;
    for item in db.range(start..end) {
        let (_, value) = match item {
            Ok(kv) => kv,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
            }
        };
        let price = match rkyv::from_bytes::<Option<TickMeta>, rkyv::rancor::Error>(&value) {
            Ok(Some(p)) => p.price,
            Ok(None) => continue,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode price")
                    .into_response();
            }
        };
        ohlc = Some(match ohlc {
            None => Ohlc {
                open: price,
                high: price,
                low: price,
                close: price,
            },
            Some(current) => Ohlc {
                open: current.open,
                high: if price > current.high {
                    price
                } else {
                    current.high
                },
                low: if price < current.low {
                    price
                } else {
                    current.low
                },
                close: price,
            },
        });
    }

    match ohlc {
        Some(ohlc) => (StatusCode::OK, Json(ohlc)).into_response(),
        None => (StatusCode::NOT_FOUND, "Price not found").into_response(),
    }
}

pub async fn ath_handler(
    State(db): State<Db>,
    Json(id): Json<PricesRangeRequest>,
) -> impl IntoResponse {
    extreme_price_at(&db, &id, true)
}

pub async fn atl_handler(
    State(db): State<Db>,
    Json(id): Json<PricesRangeRequest>,
) -> impl IntoResponse {
    extreme_price_at(&db, &id, false)
}

fn extreme_price_at(db: &Db, id: &PricesRangeRequest, find_high: bool) -> axum::response::Response {
    let start = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: id.start_timestamp,
    };
    let end = TokenPairId {
        pair_address: id.pair_address.clone(),
        version: id.version.clone(),
        timestamp: id.end_timestamp,
    };

    let mut result: Option<TimestampedPrice> = None;
    for item in db.range(start..end) {
        let (key, value) = match item {
            Ok(kv) => kv,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response();
            }
        };
        let price = match rkyv::from_bytes::<Option<TickMeta>, rkyv::rancor::Error>(&value) {
            Ok(Some(p)) => Some(p.price),
            Ok(None) => None,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode price")
                    .into_response();
            }
        };
        let timestamp = match rkyv::from_bytes::<TokenPairId, rkyv::rancor::Error>(&key) {
            Ok(k) => k.timestamp,
            Err(_) => {
                return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to decode key").into_response();
            }
        };
        let price = match price {
            Some(p) => p,
            None => continue,
        };
        let is_better = match &result {
            None => true,
            Some(current) => {
                if find_high {
                    price > current.price
                } else {
                    price < current.price
                }
            }
        };
        if is_better {
            result = Some(TimestampedPrice {
                price: price,
                timestamp,
            });
        }
    }

    match result {
        Some(extreme) => (StatusCode::OK, Json(extreme)).into_response(),
        None => (StatusCode::NOT_FOUND, "Price not found").into_response(),
    }
}
