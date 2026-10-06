use crate::handlers::*;
use axum::Router;
use axum::routing::get;
use sled::Db;

pub async fn app(sled_db: Db) -> Router {
    Router::new().nest("/eth", eth_router(sled_db.clone()))
}

fn eth_router(sled_db: Db) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/latest_price", get(price_handler))
        .route("/price", get(price_at_handler))
        .route("/prices", get(prices_range_handler))
        // .route("/ohlc", get(ohlc_handler))
        // .route("/candles", get(candles_handler))
        // .route("/chart", get(chart_handler))
        // .route("/chart/line", get(line_chart_handler))
        // .route("/chart/candlestick", get(candlestick_chart_handler))
        // .route("/volume", get(volume_handler))
        // .route("/market_cap", get(market_cap_handler))
        // .route("/stats", get(stats_handler))
        // .route("/average", get(average_price_handler))
        // .route("/high", get(high_price_handler))
        // .route("/low", get(low_price_handler))
        .route("/ath", get(ath_handler))
        .route("/atl", get(atl_handler))
        // .route("/change", get(price_change_handler))
        // .route("/convert", get(convert_handler))
        .with_state(sled_db)
}
