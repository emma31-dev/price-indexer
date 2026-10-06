use alloy::providers::ProviderBuilder;

mod error;
mod handlers;
mod price_listener;
mod routes;
mod types;

#[tokio::main]
async fn main() {
    let sled_db = sled::open("uniswap_pairs.sled").unwrap();
    let provider = &ProviderBuilder::new()
        .connect("http://127.0.0.1:8584")
        .await
        .unwrap();
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
