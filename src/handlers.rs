use crate::{error::ServerError, types::*};
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde_json::json;
use sled::Db;

type ServerResponse<T> = Result<T, ServerError>;

pub async fn health_handler() -> impl IntoResponse {
    (StatusCode::OK, Json(json!({"status": "Ok"})))
}

pub async fn price_handler(
    State(db): State<Db>,
    Json(id): Json<PriceRequest>,
) -> ServerResponse<(StatusCode, Json<PriceResponse>)> {
    let now = chrono::Utc::now().timestamp() as u64;
    price_at(&db, &id.pair_address, &id.version, now)
}

pub async fn price_at_handler(
    State(db): State<Db>,
    Json(id): Json<PriceAtRequest>,
) -> ServerResponse<(StatusCode, Json<PriceResponse>)> {
    price_at(&db, &id.pair_address, &id.version, id.timestamp)
}

fn price_at(
    db: &Db,
    pair_address: &PairAddress,
    version: &UniswapVersion,
    from_timestamp: u64,
) -> ServerResponse<(StatusCode, Json<PriceResponse>)> {
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
            let price = rkyv::from_bytes::<TickMeta, rkyv::rancor::Error>(&value)?.price;
            let last_updated =
                rkyv::from_bytes::<TokenPairId, rkyv::rancor::Error>(&key)?.timestamp;
            let response = PriceResponse {
                price,
                last_updated,
            };
            Ok((StatusCode::OK, Json(response)))
        }
        Some(Err(e)) => Err(ServerError::Sled(e)),
        None => Err(ServerError::Unknown("Price not found".into())),
    }
}

pub async fn prices_range_handler(
    State(db): State<Db>,
    Json(id): Json<PricesRangeRequest>,
) -> ServerResponse<(StatusCode, Json<PricesRangeResponse>)> {
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
        let (key, value) = item?;
        let price = rkyv::from_bytes::<TickMeta, rkyv::rancor::Error>(&value)?.price;
        let timestamp = rkyv::from_bytes::<TokenPairId, rkyv::rancor::Error>(&key)?.timestamp;
        prices.push(TimestampedPrice { price, timestamp });
    }

    Ok((StatusCode::OK, Json(PricesRangeResponse { prices })))
}

pub async fn ohlc_handler(
    State(db): State<Db>,
    Json(id): Json<PricesRangeRequest>,
) -> ServerResponse<(StatusCode, Json<Ohlc>)> {
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
        let (_, value) = item?;
        let price = rkyv::from_bytes::<TickMeta, rkyv::rancor::Error>(&value)?.price;
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
        Some(ohlc) => Ok((StatusCode::OK, Json(ohlc))),
        None => Err(ServerError::Unknown("Price not found".into())),
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

fn extreme_price_at(
    db: &Db,
    id: &PricesRangeRequest,
    find_high: bool,
) -> ServerResponse<(StatusCode, Json<TimestampedPrice>)> {
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
        let (key, value) = item?;
        let price = rkyv::from_bytes::<TickMeta, rkyv::rancor::Error>(&value)?.price;
        let timestamp = rkyv::from_bytes::<TokenPairId, rkyv::rancor::Error>(&key)?.timestamp;
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
            result = Some(TimestampedPrice { price, timestamp });
        }
    }

    match result {
        Some(extreme) => Ok((StatusCode::OK, Json(extreme))),
        None => Err(ServerError::Unknown("Price not found".into())),
    }
}
