use crate::handlers::*;
use axum::Router;
use sled::Db;

pub async fn app(sled_db: Db) -> Router {
    Router::new().nest("/eth", eth_router(sled_db.clone()))
}

fn eth_router(sled_db: Db) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .with_state(sled_db)
}
