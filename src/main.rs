use alloy::providers::ProviderBuilder;

mod error;
mod handlers;
mod price_listener;
mod routes;
mod types;

#[tokio::main]
async fn main() {
    let sled_db = sled::open("uniswap_pairs.sled").unwrap();
    let rpc_url = std::env::var("RPC_URL").expect("RPC_URL must be set");
    let provider = ProviderBuilder::new().connect(&rpc_url).await.unwrap();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    futures::try_join!(
        crate::price_listener::price_listener(sled_db.clone(), &provider),
        async {
            axum::serve(listener, crate::routes::app(sled_db.clone()).await)
                .await
                .expect("failed to serve axum app");
            Ok(())
        }
    )
    .unwrap();
}
